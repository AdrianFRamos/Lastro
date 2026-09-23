//! Station command contracts enforced by the Lastro Agent.

use lastro_agent::{
    command::StationCommand,
    serial::payload::{COMMAND_PAYLOAD_LEN, decode_command, encode_command},
};
use serde_json::Value;
use uuid::Uuid;

const ZERO32: [u8; 32] = [0; 32];

fn base_command(action: u8) -> StationCommand {
    let mut command = StationCommand {
        capture_id: Uuid::parse_str("00112233-4455-6677-8899-aabbccddeeff").unwrap(),
        action,
        deployment_id: [0xd0; 32],
        animal_id: [0x11; 32],
        event_sequence: 1,
        identity_revision: 1,
        previous_event_hash: ZERO32,
        expected_old_rfid_hash: ZERO32,
        from_custodian: ZERO32,
        to_custodian: [0xa1; 32],
    };
    match action {
        1 => {}
        2 => {
            command.event_sequence = 2;
            command.previous_event_hash = [0x22; 32];
            command.expected_old_rfid_hash = [0x33; 32];
            command.from_custodian = [0xa1; 32];
            command.to_custodian = [0xb2; 32];
        }
        3 => {
            command.event_sequence = 3;
            command.identity_revision = 2;
            command.previous_event_hash = [0x44; 32];
            command.expected_old_rfid_hash = [0x55; 32];
            command.from_custodian = [0xb2; 32];
            command.to_custodian = [0xb2; 32];
        }
        _ => unreachable!(),
    }
    command
}

#[test]
fn command_has_no_new_rfid_hash_field() {
    // PURPOSE: The backend must not determine the future physical RFID observation.
    // ASSERT: JSON and the fixed 224-byte wire command expose expected_old_rfid_hash only; no future RFID field exists.
    // FAILURE MEANS: The backend or Agent could dictate which replacement tag the Station should claim to observe.
    let command = base_command(3);
    let json = serde_json::to_value(&command).unwrap();
    let object = json.as_object().unwrap();
    assert!(object.contains_key("expected_old_rfid_hash"));
    assert!(!object.contains_key("new_rfid"));
    assert!(!object.contains_key("new_rfid_hash"));
    assert!(!object.contains_key("observed_rfid"));

    let encoded = encode_command(&command).unwrap();
    assert_eq!(encoded.len(), COMMAND_PAYLOAD_LEN);
    assert_eq!(decode_command(&encoded).unwrap(), command);
}

#[test]
fn origin_command_uses_zero_old_rfid_and_predecessor() {
    // PURPOSE: ORIGIN must carry deterministic genesis context.
    // ASSERT: The encoded/decoded command preserves zero predecessor, zero old RFID, zero source custodian, sequence 1 and revision 1.
    // FAILURE MEANS: The Station could sign ORIGIN against nonexistent prior state.
    let command = base_command(1);
    let decoded = decode_command(&encode_command(&command).unwrap()).unwrap();
    assert_eq!(decoded.event_sequence, 1);
    assert_eq!(decoded.identity_revision, 1);
    assert_eq!(decoded.previous_event_hash, ZERO32);
    assert_eq!(decoded.expected_old_rfid_hash, ZERO32);
    assert_eq!(decoded.from_custodian, ZERO32);
    assert_eq!(decoded.to_custodian, [0xa1; 32]);
}

#[test]
fn transfer_command_uses_current_rfid_and_revision() {
    // PURPOSE: TRANSFER must carry the exact current canonical context into the physical Station observation.
    // ASSERT: RFID, revision, predecessor, custodians and next sequence survive binary encode/decode unchanged.
    // FAILURE MEANS: The Station could sign stale or fabricated transfer context.
    let command = base_command(2);
    let decoded = decode_command(&encode_command(&command).unwrap()).unwrap();
    assert_eq!(decoded.event_sequence, 2);
    assert_eq!(decoded.identity_revision, 1);
    assert_eq!(decoded.previous_event_hash, [0x22; 32]);
    assert_eq!(decoded.expected_old_rfid_hash, [0x33; 32]);
    assert_eq!(decoded.from_custodian, [0xa1; 32]);
    assert_eq!(decoded.to_custodian, [0xb2; 32]);
}

#[test]
fn reidentify_command_carries_old_binding_only() {
    // PURPOSE: The replacement RFID must originate from the Station's physical read, never from backend command data.
    // ASSERT: REIDENTIFY carries only the previous binding plus incremented revision and unchanged custodian authority.
    // FAILURE MEANS: The Agent/backend could inject the replacement tag instead of observing it physically.
    let command = base_command(3);
    let decoded = decode_command(&encode_command(&command).unwrap()).unwrap();
    assert_eq!(decoded.event_sequence, 3);
    assert_eq!(decoded.identity_revision, 2);
    assert_eq!(decoded.previous_event_hash, [0x44; 32]);
    assert_eq!(decoded.expected_old_rfid_hash, [0x55; 32]);
    assert_eq!(decoded.from_custodian, [0xb2; 32]);
    assert_eq!(decoded.to_custodian, [0xb2; 32]);

    let json: Value = serde_json::to_value(&decoded).unwrap();
    let object = json.as_object().unwrap();
    assert_eq!(object.len(), 10);
    assert!(!object.keys().any(|key| key.contains("new_rfid")));
}
