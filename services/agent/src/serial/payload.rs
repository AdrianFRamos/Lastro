//! Fixed binary payloads carried inside the Lastro serial frame.
//!
//! This module is a wire contract, not a serde contract. Every multi-byte integer is
//! little-endian. UUID uses the canonical 16 bytes returned by `Uuid::as_bytes()`.
//! Do not replace these encoders with bincode/JSON/CBOR: firmware C and Rust must emit
//! byte-identical payloads and cross-language fixtures gate that invariant.

use uuid::Uuid;

use crate::{command::StationCommand, error::AgentError};

pub const COMMAND_PAYLOAD_LEN: usize = 224;
pub const EVENT_READY_PAYLOAD_LEN: usize = 397;
pub const ACK_PAYLOAD_LEN: usize = 48;
pub const ERROR_PAYLOAD_LEN: usize = 20;
pub const DOMAIN_EVENT_READY_PAYLOAD_LEN: usize = 317;
pub const DOMAIN_ACK_PAYLOAD_LEN: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventReadyPayload {
    pub capture_id: Uuid,
    pub event_bytes: [u8; 276],
    pub observed_rfid: [u8; 8],
    pub station_pubkey33: [u8; 33],
    pub station_signature64: [u8; 64],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainEventReadyPayload {
    pub envelope_bytes: [u8; 220],
    pub station_pubkey33: [u8; 33],
    pub station_signature64: [u8; 64],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DomainAckPayload {
    pub event_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AckPayload {
    pub capture_id: Uuid,
    pub event_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum StationErrorCode {
    InvalidCommand = 1,
    RfidReadFailed = 2,
    InvalidEventContext = 3,
    SigningFailed = 4,
    Busy = 5,
    RfidTimeout = 6,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorPayload {
    /// Zero UUID is allowed only when an error happens before a capture id can be accepted.
    pub capture_id: Uuid,
    pub code: StationErrorCode,
}

pub fn encode_command(command: &StationCommand) -> Result<[u8; COMMAND_PAYLOAD_LEN], AgentError> {
    command.validate()?;
    let mut out = [0u8; COMMAND_PAYLOAD_LEN];
    out[0..16].copy_from_slice(command.capture_id.as_bytes());
    out[16] = command.action;
    // 17..20 reserved = zero
    out[20..52].copy_from_slice(&command.deployment_id);
    out[52..84].copy_from_slice(&command.animal_id);
    out[84..92].copy_from_slice(&command.event_sequence.to_le_bytes());
    out[92..96].copy_from_slice(&command.identity_revision.to_le_bytes());
    out[96..128].copy_from_slice(&command.previous_event_hash);
    out[128..160].copy_from_slice(&command.expected_old_rfid_hash);
    out[160..192].copy_from_slice(&command.from_custodian);
    out[192..224].copy_from_slice(&command.to_custodian);
    Ok(out)
}

pub fn decode_command(bytes: &[u8]) -> Result<StationCommand, AgentError> {
    if bytes.len() != COMMAND_PAYLOAD_LEN {
        return Err(AgentError::Contract(format!(
            "COMMAND payload must be {COMMAND_PAYLOAD_LEN} bytes"
        )));
    }
    if bytes[17..20] != [0, 0, 0] {
        return Err(AgentError::Contract(
            "COMMAND reserved bytes must be zero".into(),
        ));
    }
    let command = StationCommand {
        capture_id: Uuid::from_bytes(bytes[0..16].try_into().expect("fixed slice")),
        action: bytes[16],
        deployment_id: bytes[20..52].try_into().expect("fixed slice"),
        animal_id: bytes[52..84].try_into().expect("fixed slice"),
        event_sequence: u64::from_le_bytes(bytes[84..92].try_into().expect("fixed slice")),
        identity_revision: u32::from_le_bytes(bytes[92..96].try_into().expect("fixed slice")),
        previous_event_hash: bytes[96..128].try_into().expect("fixed slice"),
        expected_old_rfid_hash: bytes[128..160].try_into().expect("fixed slice"),
        from_custodian: bytes[160..192].try_into().expect("fixed slice"),
        to_custodian: bytes[192..224].try_into().expect("fixed slice"),
    };
    command.validate()?;
    Ok(command)
}

pub fn encode_event_ready(payload: &EventReadyPayload) -> [u8; EVENT_READY_PAYLOAD_LEN] {
    let mut out = [0u8; EVENT_READY_PAYLOAD_LEN];
    out[0..16].copy_from_slice(payload.capture_id.as_bytes());
    out[16..292].copy_from_slice(&payload.event_bytes);
    out[292..300].copy_from_slice(&payload.observed_rfid);
    out[300..333].copy_from_slice(&payload.station_pubkey33);
    out[333..397].copy_from_slice(&payload.station_signature64);
    out
}

pub fn decode_event_ready(bytes: &[u8]) -> Result<EventReadyPayload, AgentError> {
    if bytes.len() != EVENT_READY_PAYLOAD_LEN {
        return Err(AgentError::Contract(format!(
            "EVENT_READY payload must be {EVENT_READY_PAYLOAD_LEN} bytes"
        )));
    }
    Ok(EventReadyPayload {
        capture_id: Uuid::from_bytes(bytes[0..16].try_into().expect("fixed slice")),
        event_bytes: bytes[16..292].try_into().expect("fixed slice"),
        observed_rfid: bytes[292..300].try_into().expect("fixed slice"),
        station_pubkey33: bytes[300..333].try_into().expect("fixed slice"),
        station_signature64: bytes[333..397].try_into().expect("fixed slice"),
    })
}

pub fn encode_domain_event_ready(
    payload: &DomainEventReadyPayload,
) -> [u8; DOMAIN_EVENT_READY_PAYLOAD_LEN] {
    let mut out = [0u8; DOMAIN_EVENT_READY_PAYLOAD_LEN];
    out[0..220].copy_from_slice(&payload.envelope_bytes);
    out[220..253].copy_from_slice(&payload.station_pubkey33);
    out[253..317].copy_from_slice(&payload.station_signature64);
    out
}

pub fn decode_domain_event_ready(bytes: &[u8]) -> Result<DomainEventReadyPayload, AgentError> {
    if bytes.len() != DOMAIN_EVENT_READY_PAYLOAD_LEN {
        return Err(AgentError::Contract(format!(
            "DOMAIN_EVENT_READY payload must be {DOMAIN_EVENT_READY_PAYLOAD_LEN} bytes"
        )));
    }
    Ok(DomainEventReadyPayload {
        envelope_bytes: bytes[0..220].try_into().expect("fixed slice"),
        station_pubkey33: bytes[220..253].try_into().expect("fixed slice"),
        station_signature64: bytes[253..317].try_into().expect("fixed slice"),
    })
}

pub fn encode_ack(payload: &AckPayload) -> [u8; ACK_PAYLOAD_LEN] {
    let mut out = [0u8; ACK_PAYLOAD_LEN];
    out[0..16].copy_from_slice(payload.capture_id.as_bytes());
    out[16..48].copy_from_slice(&payload.event_hash);
    out
}

pub fn decode_ack(bytes: &[u8]) -> Result<AckPayload, AgentError> {
    if bytes.len() != ACK_PAYLOAD_LEN {
        return Err(AgentError::Contract(format!(
            "ACK payload must be {ACK_PAYLOAD_LEN} bytes"
        )));
    }
    Ok(AckPayload {
        capture_id: Uuid::from_bytes(bytes[0..16].try_into().expect("fixed slice")),
        event_hash: bytes[16..48].try_into().expect("fixed slice"),
    })
}

pub fn encode_domain_ack(payload: &DomainAckPayload) -> [u8; DOMAIN_ACK_PAYLOAD_LEN] {
    payload.event_hash
}

pub fn decode_domain_ack(bytes: &[u8]) -> Result<DomainAckPayload, AgentError> {
    if bytes.len() != DOMAIN_ACK_PAYLOAD_LEN {
        return Err(AgentError::Contract(format!(
            "DOMAIN_ACK payload must be {DOMAIN_ACK_PAYLOAD_LEN} bytes"
        )));
    }
    Ok(DomainAckPayload {
        event_hash: bytes.try_into().expect("fixed slice"),
    })
}

pub fn encode_error(payload: &ErrorPayload) -> [u8; ERROR_PAYLOAD_LEN] {
    let mut out = [0u8; ERROR_PAYLOAD_LEN];
    out[0..16].copy_from_slice(payload.capture_id.as_bytes());
    out[16..18].copy_from_slice(&(payload.code as u16).to_le_bytes());
    // 18..20 reserved = zero
    out
}

pub fn decode_error(bytes: &[u8]) -> Result<ErrorPayload, AgentError> {
    if bytes.len() != ERROR_PAYLOAD_LEN {
        return Err(AgentError::Contract(format!(
            "ERROR payload must be {ERROR_PAYLOAD_LEN} bytes"
        )));
    }
    if bytes[18..20] != [0, 0] {
        return Err(AgentError::Contract(
            "ERROR reserved bytes must be zero".into(),
        ));
    }
    let code = match u16::from_le_bytes(bytes[16..18].try_into().expect("fixed slice")) {
        1 => StationErrorCode::InvalidCommand,
        2 => StationErrorCode::RfidReadFailed,
        3 => StationErrorCode::InvalidEventContext,
        4 => StationErrorCode::SigningFailed,
        5 => StationErrorCode::Busy,
        6 => StationErrorCode::RfidTimeout,
        value => {
            return Err(AgentError::Contract(format!(
                "unknown Station error code {value}"
            )));
        }
    };
    Ok(ErrorPayload {
        capture_id: Uuid::from_bytes(bytes[0..16].try_into().expect("fixed slice")),
        code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_event_ready_round_trips_exactly() {
        let payload = DomainEventReadyPayload {
            envelope_bytes: [0x11; 220],
            station_pubkey33: [0x22; 33],
            station_signature64: [0x33; 64],
        };
        let encoded = encode_domain_event_ready(&payload);
        assert_eq!(encoded.len(), DOMAIN_EVENT_READY_PAYLOAD_LEN);
        assert_eq!(decode_domain_event_ready(&encoded).unwrap(), payload);
    }

    #[test]
    fn domain_ack_round_trips_and_rejects_wrong_length() {
        let payload = DomainAckPayload {
            event_hash: [0x44; 32],
        };
        let encoded = encode_domain_ack(&payload);
        assert_eq!(decode_domain_ack(&encoded).unwrap(), payload);
        assert!(decode_domain_ack(&encoded[..31]).is_err());
    }
}
