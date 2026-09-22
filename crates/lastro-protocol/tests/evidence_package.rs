//! EvidencePackage transport and independent off-chain validation contracts.

use std::{fs, path::PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use lastro_protocol::{EvidencePackage, ProtocolError};
use serde_json::Value;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test-vectors").join(name)
}

fn valid_package() -> EvidencePackage {
    serde_json::from_slice(&fs::read(fixture_path("evidence-package.valid.json")).unwrap()).unwrap()
}

#[test]
fn package_preserves_event_order() {
    // PURPOSE: Preserve exact history order through JSON transport.
    // ASSERT: Serialize/deserialize keeps all three event payloads in the same order and the chain validates.
    // FAILURE MEANS: Predecessor-chain verification can operate on reordered evidence.
    let package = valid_package();
    let first = package.events[0].event_bytes_base64.clone();
    let encoded = serde_json::to_vec(&package).unwrap();
    let decoded: EvidencePackage = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.events[0].event_bytes_base64, first);
    assert_eq!(decoded, package);
    assert_eq!(decoded.validate_off_chain_chain().unwrap().len(), package.events.len());
}

#[test]
fn package_rejects_wrong_event_length() {
    // PURPOSE: Enforce strict raw StationEvent transport.
    // ASSERT: Both 275-byte and 277-byte base64 event payloads fail validation.
    // FAILURE MEANS: Ambiguous or trailing event bytes can reach verification.
    for length in [275usize, 277] {
        let mut package = valid_package();
        package.events[0].event_bytes_base64 = STANDARD.encode(vec![0u8; length]);
        assert_eq!(package.validate_transport(), Err(ProtocolError::InvalidEvidencePackage));
    }
}

#[test]
fn package_rejects_wrong_key_signature_lengths() {
    // PURPOSE: Freeze cryptographic transport shapes.
    // ASSERT: Wrong compressed-key or compact-signature lengths fail transport validation.
    // FAILURE MEANS: Crypto parsing ambiguity can reach independent verification.
    let mut key = valid_package();
    key.events[0].station_pubkey_hex = "00".repeat(32);
    assert_eq!(key.validate_transport(), Err(ProtocolError::InvalidEvidencePackage));
    let mut signature = valid_package();
    signature.events[0].station_signature_hex = "00".repeat(63);
    assert_eq!(signature.validate_transport(), Err(ProtocolError::InvalidEvidencePackage));
}

#[test]
fn package_does_not_trust_valid_flag() {
    // PURPOSE: Keep verifier verdicts independent from the backend.
    // ASSERT: An undeclared top-level valid=true field is rejected by strict deserialization.
    // FAILURE MEANS: A backend-supplied verdict can enter a path that must recompute validity.
    let mut value: Value = serde_json::from_slice(&fs::read(fixture_path("evidence-package.valid.json")).unwrap()).unwrap();
    value.as_object_mut().unwrap().insert("valid".into(), Value::Bool(true));
    assert!(serde_json::from_value::<EvidencePackage>(value).is_err());
}

#[test]
fn package_detects_one_byte_evidence_tampering() {
    // PURPOSE: Prove the independent evidence path detects modified signed bytes.
    // ASSERT: The committed tampered package is structurally valid but fails off-chain chain verification.
    // FAILURE MEANS: Evidence mutation can survive package verification.
    let tampered: EvidencePackage = serde_json::from_slice(&fs::read(fixture_path("evidence-package.tampered.json")).unwrap()).unwrap();
    assert!(tampered.validate_transport().is_ok());
    assert!(tampered.validate_off_chain_chain().is_err());
}

#[test]
fn package_rejects_noncanonical_signature_text() {
    // PURPOSE: Reject malformed transaction references at transport parsing without imposing an arbitrary history cap.
    let mut bad_signature = valid_package();
    bad_signature.events[0].tx_signature = Some("not-base58".into());
    assert_eq!(bad_signature.validate_transport(), Err(ProtocolError::InvalidEvidencePackage));

    let mut oversized_signature = valid_package();
    oversized_signature.events[0].tx_signature = Some("1".repeat(89));
    assert_eq!(oversized_signature.validate_transport(), Err(ProtocolError::InvalidEvidencePackage));

    // ASSERT: malformed/oversized Solana signature text fails before cryptographic history work.
    // FAILURE MEANS: hostile EvidencePackage input can carry ambiguous transaction identifiers.
}
