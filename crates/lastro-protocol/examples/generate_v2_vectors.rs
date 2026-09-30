//! Deterministic v2 capture interoperability vectors.
//!
//! Writes `test-vectors/v2-capture.json`, consumed by the Agent, the hardware simulator, the
//! firmware fixtures and the API. The Station key is the test-only scalar 1 and signatures use
//! RFC 6979, so regenerating produces byte-identical output.
//!
//! Run: `cargo run -p lastro-protocol --example generate_v2_vectors`

use std::{fs, path::PathBuf};

use lastro_protocol::{
    crypto::derive_station_id,
    rfid::{canonical_rfid_from_u64, hash_canonical_rfid},
    v2::{
        CaptureCommand, CaptureEventReady, EventType, capture_envelope, capture_event_id,
        identifier_payload_hash,
    },
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer};
use serde_json::{Value, json};

const TEST_PRIVATE_SCALAR: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = SigningKey::from_bytes((&TEST_PRIVATE_SCALAR).into())?;
    let pubkey33: [u8; 33] = key
        .verifying_key()
        .to_encoded_point(true)
        .as_bytes()
        .try_into()
        .expect("compressed key");
    let station_id = derive_station_id(&pubkey33)?;

    let deployment_id = [0xd0; 32];
    let asset_id = [0x11; 32];
    let tag_a = canonical_rfid_from_u64(0x8000_1300_0000_0001);
    let tag_b = canonical_rfid_from_u64(0x8000_1300_0000_0002);
    let (hash_a, hash_b) = (hash_canonical_rfid(&tag_a), hash_canonical_rfid(&tag_b));

    let steps = [
        ("bind", EventType::IdentifierBound, [0u8; 32], tag_a, 0x01u8),
        (
            "replace",
            EventType::IdentifierReplaced,
            hash_a,
            tag_b,
            0x02,
        ),
        (
            "observe",
            EventType::ObservationRecorded,
            hash_b,
            tag_b,
            0x03,
        ),
    ];
    let mut previous = [0u8; 32];
    let mut captures = serde_json::Map::new();
    for (index, (name, event_type, expected, tag, capture_byte)) in steps.into_iter().enumerate() {
        let capture_id = [capture_byte; 16];
        let command = CaptureCommand {
            capture_id,
            event_type,
            deployment_id,
            asset_id,
            event_id: capture_event_id(&capture_id),
            state_version: index as u64 + 1,
            previous_event_hash: previous,
            expected_rfid_hash: expected,
            observed_at: 1_700_000_000 + index as i64 * 60,
            expires_at: 1_700_000_300 + index as i64 * 60,
        };
        let envelope = capture_envelope(&command, &tag, station_id)?;
        let envelope_bytes = envelope.encode()?;
        let signature: Signature = key.sign(&envelope_bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        let signature64: [u8; 64] = signature.to_bytes().into();
        let event_hash = envelope.event_hash()?;
        let ready = CaptureEventReady {
            capture_id,
            envelope: envelope_bytes,
            observed_rfid: tag,
            station_pubkey33: pubkey33,
            station_signature64: signature64,
        };
        let mut ack = capture_id.to_vec();
        ack.extend_from_slice(&event_hash);
        captures.insert(
            name.into(),
            json!({
                "command_hex": hex::encode(command.encode()?),
                "observed_rfid_hex": hex::encode(tag),
                "envelope_hex": hex::encode(envelope_bytes),
                "payload_hash_hex": hex::encode(envelope.payload_hash),
                "station_signature_hex": hex::encode(signature64),
                "event_hash_hex": hex::encode(event_hash),
                "event_ready_hex": hex::encode(ready.encode()),
                "ack_hex": hex::encode(ack),
            }),
        );
        previous = event_hash;
    }

    let output: Value = json!({
        "description": "Lastro v2 capture vectors (bind -> replace -> observe). Test-only Station key scalar = 1.",
        "station": {
            "private_scalar_hex": hex::encode(TEST_PRIVATE_SCALAR),
            "pubkey33_hex": hex::encode(pubkey33),
            "station_id_hex": hex::encode(station_id),
        },
        "deployment_id_hex": hex::encode(deployment_id),
        "asset_id_hex": hex::encode(asset_id),
        "tags": {
            "a": { "canonical_hex": hex::encode(tag_a), "hash_hex": hex::encode(hash_a) },
            "b": { "canonical_hex": hex::encode(tag_b), "hash_hex": hex::encode(hash_b) },
        },
        "identifier_payload_bound_hex": hex::encode(identifier_payload_hash([0; 32], hash_a)),
        "captures": captures,
    });
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test-vectors/v2-capture.json");
    fs::write(&path, serde_json::to_string_pretty(&output)? + "\n")?;
    // Serial fixtures for the first capture, read byte-for-byte by Agent/simulator/firmware tests.
    let bind = &output["captures"]["bind"];
    for (file, field) in [
        ("v2-serial-command.bin", "command_hex"),
        ("v2-serial-event-ready.bin", "event_ready_hex"),
        ("v2-serial-ack.bin", "ack_hex"),
    ] {
        let hex_value = bind[field].as_str().expect("hex field");
        fs::write(path.with_file_name(file), hex::decode(hex_value)?)?;
    }
    println!("wrote {}", path.display());
    Ok(())
}
