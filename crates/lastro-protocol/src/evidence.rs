//! EvidencePackage transport model shared by API, fixtures, and independent verifiers.
//!
//! The package carries original evidence only. It deliberately has no backend-generated
//! validity field; every verifier must recompute validity from bytes, signatures, history,
//! and canonical Solana state.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::{
    constants::STATION_EVENT_LEN,
    crypto::derive_station_id,
    error::ProtocolError,
    event::StationEvent,
    rfid::{canonical_rfid_from_slice, hash_canonical_rfid},
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceEvent {
    pub event_bytes_base64: String,
    pub observed_rfid_hex: String,
    pub station_pubkey_hex: String,
    pub station_signature_hex: String,
    pub tx_signature: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidencePackage {
    pub version: u8,
    pub deployment_id: String,
    pub animal_id: String,
    pub events: Vec<EvidenceEvent>,
}

impl EvidencePackage {
    pub fn validate_transport(&self) -> Result<(), ProtocolError> {
        if self.version != 1
            || !is_lower_hex(&self.deployment_id, 32)
            || !is_lower_hex(&self.animal_id, 32)
            || self.events.is_empty()
        {
            return Err(ProtocolError::InvalidEvidencePackage);
        }

        for evidence in &self.events {
            let expected_base64_len = STATION_EVENT_LEN.div_ceil(3) * 4;
            if evidence.event_bytes_base64.len() != expected_base64_len {
                return Err(ProtocolError::InvalidEvidencePackage);
            }
            let event_bytes = STANDARD
                .decode(&evidence.event_bytes_base64)
                .map_err(|_| ProtocolError::InvalidEvidencePackage)?;
            if event_bytes.len() != STATION_EVENT_LEN
                || STANDARD.encode(&event_bytes) != evidence.event_bytes_base64
                || !is_lower_hex(&evidence.observed_rfid_hex, 8)
                || !is_lower_hex(&evidence.station_pubkey_hex, 33)
                || !is_lower_hex(&evidence.station_signature_hex, 64)
                || evidence
                    .tx_signature
                    .as_ref()
                    .is_some_and(|value| !is_solana_signature_text(value))
            {
                return Err(ProtocolError::InvalidEvidencePackage);
            }
        }
        Ok(())
    }

    pub fn validate_off_chain_chain(&self) -> Result<Vec<StationEvent>, ProtocolError> {
        self.validate_transport()?;
        let deployment_id = decode_hex_array::<32>(&self.deployment_id)?;
        let animal_id = decode_hex_array::<32>(&self.animal_id)?;
        let mut decoded = Vec::with_capacity(self.events.len());

        for (index, evidence) in self.events.iter().enumerate() {
            let event_raw = STANDARD
                .decode(&evidence.event_bytes_base64)
                .map_err(|_| ProtocolError::InvalidEvidencePackage)?;
            let event_bytes: [u8; STATION_EVENT_LEN] = event_raw
                .try_into()
                .map_err(|_| ProtocolError::InvalidEvidencePackage)?;
            let event = StationEvent::decode(&event_bytes)?;
            if event.deployment_id != deployment_id || event.animal_id != animal_id {
                return Err(ProtocolError::InvalidEvidencePackage);
            }

            let observed = decode_hex_array::<8>(&evidence.observed_rfid_hex)?;
            let expected_observed_hash =
                hash_canonical_rfid(&canonical_rfid_from_slice(&observed)?);
            if event.new_rfid_hash != expected_observed_hash {
                return Err(ProtocolError::InvalidEvidencePackage);
            }

            let pubkey = decode_hex_array::<33>(&evidence.station_pubkey_hex)?;
            if derive_station_id(&pubkey)? != event.station_id {
                return Err(ProtocolError::InvalidEvidencePackage);
            }
            let signature = decode_hex_array::<64>(&evidence.station_signature_hex)?;
            crate::crypto::verify_station_signature(&event_bytes, &pubkey, &signature)?;

            if index == 0 {
                if event.action != crate::Action::Origin {
                    return Err(ProtocolError::InvalidEvidencePackage);
                }
            } else {
                event.validate_successor(&decoded[index - 1])?;
            }
            decoded.push(event);
        }
        Ok(decoded)
    }
}

fn is_lower_hex(value: &str, bytes: usize) -> bool {
    value.len() == bytes * 2
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_solana_signature_text(value: &str) -> bool {
    (64..=88).contains(&value.len())
        && value.bytes().all(|byte| {
            matches!(byte,
                b'1'..=b'9'
                    | b'A'..=b'H'
                    | b'J'..=b'N'
                    | b'P'..=b'Z'
                    | b'a'..=b'k'
                    | b'm'..=b'z')
        })
}

#[allow(clippy::chunks_exact_to_as_chunks)]
fn decode_hex_array<const N: usize>(value: &str) -> Result<[u8; N], ProtocolError> {
    if !is_lower_hex(value, N) {
        return Err(ProtocolError::InvalidEvidencePackage);
    }
    let mut out = [0u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        out[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Ok(out)
}

fn hex_nibble(value: u8) -> Result<u8, ProtocolError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(ProtocolError::InvalidEvidencePackage),
    }
}
