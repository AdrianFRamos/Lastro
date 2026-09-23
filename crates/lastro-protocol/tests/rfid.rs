//! Canonical RFID contracts.

use lastro_protocol::{
    ProtocolError,
    constants::RFID_DOMAIN,
    rfid::{canonical_rfid_from_slice, canonical_rfid_from_u64, hash_canonical_rfid},
};
use sha2::{Digest, Sha256};

#[test]
fn same_canonical_rfid_hashes_identically() {
    // PURPOSE: Guarantee deterministic RFID hashing.
    // ASSERT: Hashing the same canonical eight bytes twice returns the same digest.
    // FAILURE MEANS: Independent verification cannot reproduce RFID identity.
    let rfid = canonical_rfid_from_u64(0x8000_1300_0000_0001);
    assert_eq!(hash_canonical_rfid(&rfid), hash_canonical_rfid(&rfid));
}

#[test]
fn different_canonical_rfid_values_hash_differently() {
    // PURPOSE: Preserve distinction between different logical RFID values.
    // ASSERT: Two different canonical values produce different hashes.
    // FAILURE MEANS: The canonicalization/hash path collapses distinct identifiers.
    let a = canonical_rfid_from_u64(0x8000_1300_0000_0001);
    let b = canonical_rfid_from_u64(0x8000_1300_0000_0002);
    assert_ne!(hash_canonical_rfid(&a), hash_canonical_rfid(&b));
}

#[test]
fn rfid_hash_uses_exact_domain_separator() {
    // PURPOSE: Freeze RFID hash domain separation.
    // ASSERT: Protocol hashing matches an independent SHA-256 over the exact domain plus eight canonical bytes.
    // FAILURE MEANS: Rust disagrees with firmware, browser, or Solana-facing fixtures.
    let rfid = canonical_rfid_from_u64(0x8000_1300_0000_0001);
    let mut hasher = Sha256::new();
    hasher.update(RFID_DOMAIN);
    hasher.update(rfid);
    let expected: [u8; 32] = hasher.finalize().into();
    assert_eq!(hash_canonical_rfid(&rfid), expected);
}

#[test]
fn formatted_display_text_is_not_silently_canonical() {
    // PURPOSE: Prevent display strings from becoming alternate RFID identities.
    // ASSERT: ASCII text for the same displayed value is rejected by the canonical byte API.
    // FAILURE MEANS: One physical tag can gain multiple hash representations.
    assert_eq!(
        canonical_rfid_from_slice(b"8000130000000001"),
        Err(ProtocolError::InvalidRfidLength)
    );
}

#[test]
fn canonical_rfid_is_exactly_8_bytes() {
    // PURPOSE: Freeze canonical RFID representation to one unsigned 64-bit value.
    // ASSERT: Seven and nine bytes fail while eight bytes succeed.
    // FAILURE MEANS: Reader adapters can hash different widths for the same logical identifier.
    assert_eq!(
        canonical_rfid_from_slice(&[0; 7]),
        Err(ProtocolError::InvalidRfidLength)
    );
    assert!(canonical_rfid_from_slice(&[0; 8]).is_ok());
    assert_eq!(
        canonical_rfid_from_slice(&[0; 9]),
        Err(ProtocolError::InvalidRfidLength)
    );
}
