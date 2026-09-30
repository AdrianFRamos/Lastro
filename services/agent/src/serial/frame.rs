use bytes::Bytes;

use crate::error::AgentError;

/// Serial envelope message types frozen by docs/PROTOCOL.md.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageType {
    Command = 1,
    EventReady = 2,
    Ack = 3,
    Error = 4,
    DomainEventReady = 5,
    DomainAck = 6,
}

impl MessageType {
    pub const fn payload_len(self) -> usize {
        match self {
            Self::Command => super::payload::COMMAND_PAYLOAD_LEN,
            Self::EventReady => super::payload::EVENT_READY_PAYLOAD_LEN,
            Self::Ack => super::payload::ACK_PAYLOAD_LEN,
            Self::Error => super::payload::ERROR_PAYLOAD_LEN,
            Self::DomainEventReady => super::payload::DOMAIN_EVENT_READY_PAYLOAD_LEN,
            Self::DomainAck => super::payload::DOMAIN_ACK_PAYLOAD_LEN,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub message_type: MessageType,
    pub payload: Bytes,
}

impl TryFrom<u8> for MessageType {
    type Error = AgentError;

    fn try_from(value: u8) -> Result<Self, AgentError> {
        match value {
            1 => Ok(Self::Command),
            2 => Ok(Self::EventReady),
            3 => Ok(Self::Ack),
            4 => Ok(Self::Error),
            5 => Ok(Self::DomainEventReady),
            6 => Ok(Self::DomainAck),
            _ => Err(AgentError::Serial(format!(
                "unknown serial message type {value}"
            ))),
        }
    }
}
