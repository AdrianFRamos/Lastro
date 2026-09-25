//! Fixed-width, explicit-endian domain event envelope for protocol v2.

use super::{asset::EventType, constants::*};
use crate::error::ProtocolError;

pub const V2_ENVELOPE_LEN: usize = DOMAIN_EVENT_ENVELOPE_LEN;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DomainEventEnvelope {
    pub schema_version: u16,
    pub event_type: u16,
    pub deployment_id: [u8; 32],
    pub subject_id: [u8; 32],
    pub event_id: [u8; 32],
    pub state_version: u64,
    pub expected_previous_hash: [u8; 32],
    pub payload_hash: [u8; 32],
    pub source_id: [u8; 32],
    pub observed_at: i64,
    pub expires_at: i64,
}

impl DomainEventEnvelope {
    pub fn new(
        event_type: EventType,
        deployment_id: [u8; 32],
        subject_id: [u8; 32],
        event_id: [u8; 32],
        state_version: u64,
        expected_previous_hash: [u8; 32],
        payload_hash: [u8; 32],
        source_id: [u8; 32],
        observed_at: i64,
        expires_at: i64,
    ) -> Result<Self, ProtocolError> {
        let envelope = Self {
            schema_version: SCHEMA_VERSION,
            event_type: event_type as u16,
            deployment_id,
            subject_id,
            event_id,
            state_version,
            expected_previous_hash,
            payload_hash,
            source_id,
            observed_at,
            expires_at,
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ProtocolError::UnsupportedV2SchemaVersion);
        }
        EventType::try_from(self.event_type)?;
        if self.deployment_id.iter().all(|byte| *byte == 0)
            || self.subject_id.iter().all(|byte| *byte == 0)
            || self.event_id.iter().all(|byte| *byte == 0)
            || self.source_id.iter().all(|byte| *byte == 0)
        {
            return Err(ProtocolError::InvalidV2Identifier);
        }
        if self.observed_at < 0 || self.expires_at < self.observed_at {
            return Err(ProtocolError::InvalidV2TimeWindow);
        }
        let age = (self.expires_at - self.observed_at) as u64;
        if age > MAX_EVENT_AGE_SECONDS {
            return Err(ProtocolError::InvalidV2TimeWindow);
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<[u8; V2_ENVELOPE_LEN], ProtocolError> {
        self.validate()?;
        let mut out = [0u8; V2_ENVELOPE_LEN];
        out[0..2].copy_from_slice(&self.schema_version.to_le_bytes());
        out[2..4].copy_from_slice(&self.event_type.to_le_bytes());
        out[4..36].copy_from_slice(&self.deployment_id);
        out[36..68].copy_from_slice(&self.subject_id);
        out[68..100].copy_from_slice(&self.event_id);
        out[100..108].copy_from_slice(&self.state_version.to_le_bytes());
        out[108..140].copy_from_slice(&self.expected_previous_hash);
        out[140..172].copy_from_slice(&self.payload_hash);
        out[172..204].copy_from_slice(&self.source_id);
        out[204..212].copy_from_slice(&self.observed_at.to_le_bytes());
        out[212..220].copy_from_slice(&self.expires_at.to_le_bytes());
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() != V2_ENVELOPE_LEN {
            return Err(ProtocolError::InvalidV2EnvelopeLength);
        }
        let envelope = Self {
            schema_version: u16::from_le_bytes(bytes[0..2].try_into().expect("fixed slice")),
            event_type: u16::from_le_bytes(bytes[2..4].try_into().expect("fixed slice")),
            deployment_id: bytes[4..36].try_into().expect("fixed slice"),
            subject_id: bytes[36..68].try_into().expect("fixed slice"),
            event_id: bytes[68..100].try_into().expect("fixed slice"),
            state_version: u64::from_le_bytes(bytes[100..108].try_into().expect("fixed slice")),
            expected_previous_hash: bytes[108..140].try_into().expect("fixed slice"),
            payload_hash: bytes[140..172].try_into().expect("fixed slice"),
            source_id: bytes[172..204].try_into().expect("fixed slice"),
            observed_at: i64::from_le_bytes(bytes[204..212].try_into().expect("fixed slice")),
            expires_at: i64::from_le_bytes(bytes[212..220].try_into().expect("fixed slice")),
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn event_hash(&self) -> Result<[u8; 32], ProtocolError> {
        Ok(crate::v2::hash::event_hash(&self.encode()?))
    }
}
