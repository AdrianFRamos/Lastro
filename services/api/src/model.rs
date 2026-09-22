//! HTTP DTOs and database projection models.
//!
//! Hex/base64 values stay strings at the HTTP edge. Domain code must decode them
//! into fixed-size arrays before cryptographic or state decisions. Never compare
//! unvalidated strings as if they were protocol values.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Hex32 = String;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateAnimalRequest { pub visual_recovery_id: String }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimalResponse {
    pub animal_id: Hex32,
    pub visual_recovery_id: String,
    pub current_rfid_hash: Option<Hex32>,
    pub current_custodian: Option<Hex32>,
    pub identity_revision: u32,
    pub event_sequence: u64,
    pub last_event_hash: Option<Hex32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CaptureAction { Origin, Transfer, Reidentify }

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCaptureRequest {
    pub action: CaptureAction,
    pub animal_id: Hex32,
    pub next_custodian: Option<Hex32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CaptureStatus { Pending, Dispatched, EvidenceAccepted, Expired, Cancelled }

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventStatus { EvidenceAccepted, Submitted, Finalized, Rejected }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResponse {
    pub capture_id: Uuid,
    pub action: CaptureAction,
    pub animal_id: Hex32,
    pub status: CaptureStatus,
    /// Immutable accepted event identifier. Null until Station evidence is admitted.
    pub event_hash: Option<Hex32>,
    /// Event lifecycle is separate from physical capture lifecycle. Null before evidence admission.
    pub event_status: Option<EventStatus>,
    /// Stored only after RPC proves the exact transaction at confirmed commitment.
    pub tx_signature: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCommandResponse {
    pub capture_id: Uuid,
    pub action: CaptureAction,
    pub deployment_id: Hex32,
    pub animal_id: Hex32,
    pub event_sequence: u64,
    pub identity_revision: u32,
    pub previous_event_hash: Hex32,
    pub expected_old_rfid_hash: Hex32,
    pub from_custodian: Hex32,
    pub to_custodian: Hex32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentEvidenceRequest {
    pub capture_id: Uuid,
    pub event_bytes_base64: String,
    pub observed_rfid_hex: String,
    pub station_pubkey_hex: String,
    pub station_signature_hex: String,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentEvidenceStatus {
    EvidenceAccepted,
    Submitted,
    Finalized,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEvidenceStatusResponse {
    pub status: AgentEvidenceStatus,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmEventRequest { pub tx_signature: String }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSubmissionResponse {
    pub status: EventStatus,
    pub tx_signature: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountMetaDto { pub address: String, pub is_signer: bool, pub is_writable: bool }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionDto {
    pub program_id: String,
    pub accounts: Vec<AccountMetaDto>,
    pub data_base64: String,
}


#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionVersionDto {
    Legacy,
    #[serde(rename = "v0")]
    V0,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionDataResponse {
    /// Wallet that must authorize this state transition.
    pub required_signer: String,
    /// The program id the browser must independently compare with VITE_LASTRO_PROGRAM_ID.
    pub lastro_program_id: String,
    /// Exactly two protocol instructions in the hackathon: Secp256r1 then Lastro.
    pub instructions: Vec<InstructionDto>,
    /// Serialized size measured by the builder fixture for the chosen transaction format.
    pub measured_serialized_bytes: usize,
    pub transaction_version: TransactionVersionDto,
}


pub fn parse_hex32(name: &str, value: &str) -> Result<[u8; 32], crate::error::ApiError> {
    if value.len() != 64 || value != value.to_ascii_lowercase() {
        return Err(crate::error::ApiError::Validation(format!("{name} must be 32-byte lowercase hex")));
    }
    let bytes = hex::decode(value)
        .map_err(|_| crate::error::ApiError::Validation(format!("{name} must be 32-byte lowercase hex")))?;
    bytes.try_into()
        .map_err(|_| crate::error::ApiError::Validation(format!("{name} must be 32-byte lowercase hex")))
}
