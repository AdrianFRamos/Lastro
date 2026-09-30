//! Cross-language fixed serial payload contracts (protocol v2).

use std::{fs, path::PathBuf};

use lastro_agent::serial::payload::{
    ACK_PAYLOAD_LEN, COMMAND_PAYLOAD_LEN, ERROR_PAYLOAD_LEN, EVENT_READY_PAYLOAD_LEN,
    StationErrorCode, decode_ack, decode_command, decode_error, decode_event_ready, encode_ack,
    encode_command, encode_error, encode_event_ready,
};
use lastro_protocol::v2::EventType;
use serde_json::Value;
use uuid::Uuid;

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name)).expect("read fixture")
}

fn vectors() -> Value {
    serde_json::from_slice(&fixture("v2-capture.json")).expect("v2 vectors JSON")
}

fn vector_hex(capture: &str, field: &str) -> Vec<u8> {
    hex::decode(vectors()["captures"][capture][field].as_str().unwrap()).unwrap()
}

#[test]
fn command_fixture_is_exactly_202_bytes_and_roundtrips_every_field() {
    // PURPOSE: Freeze the Rust COMMAND payload against the bytes consumed by firmware/simulator.
    // ASSERT: Every field and the complete re-encoded payload match the committed fixture.
    // FAILURE MEANS: Rust Agent and firmware disagree about COMMAND offsets or endianness.
    let bytes = fixture("v2-serial-command.bin");
    assert_eq!(bytes.len(), COMMAND_PAYLOAD_LEN);
    let command = decode_command(&bytes).expect("valid command fixture").0;
    assert_eq!(command.capture_id, [0x01; 16]);
    assert_eq!(command.event_type, EventType::IdentifierBound);
    assert_eq!(command.deployment_id, [0xd0; 32]);
    assert_eq!(command.asset_id, [0x11; 32]);
    assert_eq!(command.state_version, 1);
    assert_eq!(command.previous_event_hash, [0; 32]);
    assert_eq!(command.expected_rfid_hash, [0; 32]);
    assert_eq!(
        encode_command(&lastro_agent::command::StationCommand(command))
            .unwrap()
            .as_slice(),
        bytes
    );
}

#[test]
fn command_with_unknown_event_type_or_inconsistent_context_is_rejected() {
    // PURPOSE: Reject unsupported event types rather than partially decoding them.
    // ASSERT: Non-capture event types and a forged expected RFID fail to decode.
    // FAILURE MEANS: Station and Agent could interpret an unknown COMMAND differently.
    let fixture = fixture("v2-serial-command.bin");
    for event_type in [0u16, 1, 4, 20, u16::MAX] {
        let mut bytes = fixture.clone();
        bytes[16..18].copy_from_slice(&event_type.to_le_bytes());
        assert!(decode_command(&bytes).is_err());
    }
    let mut bytes = fixture.clone();
    bytes[154] = 1;
    assert!(decode_command(&bytes).is_err());
}

#[test]
fn event_ready_fixture_is_exactly_341_bytes_and_preserves_signed_envelope() {
    // PURPOSE: Preserve the Station-signed evidence byte-for-byte across the serial bridge.
    // ASSERT: Decoded envelope, RFID and signature equal the vectors and re-encode exactly.
    // FAILURE MEANS: Agent transport could corrupt evidence before SQLite/API persistence.
    let bytes = fixture("v2-serial-event-ready.bin");
    assert_eq!(bytes.len(), EVENT_READY_PAYLOAD_LEN);
    let payload = decode_event_ready(&bytes).expect("valid event fixture");
    assert_eq!(payload.capture_id, Uuid::from_bytes([0x01; 16]));
    assert_eq!(
        payload.event_bytes.to_vec(),
        vector_hex("bind", "envelope_hex")
    );
    assert_eq!(payload.observed_rfid, [0x80, 0x00, 0x13, 0, 0, 0, 0, 1]);
    assert_eq!(
        payload.station_signature64.to_vec(),
        vector_hex("bind", "station_signature_hex")
    );
    assert_eq!(encode_event_ready(&payload).as_slice(), bytes);
}

#[test]
fn ack_fixture_binds_capture_id_to_exact_event_hash() {
    // PURPOSE: Bind a Station ACK to the capture and exact persisted event hash.
    // ASSERT: The 48-byte fixture decodes to the expected capture/hash and round-trips exactly.
    // FAILURE MEANS: Station could accept an ACK for a different event or capture.
    let bytes = fixture("v2-serial-ack.bin");
    assert_eq!(bytes.len(), ACK_PAYLOAD_LEN);
    let payload = decode_ack(&bytes).expect("valid ACK fixture");
    assert_eq!(payload.capture_id, Uuid::from_bytes([0x01; 16]));
    assert_eq!(
        payload.event_hash.to_vec(),
        vector_hex("bind", "event_hash_hex")
    );
    assert_eq!(encode_ack(&payload).as_slice(), bytes);
}

#[test]
fn error_payload_accepts_only_defined_codes_and_zero_reserved_bytes() {
    // PURPOSE: Freeze Station ERROR semantics to the six documented codes and zero reserved bytes.
    // ASSERT: The fixture decodes, while undefined codes and reserved-byte mutations fail.
    // FAILURE MEANS: Firmware and Agent can disagree about Station error semantics.
    let fixture = fixture("serial-error.bin");
    assert_eq!(fixture.len(), ERROR_PAYLOAD_LEN);
    let payload = decode_error(&fixture).expect("valid error fixture");
    assert_eq!(payload.code, StationErrorCode::RfidReadFailed);
    assert_eq!(encode_error(&payload).as_slice(), fixture);

    let mut timeout = fixture.clone();
    timeout[16..18].copy_from_slice(&6u16.to_le_bytes());
    assert_eq!(
        decode_error(&timeout).unwrap().code,
        StationErrorCode::RfidTimeout
    );
    for code in [0u16, 7, u16::MAX] {
        let mut bytes = fixture.clone();
        bytes[16..18].copy_from_slice(&code.to_le_bytes());
        assert!(decode_error(&bytes).is_err());
    }
    for index in 18..20 {
        let mut bytes = fixture.clone();
        bytes[index] = 1;
        assert!(decode_error(&bytes).is_err());
    }
}
