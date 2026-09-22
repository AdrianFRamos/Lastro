//! COMMAND contract received from the API and sent to the Station.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AgentError;

const ZERO32: [u8; 32] = [0; 32];

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct StationCommand {
    pub capture_id: Uuid,
    pub action: u8,
    pub deployment_id: [u8; 32],
    pub animal_id: [u8; 32],
    pub event_sequence: u64,
    pub identity_revision: u32,
    pub previous_event_hash: [u8; 32],
    pub expected_old_rfid_hash: [u8; 32],
    pub from_custodian: [u8; 32],
    pub to_custodian: [u8; 32],
}

impl StationCommand {
    /// Validate all context known before the physical RFID read.
    /// A future/new RFID deliberately does not exist in this contract.
    pub fn validate(&self) -> Result<(), AgentError> {
        if self.capture_id.is_nil() {
            return Err(AgentError::Contract("capture_id must not be nil".into()));
        }
        if self.deployment_id == ZERO32 || self.animal_id == ZERO32 {
            return Err(AgentError::Contract(
                "deployment_id and animal_id must be non-zero".into(),
            ));
        }

        let valid = match self.action {
            1 => {
                self.event_sequence == 1
                    && self.identity_revision == 1
                    && self.previous_event_hash == ZERO32
                    && self.expected_old_rfid_hash == ZERO32
                    && self.from_custodian == ZERO32
                    && self.to_custodian != ZERO32
            }
            2 => {
                self.event_sequence >= 2
                    && self.identity_revision >= 1
                    && self.previous_event_hash != ZERO32
                    && self.expected_old_rfid_hash != ZERO32
                    && self.from_custodian != ZERO32
                    && self.to_custodian != ZERO32
                    && self.from_custodian != self.to_custodian
            }
            3 => {
                self.event_sequence >= 2
                    && self.identity_revision >= 2
                    && self.previous_event_hash != ZERO32
                    && self.expected_old_rfid_hash != ZERO32
                    && self.from_custodian != ZERO32
                    && self.from_custodian == self.to_custodian
            }
            _ => false,
        };

        if !valid {
            return Err(AgentError::Contract(format!(
                "invalid command semantics for action {}",
                self.action
            )));
        }
        Ok(())
    }
}
