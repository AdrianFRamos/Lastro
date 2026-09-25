//! Domain v2 admission rules shared by authenticated Agent routes.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use lastro_protocol::{
    crypto::{derive_station_id, verify_station_signature_bytes},
    v2::{DomainEventEnvelope, EventType, V2_ENVELOPE_LEN},
};
use time::OffsetDateTime;

use crate::{error::ApiError, model::DomainObservationRequest};

#[derive(Clone, Debug)]
pub struct VerifiedDomainObservation {
    pub envelope: DomainEventEnvelope,
    pub envelope_bytes: [u8; V2_ENVELOPE_LEN],
    pub station_pubkey33: [u8; 33],
    pub station_signature64: [u8; 64],
    pub event_hash: [u8; 32],
}

pub fn verify_observation(
    request: &DomainObservationRequest,
    deployment_id: [u8; 32],
    expected_station_pubkey33: &[u8; 33],
) -> Result<VerifiedDomainObservation, ApiError> {
    let envelope_bytes = decode_base64_array::<V2_ENVELOPE_LEN>(
        "envelopeBytesBase64",
        &request.envelope_bytes_base64,
    )?;
    let station_pubkey33 = decode_hex_array::<33>("stationPubkeyHex", &request.station_pubkey_hex)?;
    let station_signature64 =
        decode_hex_array::<64>("stationSignatureHex", &request.station_signature_hex)?;
    let envelope = DomainEventEnvelope::decode(&envelope_bytes)
        .map_err(|error| ApiError::Validation(format!("invalid v2 domain envelope: {error}")))?;

    if envelope.deployment_id != deployment_id {
        return Err(ApiError::Conflict(
            "v2 envelope belongs to a different deployment".into(),
        ));
    }
    if envelope.event_type != EventType::ObservationRecorded as u16 {
        return Err(ApiError::Validation(
            "observation endpoint accepts only ObservationRecorded events".into(),
        ));
    }
    if &station_pubkey33 != expected_station_pubkey33 {
        return Err(ApiError::Conflict(
            "submitted Station public key is not registered for this deployment".into(),
        ));
    }
    let derived_station_id = derive_station_id(&station_pubkey33)
        .map_err(|error| ApiError::Validation(format!("invalid Station public key: {error}")))?;
    if envelope.source_id != derived_station_id {
        return Err(ApiError::Validation(
            "v2 envelope sourceId does not match the Station public key".into(),
        ));
    }

    let now = OffsetDateTime::now_utc().unix_timestamp();
    if now < envelope.observed_at || now > envelope.expires_at {
        return Err(ApiError::Conflict(
            "v2 envelope is outside its validity window".into(),
        ));
    }
    verify_station_signature_bytes(&envelope_bytes, &station_pubkey33, &station_signature64)
        .map_err(|error| ApiError::Validation(format!("invalid v2 Station signature: {error}")))?;
    let event_hash = envelope
        .event_hash()
        .map_err(|error| ApiError::Validation(format!("cannot hash v2 envelope: {error}")))?;

    Ok(VerifiedDomainObservation {
        envelope,
        envelope_bytes,
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

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
    use lastro_protocol::v2::{EventType, constants::MAX_EVENT_AGE_SECONDS};
    use p256::ecdsa::{SigningKey, signature::Signer};

    #[test]
    fn accepts_a_valid_signed_observation_envelope() {
        let signing_key = SigningKey::from_bytes((&[1u8; 32]).into()).expect("fixed key");
        let station_pubkey33: [u8; 33] = signing_key
            .verifying_key()
            .to_encoded_point(true)
            .as_bytes()
            .try_into()
            .expect("compressed P-256 key");
        let deployment_id = [0x11; 32];
        let source_id = derive_station_id(&station_pubkey33).expect("station id");
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let envelope = DomainEventEnvelope::new(
            EventType::ObservationRecorded,
            deployment_id,
            [0x22; 32],
            [0x33; 32],
            1,
            [0; 32],
            [0x44; 32],
            source_id,
            now - 1,
            now + i64::try_from(MAX_EVENT_AGE_SECONDS).expect("age") - 1,
        )
        .expect("valid envelope");
        let bytes = envelope.encode().expect("encoded envelope");
        let signature: p256::ecdsa::Signature = signing_key.sign(&bytes);
        let signature = signature.normalize_s().unwrap_or(signature);
        let request = DomainObservationRequest {
            envelope_bytes_base64: BASE64.encode(bytes),
            station_pubkey_hex: hex::encode(station_pubkey33),
            station_signature_hex: hex::encode(signature.to_bytes()),
        };

        let verified = verify_observation(&request, deployment_id, &station_pubkey33)
            .expect("valid observation must be admitted");
        assert_eq!(verified.envelope, envelope);
        assert_eq!(verified.event_hash, envelope.event_hash().expect("hash"));
    }

    #[test]
    fn rejects_an_observation_signed_by_a_different_station_key() {
        let request = DomainObservationRequest {
            envelope_bytes_base64: BASE64.encode([0u8; V2_ENVELOPE_LEN]),
            station_pubkey_hex: "02".to_owned() + &"00".repeat(32),
            station_signature_hex: "00".repeat(64),
        };
        assert!(verify_observation(&request, [0x11; 32], &[0x03; 33]).is_err());
    }
}
