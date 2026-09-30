//! Station command contracts enforced by the Lastro Agent (protocol v2).

use lastro_agent::{
    command::StationCommand,
    serial::payload::{COMMAND_PAYLOAD_LEN, decode_command, encode_command},
};
use lastro_protocol::v2::{CaptureCommand, EventType, capture_event_id};

const ZERO32: [u8; 32] = [0; 32];

fn command(event_type: EventType, expected: [u8; 32], version: u64) -> StationCommand {
    let capture_id = [0x42; 16];
    StationCommand(CaptureCommand {
        capture_id,
        event_type,
        deployment_id: [0xd0; 32],
        asset_id: [0x11; 32],
        event_id: capture_event_id(&capture_id),
        state_version: version,
        previous_event_hash: if version == 1 { ZERO32 } else { [0x22; 32] },
        expected_rfid_hash: expected,
        observed_at: 1_000,
        expires_at: 1_300,
    })
}

#[test]
fn command_carries_only_the_expected_rfid_never_the_new_one() {
    // PURPOSE: The backend must not determine the future physical RFID observation.
    // ASSERT: The fixed wire command round-trips and has no field for the RFID to be read.
    // FAILURE MEANS: The backend or Agent could dictate which tag the Station claims to observe.
    let replace = command(EventType::IdentifierReplaced, [0x33; 32], 2);
    let encoded = encode_command(&replace).unwrap();
    assert_eq!(encoded.len(), COMMAND_PAYLOAD_LEN);
    assert_eq!(decode_command(&encoded).unwrap(), replace);
    assert_eq!(&encoded[154..186], &[0x33; 32]);
}

#[test]
fn identity_commands_enforce_their_rfid_context() {
    // PURPOSE: BOUND starts untagged; REPLACED/OBSERVATION reference the active tag.
    // ASSERT: Each inconsistent combination is rejected before reaching the Station.
    // FAILURE MEANS: The Station could sign an identity change against nonexistent state.
    assert!(
        command(EventType::IdentifierBound, ZERO32, 1)
            .validate()
            .is_ok()
    );
    assert!(
        command(EventType::IdentifierBound, [0x33; 32], 1)
            .validate()
            .is_err()
    );
    assert!(
        command(EventType::IdentifierReplaced, ZERO32, 2)
            .validate()
            .is_err()
    );
    assert!(
        command(EventType::ObservationRecorded, ZERO32, 2)
            .validate()
            .is_err()
    );
    assert!(
        command(EventType::CustodyTransferred, [0x33; 32], 2)
            .validate()
            .is_err()
    );
}

#[test]
fn state_version_zero_is_never_a_valid_successor() {
    // PURPOSE: Every Station event advances the asset's state version.
    // ASSERT: version 0 is rejected; a zero predecessor after custody-only versions is allowed.
    // FAILURE MEANS: The Station could sign an event that can never be anchored.
    let mut genesis = command(EventType::IdentifierBound, ZERO32, 1);
    genesis.0.state_version = 0;
    assert!(genesis.validate().is_err());
    // Custody transfers advance state_version without an event hash.
    let mut after_custody = command(EventType::IdentifierBound, ZERO32, 3);
    after_custody.0.previous_event_hash = ZERO32;
    assert!(after_custody.validate().is_ok());
}
