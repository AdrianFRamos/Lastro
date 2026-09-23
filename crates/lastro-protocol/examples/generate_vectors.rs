//! Deterministic test-only fixture generator.
//!
//! This example writes a self-consistent synthetic fixture set under
//! `test-vectors/generated/`. It never overwrites the frozen interoperability fixtures and
//! never uses production Station or funded wallet secrets.

use std::{fs, path::PathBuf};

use lastro_protocol::{
    Action, StationEvent,
    crypto::derive_station_id,
    rfid::{canonical_rfid_from_u64, hash_canonical_rfid},
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer};
use serde_json::json;

const TEST_PRIVATE_SCALAR: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let signing_key = SigningKey::from_bytes((&TEST_PRIVATE_SCALAR).into())?;
    let encoded_point = signing_key.verifying_key().to_encoded_point(true);
    let station_pubkey: [u8; 33] = encoded_point
        .as_bytes()
        .try_into()
        .expect("compressed P-256 public key is 33 bytes");
    let station_id = derive_station_id(&station_pubkey)?;

    let deployment_id = [0xd0; 32];
    let animal_id = [0x11; 32];
    let custodian_a = [0xa1; 32];
    let custodian_b = [0xb2; 32];
    let custodian_c = [0xc3; 32];
    let rfid_a = canonical_rfid_from_u64(0x8000_1300_0000_0001);
    let rfid_b = canonical_rfid_from_u64(0x8000_1300_0000_0002);
    let rfid_a_hash = hash_canonical_rfid(&rfid_a);
    let rfid_b_hash = hash_canonical_rfid(&rfid_b);

    let origin = StationEvent {
        action: Action::Origin,
        deployment_id,
        animal_id,
        station_id,
        event_sequence: 1,
        identity_revision: 1,
        previous_event_hash: [0; 32],
        old_rfid_hash: [0; 32],
        new_rfid_hash: rfid_a_hash,
        from_custodian: [0; 32],
        to_custodian: custodian_a,
    };
    let transfer = StationEvent {
        action: Action::Transfer,
        deployment_id,
        animal_id,
        station_id,
        event_sequence: 2,
        identity_revision: 1,
        previous_event_hash: origin.event_hash(),
        old_rfid_hash: rfid_a_hash,
        new_rfid_hash: rfid_a_hash,
        from_custodian: custodian_a,
        to_custodian: custodian_b,
    };
    let reidentify = StationEvent {
        action: Action::Reidentify,
        deployment_id,
        animal_id,
        station_id,
        event_sequence: 3,
        identity_revision: 2,
        previous_event_hash: transfer.event_hash(),
        old_rfid_hash: rfid_a_hash,
        new_rfid_hash: rfid_b_hash,
        from_custodian: custodian_b,
        to_custodian: custodian_b,
    };
    let transfer_b_to_c = StationEvent {
        action: Action::Transfer,
        deployment_id,
        animal_id,
        station_id,
        event_sequence: 4,
        identity_revision: 2,
        previous_event_hash: reidentify.event_hash(),
        old_rfid_hash: rfid_b_hash,
        new_rfid_hash: rfid_b_hash,
        from_custodian: custodian_b,
        to_custodian: custodian_c,
    };

    transfer.validate_successor(&origin)?;
    reidentify.validate_successor(&transfer)?;
    transfer_b_to_c.validate_successor(&reidentify)?;

    let generated = output_dir();
    fs::create_dir_all(&generated)?;
    let events = [
        ("origin", &origin),
        ("transfer", &transfer),
        ("reidentify", &reidentify),
        ("transfer-b-to-c", &transfer_b_to_c),
    ];

    let mut event_json = serde_json::Map::new();
    for (name, event) in events {
        let bytes = event.encode();
        let signature: Signature = signing_key.sign(&bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        fs::write(generated.join(format!("{name}.bin")), bytes)?;
        event_json.insert(
            name.replace('-', "_"),
            json!({
                "event_bytes_hex": hex::encode(bytes),
                "event_hash_hex": hex::encode(event.event_hash()),
                "station_signature_hex": hex::encode(signature.to_bytes()),
            }),
        );
    }

    let vectors = json!({
        "version": 1,
        "fixture_scope": "deterministic generated test-only fixture; scalar 1 is public and must never be used in a deployment",
        "deployment_id_hex": hex::encode(deployment_id),
        "animal_id_hex": hex::encode(animal_id),
        "station": {
            "test_key_notice": "public key derives from fixed test-only scalar 1; never use this key in Station firmware or deployment",
            "pubkey_compressed_hex": hex::encode(station_pubkey),
            "station_id_hex": hex::encode(station_id),
        },
        "rfid": {
            "a_canonical_hex": hex::encode(rfid_a),
            "a_hash_hex": hex::encode(rfid_a_hash),
            "b_canonical_hex": hex::encode(rfid_b),
            "b_hash_hex": hex::encode(rfid_b_hash),
        },
        "custodians": {
            "a_hex": hex::encode(custodian_a),
            "b_hex": hex::encode(custodian_b),
            "c_hex": hex::encode(custodian_c),
        },
        "events": event_json,
    });
    fs::write(
        generated.join("vectors.json"),
        serde_json::to_vec_pretty(&vectors)?,
    )?;

    println!(
        "generated deterministic test fixtures at {}",
        generated.display()
    );
    Ok(())
}

fn output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("test-vectors")
        .join("generated")
}
