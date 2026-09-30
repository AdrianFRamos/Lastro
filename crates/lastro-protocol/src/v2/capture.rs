//! Physical capture contract shared by API, Agent, simulator and firmware.
//!
//! The API sends a [`CaptureCommand`] through the Agent; the Station reads one RFID and signs
//! the envelope returned by [`capture_envelope`]. The API recomputes the same envelope from
//! its capture record and the observed RFID and accepts only byte-identical evidence.
//! Wire layouts are documented in `docs/MIGRACAO_V1_PARA_V2.md`.

use sha2::{Digest, Sha256};

use super::{DomainEventEnvelope, EventType, V2_ENVELOPE_LEN, identifier::identifier_payload_hash};
use crate::{
    error::ProtocolError,
    rfid::{CanonicalRfid, hash_canonical_rfid},
};

pub const CAPTURE_COMMAND_LEN: usize = 202;
pub const CAPTURE_EVENT_READY_LEN: usize = 341;
const CAPTURE_EVENT_DOMAIN: &[u8] = b"LASTRO_V2_CAPTURE_EVENT\0";
const ZERO32: [u8; 32] = [0; 32];

/// Deterministic event id, so retries of one capture always sign the same envelope.
pub fn capture_event_id(capture_id: &[u8; 16]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CAPTURE_EVENT_DOMAIN);
    hasher.update(capture_id);
    hasher.finalize().into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureCommand {
    pub capture_id: [u8; 16],
    pub event_type: EventType,
    pub deployment_id: [u8; 32],
    pub asset_id: [u8; 32],
    pub event_id: [u8; 32],
    pub state_version: u64,
    pub previous_event_hash: [u8; 32],
    pub expected_rfid_hash: [u8; 32],
    pub observed_at: i64,
    pub expires_at: i64,
}

