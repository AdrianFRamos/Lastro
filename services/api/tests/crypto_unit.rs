//! Evidence-ingest cryptographic unit contracts that do not require PostgreSQL or RPC.

use std::{fs, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use lastro_api::{crypto::verify_agent_evidence, model::AgentEvidenceRequest};
use uuid::Uuid;

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name)).expect("read fixture")
}

fn valid_request() -> AgentEvidenceRequest {
    AgentEvidenceRequest {
        capture_id: Uuid::parse_str("00112233-4455-6677-8899-aabbccddeeff").unwrap(),
        event_bytes_base64: BASE64.encode(fixture("origin.bin")),
        observed_rfid_hex: "8000130000000001".into(),
        station_pubkey_hex: "036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296".into(),
        station_signature_hex: "09579de39fbd8108dea2fc89262240c45b2043d85e6e16854d9bed737e6426e109be548b3eb9d3ba3252aeba61af230a18115aeaf0b74734df88c90955f9184f".into(),
    }
}

#[test]
fn valid_origin_evidence_preserves_exact_station_bytes() {
    // PURPOSE: Accept a frozen Station fixture without reserializing its signed 276-byte message.
    // ASSERT: Verification succeeds and returned event bytes/hash/key/signature remain exact.
    // FAILURE MEANS: API ingest is incompatible with the shared Station evidence contract.
    let key: [u8; 33] = hex::decode(&valid_request().station_pubkey_hex)
        .unwrap()
        .try_into()
        .unwrap();
    let verified = verify_agent_evidence(&valid_request(), &key).unwrap();
    assert_eq!(verified.event_bytes.as_slice(), fixture("origin.bin"));
    assert_eq!(
        hex::encode(verified.event_hash),
        "5845dc20fd6b266ec98399f0aa93c736ec9aa778bf038af5291e5df81334b531"
    );
}

#[test]
fn tampered_signature_or_observed_rfid_is_rejected() {
    // PURPOSE: Bind accepted evidence to both the P-256 signature and physical RFID bytes.
    // ASSERT: One-bit signature tampering and a different observed RFID each fail verification.
    // FAILURE MEANS: API could persist unauthenticated or physically inconsistent evidence.
    let key: [u8; 33] = hex::decode(&valid_request().station_pubkey_hex)
        .unwrap()
        .try_into()
        .unwrap();
    let mut signature = valid_request();
    signature.station_signature_hex.replace_range(0..2, "08");
    assert!(verify_agent_evidence(&signature, &key).is_err());
    let mut rfid = valid_request();
    rfid.observed_rfid_hex = "8000130000000002".into();
    assert!(verify_agent_evidence(&rfid, &key).is_err());
}

#[test]
fn unregistered_station_key_is_rejected() {
    // PURPOSE: A mathematically valid Station signature is insufficient unless its key is deployment-authorized.
    // ASSERT: Verification rejects the valid fixture when the registered key differs.
    // FAILURE MEANS: Any P-256 device could inject evidence into the deployment.
    let other: [u8; 33] =
        hex::decode("037cf27b188d034f7e8a52380304b51ac3c08969e277f21b35a60b48fc47669978")
            .unwrap()
            .try_into()
            .unwrap();
    assert!(verify_agent_evidence(&valid_request(), &other).is_err());
}
