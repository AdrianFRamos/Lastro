//! Cross-language fixed serial payload contracts.

use std::{fs, path::PathBuf};

use lastro_agent::serial::payload::{
    decode_ack, decode_command, decode_error, decode_event_ready, encode_ack, encode_command,
    encode_error, encode_event_ready, StationErrorCode, ACK_PAYLOAD_LEN, COMMAND_PAYLOAD_LEN,
    ERROR_PAYLOAD_LEN, EVENT_READY_PAYLOAD_LEN,
};
use uuid::Uuid;

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name)).expect("read fixture")
}

#[test]
fn command_fixture_is_exactly_224_bytes_and_roundtrips_every_field() {
    // PURPOSE: Freeze the Rust COMMAND payload against the same 224 bytes consumed by firmware.
    // ASSERT: Every field and the complete re-encoded payload match the committed fixture exactly.
    // FAILURE MEANS: Rust Agent and firmware disagree about COMMAND offsets or endianness.
    let bytes = fixture("serial-command.bin");
    assert_eq!(bytes.len(), COMMAND_PAYLOAD_LEN);
    let command = decode_command(&bytes).expect("valid command fixture");
    assert_eq!(command.capture_id, Uuid::parse_str("00112233-4455-6677-8899-aabbccddeeff").unwrap());
    assert_eq!(command.action, 1);
    assert_eq!(command.deployment_id, [0xd0; 32]);
    assert_eq!(command.animal_id, [0x11; 32]);
    assert_eq!(command.event_sequence, 1);
    assert_eq!(command.identity_revision, 1);
    assert_eq!(command.previous_event_hash, [0; 32]);
    assert_eq!(command.expected_old_rfid_hash, [0; 32]);
    assert_eq!(command.from_custodian, [0; 32]);
    assert_eq!(command.to_custodian, [0xa1; 32]);
    assert_eq!(encode_command(&command).unwrap().as_slice(), bytes);
}

#[test]
fn command_reserved_bytes_and_unknown_action_are_rejected() {
    // PURPOSE: Reject unsupported protocol extensions/actions rather than partially decoding them.
    // ASSERT: Every reserved-byte mutation and action 0/4/255 returns a contract error.
    // FAILURE MEANS: Station and Agent could interpret an unknown COMMAND differently.
    let fixture = fixture("serial-command.bin");
    for index in 17..20 {
        let mut bytes = fixture.clone();
        bytes[index] = 1;
        assert!(decode_command(&bytes).is_err());
    }
    for action in [0, 4, 255] {
        let mut bytes = fixture.clone();
        bytes[16] = action;
        assert!(decode_command(&bytes).is_err());
    }
}

#[test]
fn event_ready_fixture_is_exactly_397_bytes_and_preserves_signed_event() {
    // PURPOSE: Preserve the Station-signed evidence byte-for-byte across the serial bridge.
    // ASSERT: The decoded fields and complete re-encoded payload exactly match committed fixtures.
    // FAILURE MEANS: Agent transport could corrupt evidence before SQLite/API persistence.
    let bytes = fixture("serial-event-ready.bin");
    let origin = fixture("origin.bin");
    assert_eq!(bytes.len(), EVENT_READY_PAYLOAD_LEN);
    let payload = decode_event_ready(&bytes).expect("valid event fixture");
    assert_eq!(payload.capture_id, Uuid::parse_str("00112233-4455-6677-8899-aabbccddeeff").unwrap());
    assert_eq!(payload.event_bytes.as_slice(), origin);
    assert_eq!(payload.observed_rfid, [0x80, 0x00, 0x13, 0, 0, 0, 0, 1]);
    assert_eq!(payload.station_pubkey33.len(), 33);
    assert_eq!(payload.station_signature64.len(), 64);
    assert_eq!(encode_event_ready(&payload).as_slice(), bytes);
}

#[test]
fn ack_fixture_binds_capture_id_to_exact_event_hash() {
    // PURPOSE: Bind a Station ACK to the capture and exact persisted event hash.
    // ASSERT: The 48-byte fixture decodes to the expected capture/hash and round-trips exactly.
    // FAILURE MEANS: Station could accept an ACK for a different event or capture.
    let bytes = fixture("serial-ack.bin");
    assert_eq!(bytes.len(), ACK_PAYLOAD_LEN);
    let payload = decode_ack(&bytes).expect("valid ACK fixture");
    assert_eq!(payload.capture_id, Uuid::parse_str("00112233-4455-6677-8899-aabbccddeeff").unwrap());
    assert_eq!(payload.event_hash, [0x58,0x45,0xdc,0x20,0xfd,0x6b,0x26,0x6e,0xc9,0x83,0x99,0xf0,0xaa,0x93,0xc7,0x36,0xec,0x9a,0xa7,0x78,0xbf,0x03,0x8a,0xf5,0x29,0x1e,0x5d,0xf8,0x13,0x34,0xb5,0x31]);
    assert_eq!(encode_ack(&payload).as_slice(), bytes);
}

#[test]
fn error_payload_accepts_only_defined_codes_and_zero_reserved_bytes() {
    // PURPOSE: Freeze Station ERROR semantics to the five documented codes and zero reserved bytes.
    // ASSERT: The fixture decodes, while undefined codes and reserved-byte mutations fail.
    // FAILURE MEANS: Firmware and Agent can disagree about Station error semantics.
    let fixture = fixture("serial-error.bin");
    assert_eq!(fixture.len(), ERROR_PAYLOAD_LEN);
    let payload = decode_error(&fixture).expect("valid error fixture");
    assert_eq!(payload.code, StationErrorCode::RfidReadFailed);
    assert_eq!(encode_error(&payload).as_slice(), fixture);

    for code in [0u16, 6, u16::MAX] {
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
