//! COMMAND contract received from the API and sent to the Station (protocol v2).

use lastro_protocol::v2::CaptureCommand;
use uuid::Uuid;

use crate::error::AgentError;

/// One physical capture request. The inner [`CaptureCommand`] is the exact wire contract
/// shared with the Station firmware, the simulator and the API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StationCommand(pub CaptureCommand);

impl StationCommand {
    pub fn capture_id(&self) -> Uuid {
        Uuid::from_bytes(self.0.capture_id)
    }

    /// Validate all context known before the physical RFID read.
    pub fn validate(&self) -> Result<(), AgentError> {
        self.0
            .validate()
            .map_err(|error| AgentError::Contract(format!("invalid capture command: {error}")))
    }
}
