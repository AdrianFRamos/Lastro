//! P-256 Station signature contracts.

mod common;

use lastro_protocol::{crypto::verify_station_signature, ProtocolError};

#[test]
fn valid_compact_low_s_signature_verifies_raw_event() {
    // PURPOSE: Freeze Station/host signature interoperability.
    // ASSERT: The committed compact low-S signature verifies the exact raw 276-byte ORIGIN event.
    // FAILURE MEANS: Station, host, verifier, and Solana signature semantics differ.
    let fixture = common::fixture_json();
    let event = common::event_bytes("origin");
    let pubkey = common::hex_array::<33>(fixture["station"]["pubkey_compressed_hex"].as_str().unwrap());
    let signature = common::hex_array::<64>(fixture["events"]["origin"]["station_signature_hex"].as_str().unwrap());
    assert!(verify_station_signature(&event, &pubkey, &signature).is_ok());
}

#[test]
fn one_byte_event_change_breaks_signature() {
    // PURPOSE: Bind the Station signature to every event byte.
    // ASSERT: Flipping one event byte makes signature verification fail.
    // FAILURE MEANS: Signed evidence can be modified without detection.
    let fixture = common::fixture_json();
    let mut event = common::event_bytes("origin");
    let pubkey = common::hex_array::<33>(fixture["station"]["pubkey_compressed_hex"].as_str().unwrap());
    let signature = common::hex_array::<64>(fixture["events"]["origin"]["station_signature_hex"].as_str().unwrap());
    event[200] ^= 1;
    assert_eq!(verify_station_signature(&event, &pubkey, &signature), Err(ProtocolError::InvalidStationSignature));
}

#[test]
fn wrong_station_key_breaks_signature() {
    // PURPOSE: Bind evidence to the registered Station key.
    // ASSERT: A different valid compressed P-256 public key cannot verify the fixture signature.
    // FAILURE MEANS: Station identity can be substituted after capture.
    let fixture = common::fixture_json();
    let event = common::event_bytes("origin");
    let signature = common::hex_array::<64>(fixture["events"]["origin"]["station_signature_hex"].as_str().unwrap());
    let wrong_pubkey = common::hex_array::<33>("037cf27b188d034f7e8a52380304b51ac3c08969e277f21b35a60b48fc47669978");
    assert_eq!(verify_station_signature(&event, &wrong_pubkey, &signature), Err(ProtocolError::InvalidStationSignature));
}

#[test]
fn high_s_signature_is_rejected() {
    // PURPOSE: Match Solana's canonical low-S acceptance rule.
    // ASSERT: The mathematically equivalent high-S form is rejected before signature acceptance.
    // FAILURE MEANS: Off-chain verification could accept evidence the chain rejects.
    let fixture = common::fixture_json();
    let event = common::event_bytes("origin");
    let pubkey = common::hex_array::<33>(fixture["station"]["pubkey_compressed_hex"].as_str().unwrap());
    let mut signature = common::hex_array::<64>(fixture["events"]["origin"]["station_signature_hex"].as_str().unwrap());
    let low_s: [u8; 32] = signature[32..].try_into().unwrap();
    signature[32..].copy_from_slice(&common::p256_order_minus(&low_s));
    assert_eq!(verify_station_signature(&event, &pubkey, &signature), Err(ProtocolError::HighSSignature));
}
