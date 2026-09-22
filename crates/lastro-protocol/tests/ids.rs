//! Protocol identifier contracts.

mod common;

use lastro_protocol::{crypto::derive_station_id, AnimalId, ProtocolError};

#[test]
fn animal_id_is_exactly_32_bytes() {
    // PURPOSE: Keep persistent AnimalID independent from replaceable physical identifiers.
    // ASSERT: The protocol AnimalId type is exactly 32 bytes; RFID input is not part of this type or derivation API.
    // FAILURE MEANS: Logical identity can become coupled to RFID representation.
    assert_eq!(std::mem::size_of::<AnimalId>(), 32);
}

#[test]
fn station_id_matches_domain_separated_pubkey_hash() {
    // PURPOSE: Bind StationID deterministically to the presented P-256 key.
    // ASSERT: Derivation from the fixture compressed key matches the committed StationID exactly.
    // FAILURE MEANS: Station registration can disagree across layers.
    let fixture = common::fixture_json();
    let pubkey = common::hex_array::<33>(fixture["station"]["pubkey_compressed_hex"].as_str().unwrap());
    let expected = common::hex_array::<32>(fixture["station"]["station_id_hex"].as_str().unwrap());
    assert_eq!(derive_station_id(&pubkey).unwrap(), expected);
}

#[test]
fn invalid_p256_pubkey_length_is_rejected() {
    // PURPOSE: Enforce compressed SEC1 Station public keys only.
    // ASSERT: 32-byte and 65-byte inputs are rejected before StationID derivation.
    // FAILURE MEANS: Station identity hashing can accept ambiguous key encodings.
    assert_eq!(derive_station_id(&[0; 32]), Err(ProtocolError::InvalidStationKey));
    assert_eq!(derive_station_id(&[0; 65]), Err(ProtocolError::InvalidStationKey));
}
