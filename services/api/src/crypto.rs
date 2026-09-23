//! Cryptographic verification at the evidence-ingest boundary.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use lastro_protocol::{
    crypto::{derive_station_id, verify_station_signature},
    event::StationEvent,
    rfid::hash_canonical_rfid,
};
use sha2::{Digest, Sha256};

use crate::{error::ApiError, model::AgentEvidenceRequest};

#[derive(Clone, Debug)]
pub struct VerifiedEvidence {
    pub event: StationEvent,
    pub event_bytes: [u8; 276],
    pub observed_rfid: [u8; 8],
    pub station_pubkey33: [u8; 33],
    pub station_signature64: [u8; 64],
    pub event_hash: [u8; 32],
}

pub fn verify_agent_evidence(
    request: &AgentEvidenceRequest,
    registered_station_pubkey33: &[u8; 33],
) -> Result<VerifiedEvidence, ApiError> {
    let evidence = verify_agent_evidence_locally(request)?;
    if &evidence.station_pubkey33 != registered_station_pubkey33 {
        return Err(ApiError::Conflict(
            "submitted Station public key is not registered for this deployment".into(),
        ));
    }
    Ok(evidence)
}

/// Validate all self-contained evidence bytes before database or RPC work.
/// Canonical Station registration is checked separately against ProtocolConfig.
pub fn verify_agent_evidence_locally(
    request: &AgentEvidenceRequest,
) -> Result<VerifiedEvidence, ApiError> {
    let event_bytes = decode_base64_array::<276>("eventBytesBase64", &request.event_bytes_base64)?;
    let observed_rfid = decode_hex_array::<8>("observedRfidHex", &request.observed_rfid_hex)?;
    let station_pubkey33 = decode_hex_array::<33>("stationPubkeyHex", &request.station_pubkey_hex)?;
    let station_signature64 =
        decode_hex_array::<64>("stationSignatureHex", &request.station_signature_hex)?;

    let event = StationEvent::decode(&event_bytes)
        .map_err(|error| ApiError::Validation(format!("invalid StationEvent: {error}")))?;
    if hash_canonical_rfid(&observed_rfid) != event.new_rfid_hash {
        return Err(ApiError::Validation(
            "observed RFID does not match StationEvent new_rfid_hash".into(),
        ));
    }
    let derived_station_id = derive_station_id(&station_pubkey33)
        .map_err(|error| ApiError::Validation(format!("invalid Station public key: {error}")))?;
    if derived_station_id != event.station_id {
        return Err(ApiError::Validation(
            "StationEvent station_id does not match submitted public key".into(),
        ));
    }
    verify_station_signature(&event_bytes, &station_pubkey33, &station_signature64)
        .map_err(|error| ApiError::Validation(format!("invalid Station signature: {error}")))?;

    let event_hash: [u8; 32] = Sha256::digest(event_bytes).into();
    Ok(VerifiedEvidence {
        event,
        event_bytes,
        observed_rfid,
        station_pubkey33,
        station_signature64,
        event_hash,
    })
}

fn decode_hex_array<const N: usize>(name: &str, value: &str) -> Result<[u8; N], ApiError> {
    if value.len() != N * 2 || value != value.to_ascii_lowercase() {
        return Err(ApiError::Validation(format!(
            "{name} must be exactly {N} bytes of lowercase hex"
        )));
    }
    let bytes = hex::decode(value)
        .map_err(|_| ApiError::Validation(format!("{name} must be valid lowercase hex")))?;
    bytes
        .try_into()
        .map_err(|_| ApiError::Validation(format!("{name} has invalid length")))
}

fn decode_base64_array<const N: usize>(name: &str, value: &str) -> Result<[u8; N], ApiError> {
    let encoded_len = N.div_ceil(3) * 4;
    if value.len() != encoded_len {
        return Err(ApiError::Validation(format!(
            "{name} must be canonical base64 for exactly {N} bytes"
        )));
    }
    let bytes = BASE64
        .decode(value)
        .map_err(|_| ApiError::Validation(format!("{name} must be canonical base64")))?;
    if BASE64.encode(&bytes) != value {
        return Err(ApiError::Validation(format!(
            "{name} must be canonical base64"
        )));
    }
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        ApiError::Validation(format!(
            "{name} must decode to {N} bytes, got {}",
            bytes.len()
        ))
    })
}