impl CaptureCommand {
    /// Context checks possible before the RFID is read.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        let expects_tag = self.expected_rfid_hash != ZERO32;
        let type_ok = match self.event_type {
            EventType::IdentifierBound => !expects_tag,
            EventType::IdentifierReplaced | EventType::ObservationRecorded => expects_tag,
            _ => false,
        };
        if !type_ok
            || self.capture_id == [0; 16]
            || self.event_id != capture_event_id(&self.capture_id)
            // The predecessor is not tied to the version: custody transfers advance
            // state_version without an event, so version > 1 may still follow a zero hash.
            // The program enforces the exact (version, predecessor) against AssetState.
            || self.state_version == 0
        {
            return Err(ProtocolError::InvalidSemantics);
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<[u8; CAPTURE_COMMAND_LEN], ProtocolError> {
        self.validate()?;
        let mut out = [0u8; CAPTURE_COMMAND_LEN];
        out[0..16].copy_from_slice(&self.capture_id);
        out[16..18].copy_from_slice(&(self.event_type as u16).to_le_bytes());
        out[18..50].copy_from_slice(&self.deployment_id);
        out[50..82].copy_from_slice(&self.asset_id);
        out[82..114].copy_from_slice(&self.event_id);
        out[114..122].copy_from_slice(&self.state_version.to_le_bytes());
        out[122..154].copy_from_slice(&self.previous_event_hash);
        out[154..186].copy_from_slice(&self.expected_rfid_hash);
        out[186..194].copy_from_slice(&self.observed_at.to_le_bytes());
        out[194..202].copy_from_slice(&self.expires_at.to_le_bytes());
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() != CAPTURE_COMMAND_LEN {
            return Err(ProtocolError::InvalidLength);
        }
        let array32 = |range: std::ops::Range<usize>| -> [u8; 32] {
            bytes[range].try_into().expect("fixed range")
        };
        let command = Self {
            capture_id: bytes[0..16].try_into().expect("fixed range"),
            event_type: EventType::try_from(u16::from_le_bytes([bytes[16], bytes[17]]))?,
            deployment_id: array32(18..50),
            asset_id: array32(50..82),
            event_id: array32(82..114),
            state_version: u64::from_le_bytes(bytes[114..122].try_into().expect("fixed range")),
            previous_event_hash: array32(122..154),
            expected_rfid_hash: array32(154..186),
            observed_at: i64::from_le_bytes(bytes[186..194].try_into().expect("fixed range")),
            expires_at: i64::from_le_bytes(bytes[194..202].try_into().expect("fixed range")),
        };
        command.validate()?;
        Ok(command)
    }
}

/// Station rule: bind a tag to an untagged asset, replace the active tag with a different
/// one, or prove the active tag is present. Returns the envelope the Station must sign.
pub fn capture_envelope(
    command: &CaptureCommand,
    observed_rfid: &CanonicalRfid,
    station_id: [u8; 32],
) -> Result<DomainEventEnvelope, ProtocolError> {
    command.validate()?;
    let observed = hash_canonical_rfid(observed_rfid);
    let expected = command.expected_rfid_hash;
    let (old, new) = match command.event_type {
        EventType::IdentifierBound => (ZERO32, observed),
        EventType::IdentifierReplaced if observed != expected => (expected, observed),
        EventType::ObservationRecorded if observed == expected => (observed, observed),
        _ => return Err(ProtocolError::InvalidSuccessor),
    };
    DomainEventEnvelope::new(
        command.event_type,
        command.deployment_id,
        command.asset_id,
        command.event_id,
        command.state_version,
        command.previous_event_hash,
        identifier_payload_hash(old, new),
        station_id,
        command.observed_at,
        command.expires_at,
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureEventReady {
    pub capture_id: [u8; 16],
    pub envelope: [u8; V2_ENVELOPE_LEN],
    pub observed_rfid: CanonicalRfid,
    pub station_pubkey33: [u8; 33],
    pub station_signature64: [u8; 64],
}

impl CaptureEventReady {
    pub fn encode(&self) -> [u8; CAPTURE_EVENT_READY_LEN] {
        let mut out = [0u8; CAPTURE_EVENT_READY_LEN];
        out[0..16].copy_from_slice(&self.capture_id);
        out[16..236].copy_from_slice(&self.envelope);
        out[236..244].copy_from_slice(&self.observed_rfid);
        out[244..277].copy_from_slice(&self.station_pubkey33);
        out[277..341].copy_from_slice(&self.station_signature64);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() != CAPTURE_EVENT_READY_LEN {
            return Err(ProtocolError::InvalidLength);
        }
        Ok(Self {
            capture_id: bytes[0..16].try_into().expect("fixed range"),
            envelope: bytes[16..236].try_into().expect("fixed range"),
            observed_rfid: bytes[236..244].try_into().expect("fixed range"),
            station_pubkey33: bytes[244..277].try_into().expect("fixed range"),
            station_signature64: bytes[277..341].try_into().expect("fixed range"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rfid::canonical_rfid_from_u64;

    fn command(event_type: EventType, expected: [u8; 32], version: u64) -> CaptureCommand {
        let capture_id = [7; 16];
        CaptureCommand {
            capture_id,
            event_type,
            deployment_id: [1; 32],
            asset_id: [2; 32],
            event_id: capture_event_id(&capture_id),
            state_version: version,
            previous_event_hash: if version == 1 { ZERO32 } else { [3; 32] },
            expected_rfid_hash: expected,
            observed_at: 1_000,
            expires_at: 1_300,
        }
    }

    #[test]
    fn command_round_trips_and_rejects_inconsistent_context() {
        let bound = command(EventType::IdentifierBound, ZERO32, 1);
        assert_eq!(
            CaptureCommand::decode(&bound.encode().unwrap()).unwrap(),
            bound
        );
        assert!(
            command(EventType::IdentifierBound, [9; 32], 1)
                .encode()
                .is_err()
        );
        assert!(
            command(EventType::IdentifierReplaced, ZERO32, 2)
                .encode()
                .is_err()
        );
        assert!(
            command(EventType::CustodyTransferred, [9; 32], 2)
                .encode()
                .is_err()
        );
        let mut wrong_id = bound;
        wrong_id.event_id = [5; 32];
        assert!(wrong_id.encode().is_err());
    }

    #[test]
    fn station_rules_bind_replace_and_observe() {
        let tag = canonical_rfid_from_u64(42);
        let tag_hash = hash_canonical_rfid(&tag);
        let other = canonical_rfid_from_u64(43);

        let bound = capture_envelope(
            &command(EventType::IdentifierBound, ZERO32, 1),
            &tag,
            [4; 32],
        )
        .unwrap();
        assert_eq!(
            bound.payload_hash,
            identifier_payload_hash(ZERO32, tag_hash)
        );

        let replace = command(EventType::IdentifierReplaced, tag_hash, 2);
        assert!(capture_envelope(&replace, &tag, [4; 32]).is_err());
        let replaced = capture_envelope(&replace, &other, [4; 32]).unwrap();
        assert_eq!(
            replaced.payload_hash,
            identifier_payload_hash(tag_hash, hash_canonical_rfid(&other))
        );

        let observe = command(EventType::ObservationRecorded, tag_hash, 3);
        assert!(capture_envelope(&observe, &other, [4; 32]).is_err());
        assert!(capture_envelope(&observe, &tag, [4; 32]).is_ok());
    }

    #[test]
    fn event_ready_round_trips() {
        let ready = CaptureEventReady {
            capture_id: [1; 16],
            envelope: [2; V2_ENVELOPE_LEN],
            observed_rfid: [3; 8],
            station_pubkey33: [4; 33],
            station_signature64: [5; 64],
        };
        assert_eq!(CaptureEventReady::decode(&ready.encode()).unwrap(), ready);
        assert!(CaptureEventReady::decode(&[0; 340]).is_err());
    }
}
