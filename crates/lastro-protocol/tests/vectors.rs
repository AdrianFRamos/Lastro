//! Shared StationEvent fixture contracts.

mod common;

use lastro_protocol::StationEvent;

fn assert_fixture(name: &str) {
    let fixture = common::fixture_json();
    let bytes = common::event_bytes(name);
    let event = StationEvent::decode(&bytes).expect("decode fixture");
    assert_eq!(event.encode(), bytes);
    let expected_hash = common::hex_array::<32>(fixture["events"][name]["event_hash_hex"].as_str().unwrap());
    assert_eq!(event.event_hash(), expected_hash);
}

#[test]
fn origin_fixture_matches_reference() {
    // PURPOSE: Freeze ORIGIN bytes and event hash across implementations.
    // ASSERT: Decoding and re-encoding the committed fixture is byte-exact and hashes to the committed digest.
    // FAILURE MEANS: The protocol changed silently or encoding is not canonical.
    assert_fixture("origin");
}

#[test]
fn transfer_fixture_matches_reference() {
    // PURPOSE: Freeze TRANSFER bytes and event hash across implementations.
    // ASSERT: Decoding and re-encoding the committed fixture is byte-exact and hashes to the committed digest.
    // FAILURE MEANS: Custody transition encoding diverged across stacks.
    assert_fixture("transfer");
}

#[test]
fn reidentify_fixture_matches_reference() {
    // PURPOSE: Freeze REIDENTIFY bytes and event hash across implementations.
    // ASSERT: Decoding and re-encoding the committed fixture is byte-exact and hashes to the committed digest.
    // FAILURE MEANS: RFID replacement encoding diverged across stacks.
    assert_fixture("reidentify");
}
