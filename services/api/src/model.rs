//! HTTP DTOs and database projection models.
//!
//! Hex/base64 values stay strings at the HTTP edge. Domain code must decode them
//! into fixed-size arrays before cryptographic or state decisions. Never compare
//! unvalidated strings as if they were protocol values.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Hex32 = String;

/// Physical capture kinds. BIND/REPLACE are signed by the asset's current custodian;
/// OBSERVE (presence proof) likewise; the program also accepts the deployment authority.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CaptureAction {
    BindIdentifier,
    ReplaceIdentifier,
    ObservePresence,
}

impl CaptureAction {
    pub const fn event_type(self) -> u16 {
        match self {
            Self::BindIdentifier => 18,
            Self::ReplaceIdentifier => 19,
            Self::ObservePresence => 2,
        }
    }

    pub const fn from_event_type(value: u16) -> Option<Self> {
        match value {
            18 => Some(Self::BindIdentifier),
            19 => Some(Self::ReplaceIdentifier),
            2 => Some(Self::ObservePresence),
            _ => None,
        }
    }

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::BindIdentifier => "IDENTIFIER_BOUND",
            Self::ReplaceIdentifier => "IDENTIFIER_REPLACED",
            Self::ObservePresence => "OBSERVATION_RECORDED",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureIntentRequest {
    pub action: CaptureAction,
    pub asset_id: Hex32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureAuthorizationProof {
    pub challenge_id: Uuid,
    pub signature_base64: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCaptureRequest {
    pub action: CaptureAction,
    pub asset_id: Hex32,
    pub authorization: CaptureAuthorizationProof,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureAuthorizationChallengeResponse {
    pub challenge_id: Uuid,
    pub deployment_id: Hex32,
    pub required_signer: String,
    pub message_base64: String,
    pub expires_at_unix: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CaptureStatus {
    Pending,
    Dispatched,
    EvidenceAccepted,
    Expired,
    Cancelled,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureResponse {
    pub capture_id: Uuid,
    pub action: CaptureAction,
    pub asset_id: Hex32,
    pub state_version: u64,
    pub status: CaptureStatus,
    /// Immutable accepted event hash. Null until Station evidence is admitted.
    pub event_hash: Option<Hex32>,
    /// Event lifecycle, separate from the physical capture lifecycle.
    pub event_status: Option<DomainEventStatus>,
    /// Stored only after RPC proves the exact transaction at confirmed commitment.
    pub tx_signature: Option<String>,
}

/// v2 Station command (see docs/MIGRACAO_V1_PARA_V2.md). The event id is derived from
/// captureId by every party, so it is not transported.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCommandResponse {
    pub capture_id: Uuid,
    pub event_type: &'static str,
    pub deployment_id: Hex32,
    pub asset_id: Hex32,
    pub state_version: u64,
    pub previous_event_hash: Hex32,
    pub expected_rfid_hash: Hex32,
    pub observed_at: i64,
    pub expires_at: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentEvidenceRequest {
    pub capture_id: Uuid,
    pub envelope_base64: String,
    pub observed_rfid_hex: String,
    pub station_pubkey_hex: String,
    pub station_signature_hex: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegisterAssetRequest {
    pub asset_id: Hex32,
    /// Initial custodian wallet (32-byte lowercase hex).
    pub custodian: Hex32,
    /// 1 = animal (default flow); other v2 asset types are accepted as defined on-chain.
    pub asset_type: u8,
    pub available_weight_grams: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetResponse {
    pub asset_id: Hex32,
    pub asset_type: u8,
    pub status: u8,
    pub custodian: Hex32,
    pub state_version: u64,
    pub event_sequence: u64,
    pub last_event_hash: Hex32,
    /// Null until an RFID is bound.
    pub current_rfid_hash: Option<Hex32>,
    pub available_weight_grams: u64,
}

/// Portable proof bundle an independent verifier checks against Solana.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidencePackageV2 {
    pub schema: &'static str,
    pub deployment_id: Hex32,
    pub lastro_program_id: String,
    pub asset: AssetResponse,
    /// Finalized Station events in state-version order.
    pub events: Vec<DomainEventAnchorResponse>,
    /// Accepted custody transfers: the verifier re-checks each finalized transaction.
    pub custody_transfers: Vec<CustodyProofResponse>,
}

/// Minimal public proof of one custody change (no reasons or internal party ids).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustodyProofResponse {
    pub transfer_id: Uuid,
    pub new_custodian: Hex32,
    pub tx_signature: String,
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
pub struct ConfirmEventRequest {
    pub tx_signature: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountMetaDto {
    pub address: String,
    pub is_signer: bool,
    pub is_writable: bool,
}

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
    /// Protocol instructions (Secp256r1 then Lastro for Station events), optionally followed
    /// by ComputeBudget priority-fee instructions.
    pub instructions: Vec<InstructionDto>,
    /// Serialized size measured by the builder fixture for the chosen transaction format.
    pub measured_serialized_bytes: usize,
    pub transaction_version: TransactionVersionDto,
}

pub fn parse_hex32(name: &str, value: &str) -> Result<[u8; 32], crate::error::ApiError> {
    if value.len() != 64 || value != value.to_ascii_lowercase() {
        return Err(crate::error::ApiError::Validation(format!(
            "{name} must be 32-byte lowercase hex"
        )));
    }
    let bytes = hex::decode(value).map_err(|_| {
        crate::error::ApiError::Validation(format!("{name} must be 32-byte lowercase hex"))
    })?;
    bytes.try_into().map_err(|_| {
        crate::error::ApiError::Validation(format!("{name} must be 32-byte lowercase hex"))
    })
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DomainObservationRequest {
    pub envelope_bytes_base64: String,
    pub station_pubkey_hex: String,
    pub station_signature_hex: String,
}

#[derive(Clone, Copy, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DomainEventStatus {
    EvidenceAccepted,
    Submitted,
    Finalized,
    Rejected,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DomainEventAnchorResponse {
    pub event_id: Hex32,
    pub event_hash: Hex32,
    pub deployment_id: Hex32,
    pub subject_id: Hex32,
    pub source_id: Hex32,
    pub event_type: u16,
    pub state_version: u64,
    pub observed_at: i64,
    pub expires_at: i64,
    pub expected_previous_hash: Hex32,
    pub payload_hash: Hex32,
    pub envelope_bytes_base64: String,
    pub station_pubkey_hex: String,
    pub station_signature_hex: String,
    /// Canonical RFID the Station read (captured identity/presence events only).
    pub observed_rfid_hex: Option<String>,
    pub status: DomainEventStatus,
    pub tx_signature: Option<String>,
}

/// One lineage leaf. Inputs use role 1; outputs use role 2 (OUTPUT) or 3 (BYPRODUCT).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LineageLeafInput {
    pub asset_id: Hex32,
    pub role: u8,
    pub position: u32,
    pub quantity: u64,
    pub weight_grams: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegisterTransformationRequest {
    pub transformation_id: Hex32,
    pub facility_id: Hex32,
    pub transformation_type: u16,
    pub inputs: Vec<LineageLeafInput>,
    pub outputs: Vec<LineageLeafInput>,
    pub loss_weight_grams: u64,
    pub tolerance_basis_points: u16,
    pub manifest_nonce: u64,
    pub expires_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineageLeafProofResponse {
    pub asset_id: Hex32,
    pub role: u8,
    pub position: u32,
    pub quantity: u64,
    pub weight_grams: u64,
    /// Index in the position-sorted leaf list, as the on-chain verifier expects.
    pub leaf_index: u32,
    pub proof: Vec<Hex32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformationRegistrationResponse {
    pub transformation: TransformationResponse,
    pub input_proofs: Vec<LineageLeafProofResponse>,
    pub output_proofs: Vec<LineageLeafProofResponse>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyncTransformationRequest {
    /// Required when the chain reports FINALIZED: the facility owner's finalize transaction.
    pub tx_signature: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformationResponse {
    pub transformation_id: Hex32,
    pub deployment_id: Hex32,
    pub facility_id: Hex32,
    pub transformation_type: u16,
    pub input_root: Hex32,
    pub output_root: Hex32,
    pub input_count: u32,
    pub output_count: u32,
    pub input_weight_grams: u64,
    pub output_weight_grams: u64,
    pub byproduct_weight_grams: u64,
    pub loss_weight_grams: u64,
    pub tolerance_basis_points: u16,
    pub manifest_nonce: u64,
    pub manifest_hash: Hex32,
    pub manifest_bytes_base64: String,
    pub status: String,
    pub sequence: u64,
    pub expires_at: i64,
    pub tx_signature: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineageEdgeResponse {
    pub transformation_id: Hex32,
    pub parent_asset_id: Hex32,
    pub child_asset_id: Hex32,
    pub role: u16,
    pub position: u32,
    pub quantity: u64,
    pub weight_grams: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreatePartyRequest {
    pub party_id: Hex32,
    pub legal_name: String,
    pub tax_id_hash: Option<Hex32>,
    pub wallet: Hex32,
    pub role: u16,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartyResponse {
    pub party_id: Hex32,
    pub deployment_id: Hex32,
    pub legal_name: String,
    pub tax_id_hash: Option<Hex32>,
    pub wallet: Hex32,
    pub role: u16,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateFacilityRequest {
    pub facility_id: Hex32,
    pub owner_party_id: Hex32,
    pub facility_type: u16,
    pub display_name: String,
    pub credential_hash: Hex32,
    pub valid_from: i64,
    pub valid_until: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacilityResponse {
    pub facility_id: Hex32,
    pub deployment_id: Hex32,
    pub owner_party_id: Hex32,
    pub facility_type: u16,
    pub display_name: String,
    pub credential_hash: Hex32,
    pub valid_from: i64,
    pub valid_until: i64,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LotAssetInput {
    pub asset_id: Hex32,
    pub quantity: u64,
    pub weight_grams: u64,
    pub role: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateLotRequest {
    pub lot_id: Hex32,
    pub facility_id: Hex32,
    pub owner_party_id: Hex32,
    pub external_reference: Option<String>,
    pub head_count: u32,
    pub live_weight_grams: u64,
    pub assets: Vec<LotAssetInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LotAssetResponse {
    pub asset_id: Hex32,
    pub quantity: u64,
    pub weight_grams: u64,
    pub role: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LotResponse {
    pub lot_id: Hex32,
    pub deployment_id: Hex32,
    pub facility_id: Hex32,
    pub owner_party_id: Hex32,
    pub external_reference: Option<String>,
    pub head_count: u32,
    pub live_weight_grams: u64,
    pub status: String,
    pub assets: Vec<LotAssetResponse>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCustodyTransferRequest {
    pub transfer_id: Uuid,
    pub asset_id: Hex32,
    pub from_party_id: Option<Hex32>,
    pub to_party_id: Hex32,
    pub from_facility_id: Option<Hex32>,
    pub to_facility_id: Hex32,
    pub reason: String,
    pub created_by_party_id: Option<Hex32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustodyTransferResponse {
    pub transfer_id: Uuid,
    pub deployment_id: Hex32,
    pub asset_id: Hex32,
    pub from_party_id: Option<Hex32>,
    pub to_party_id: Hex32,
    pub from_facility_id: Option<Hex32>,
    pub to_facility_id: Hex32,
    pub reason: String,
    pub status: String,
    pub event_id: Option<Hex32>,
    pub tx_signature: Option<String>,
    pub created_by_party_id: Option<Hex32>,
    pub accepted_by_party_id: Option<Hex32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptCustodyTransferRequest {
    /// Finalized `accept_custody_transfer` transaction signed by the recipient wallet.
    pub tx_signature: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CustodyTransferPhase {
    Propose,
    Accept,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustodyTransferTransactionQuery {
    pub phase: CustodyTransferPhase,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessingItemInput {
    pub asset_id: Option<Hex32>,
    pub direction: String,
    pub quantity: u64,
    pub weight_grams: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateProcessingOperationRequest {
    pub operation_id: Hex32,
    pub facility_id: Hex32,
    pub lot_id: Option<Hex32>,
    pub transformation_id: Option<Hex32>,
    pub operator_party_id: Hex32,
    pub operation_kind: String,
    pub notes: Option<String>,
    pub items: Vec<ProcessingItemInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingItemResponse {
    pub position: u32,
    pub asset_id: Option<Hex32>,
    pub direction: String,
    pub quantity: u64,
    pub weight_grams: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingOperationResponse {
    pub operation_id: Hex32,
    pub deployment_id: Hex32,
    pub facility_id: Hex32,
    pub lot_id: Option<Hex32>,
    pub transformation_id: Option<Hex32>,
    pub operator_party_id: Hex32,
    pub operation_kind: String,
    pub status: String,
    pub notes: Option<String>,
    pub tx_signature: Option<String>,
    pub items: Vec<ProcessingItemResponse>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShipmentItemInput {
    pub asset_id: Hex32,
    pub quantity: u64,
    pub weight_grams: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateShipmentRequest {
    pub shipment_id: Hex32,
    pub origin_facility_id: Hex32,
    pub destination_facility_id: Hex32,
    pub carrier_party_id: Hex32,
    pub created_by_party_id: Hex32,
    pub planned_departure: Option<String>,
    pub notes: Option<String>,
    pub items: Vec<ShipmentItemInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetShipmentStatusRequest {
    pub status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipmentItemResponse {
    pub position: u32,
    pub asset_id: Hex32,
    pub quantity: u64,
    pub weight_grams: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipmentResponse {
    pub shipment_id: Hex32,
    pub deployment_id: Hex32,
    pub origin_facility_id: Hex32,
    pub destination_facility_id: Hex32,
    pub carrier_party_id: Hex32,
    pub created_by_party_id: Hex32,
    pub status: String,
    pub planned_departure: Option<String>,
    pub departed_at: Option<String>,
    pub delivered_at: Option<String>,
    pub notes: Option<String>,
    pub tx_signature: Option<String>,
    pub items: Vec<ShipmentItemResponse>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenRecallRequest {
    pub recall_id: Hex32,
    pub opened_by_party_id: Hex32,
    pub scope_type: String,
    pub scope_id: Hex32,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallMemberResponse {
    pub asset_id: Hex32,
    pub traversal_depth: u32,
    pub relation: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallResponse {
    pub recall_id: Hex32,
    pub deployment_id: Hex32,
    pub opened_by_party_id: Hex32,
    pub scope_type: String,
    pub scope_id: Hex32,
    pub reason: String,
    pub status: String,
    pub snapshot_root: Hex32,
    pub members: Vec<RecallMemberResponse>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CloseRecallRequest {
    pub closed_by_party_id: Hex32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrantAuthorityRequest {
    pub grant_id: Uuid,
    pub party_id: Hex32,
    pub facility_id: Option<Hex32>,
    pub capability: String,
    pub granted_by_party_id: Option<Hex32>,
    pub valid_from: i64,
    pub valid_until: i64,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityGrantResponse {
    pub grant_id: Uuid,
    pub deployment_id: Hex32,
    pub party_id: Hex32,
    pub facility_id: Option<Hex32>,
    pub capability: String,
    pub status: String,
    pub granted_by_party_id: Option<Hex32>,
    pub valid_from: i64,
    pub valid_until: i64,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevokeAuthorityRequest {
    pub revoked_by_party_id: Option<Hex32>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SetStatusRequest {
    pub status: String,
    pub actor_party_id: Option<Hex32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntryResponse {
    pub audit_id: i64,
    pub deployment_id: Hex32,
    pub actor_party_id: Option<Hex32>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<Hex32>,
    pub payload: serde_json::Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateMigrationRunRequest {
    pub run_id: Uuid,
    pub requested_by_party_id: Option<Hex32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationCandidateResponse {
    pub source_animal_id: Hex32,
    pub target_asset_id: Option<Hex32>,
    pub status: String,
    pub reason: String,
    pub source_event_sequence: u64,
    pub source_last_event_hash: Option<Hex32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationRunResponse {
    pub run_id: Uuid,
    pub deployment_id: Hex32,
    pub requested_by_party_id: Option<Hex32>,
    pub status: String,
    pub source_count: u64,
    pub eligible_count: u64,
    pub promoted_count: u64,
    pub rejected_count: u64,
    pub candidates: Vec<MigrationCandidateResponse>,
}
