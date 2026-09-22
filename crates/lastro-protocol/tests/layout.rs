//! StationEvent binary layout contracts.

mod common;

use lastro_protocol::{constants::{offset, MAGIC, RESERVED, STATION_EVENT_LEN, VERSION}, StationEvent};

#[test]
fn station_event_is_exactly_276_bytes() {
    // PURPOSE: Freeze the signed wire size.
    // ASSERT: Encoding a valid event produces exactly 276 bytes.
    // FAILURE MEANS: Cross-language offsets or signed bytes diverged.
    assert_eq!(common::event("origin").encode().len(), STATION_EVENT_LEN);
}

#[test]
fn all_offsets_are_contiguous_and_end_at_276() {
    // PURPOSE: Freeze every field boundary.
    // ASSERT: Every documented offset starts exactly after the preceding field and END is 276.
    // FAILURE MEANS: Two implementations may sign different byte ranges.
    assert_eq!(offset::VERSION, offset::MAGIC + 4);
    assert_eq!(offset::ACTION, offset::VERSION + 1);
    assert_eq!(offset::RESERVED, offset::ACTION + 1);
    assert_eq!(offset::DEPLOYMENT_ID, offset::RESERVED + 2);
    assert_eq!(offset::ANIMAL_ID, offset::DEPLOYMENT_ID + 32);
    assert_eq!(offset::STATION_ID, offset::ANIMAL_ID + 32);
    assert_eq!(offset::EVENT_SEQUENCE, offset::STATION_ID + 32);
    assert_eq!(offset::IDENTITY_REVISION, offset::EVENT_SEQUENCE + 8);
    assert_eq!(offset::PREVIOUS_EVENT_HASH, offset::IDENTITY_REVISION + 4);
    assert_eq!(offset::OLD_RFID_HASH, offset::PREVIOUS_EVENT_HASH + 32);
    assert_eq!(offset::NEW_RFID_HASH, offset::OLD_RFID_HASH + 32);
    assert_eq!(offset::FROM_CUSTODIAN, offset::NEW_RFID_HASH + 32);
    assert_eq!(offset::TO_CUSTODIAN, offset::FROM_CUSTODIAN + 32);
    assert_eq!(offset::END, offset::TO_CUSTODIAN + 32);
    assert_eq!(offset::END, STATION_EVENT_LEN);
}

#[test]
fn header_is_lstr_version_one_and_reserved_zero() {
    // PURPOSE: Freeze canonical protocol header bytes.
    // ASSERT: Magic, version, action position, and reserved bytes match the protocol exactly.
    // FAILURE MEANS: Version negotiation or canonical encoding became ambiguous.
    let bytes = common::event("origin").encode();
    assert_eq!(&bytes[0..4], &MAGIC);
    assert_eq!(bytes[4], VERSION);
    assert_eq!(bytes[5], 1);
    assert_eq!(&bytes[6..8], &RESERVED);
}

#[test]
fn integers_are_little_endian() {
    // PURPOSE: Prevent host-endian StationEvent encoding.
    // ASSERT: Asymmetric sequence and revision values appear in little-endian order at frozen offsets.
    // FAILURE MEANS: Wire bytes depend on host architecture.
    let mut event: StationEvent = common::event("transfer");
    event.event_sequence = 0x0102_0304_0506_0708;
    event.identity_revision = 0x1122_3344;
    let bytes = event.encode();
    assert_eq!(&bytes[104..112], &[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]);
    assert_eq!(&bytes[112..116], &[0x44, 0x33, 0x22, 0x11]);
}
