//! Explicit errors for v2 account and authorization guards.

use anchor_lang::prelude::*;

#[error_code]
pub enum LastroV2Error {
    #[msg("invalid deployment identifier")]
    InvalidDeployment,
    #[msg("unsupported schema version")]
    UnsupportedSchemaVersion,
    #[msg("invalid configuration authority")]
    InvalidConfigAuthority,
    #[msg("invalid identifier")]
    InvalidIdentifier,
    #[msg("invalid public key")]
    InvalidPublicKey,
    #[msg("invalid time window")]
    InvalidTimeWindow,
    #[msg("invalid weight or tolerance")]
    InvalidWeight,
    #[msg("invalid station status")]
    InvalidStationStatus,
    #[msg("invalid facility status")]
    InvalidFacilityStatus,
    #[msg("invalid facility type")]
    InvalidFacilityType,
    #[msg("invalid party role")]
    InvalidPartyRole,
    #[msg("invalid asset type")]
    InvalidAssetType,
    #[msg("invalid asset status")]
    InvalidAssetStatus,
    #[msg("invalid intent type")]
    InvalidIntentType,
    #[msg("invalid intent expiration")]
    InvalidIntentExpiration,
    #[msg("invalid state version")]
    InvalidStateVersion,
    #[msg("invalid previous event hash")]
    InvalidPreviousHash,
    #[msg("asset already exists")]
    DuplicateAsset,
    #[msg("asset is already closed")]
    AssetClosed,
    #[msg("intent already exists")]
    DuplicateIntent,
    #[msg("intent actor is not authorized")]
    UnauthorizedActor,
    #[msg("intent is not open")]
    IntentNotOpen,
    #[msg("intent is expired")]
    IntentExpired,
    #[msg("intent payload does not match")]
    IntentPayloadMismatch,
    #[msg("invalid domain event")]
    InvalidDomainEvent,
    #[msg("station proof is missing or does not match the event")]
    InvalidStationProof,
    #[msg("event predecessor does not match asset state")]
    InvalidEventPredecessor,
    #[msg("event state version does not advance exactly once")]
    InvalidEventStateVersion,
    #[msg("event is outside its validity window")]
    EventOutsideValidityWindow,
    #[msg("facility owner is not authorized for this operation")]
    UnauthorizedFacility,
    #[msg("transformation manifest is invalid")]
    InvalidTransformationManifest,
    #[msg("transformation manifest hash does not match canonical bytes")]
    ManifestHashMismatch,
    #[msg("transformation is not open")]
    TransformationNotOpen,
    #[msg("transformation has expired")]
    TransformationExpired,
    #[msg("asset is already reserved by another transformation")]
    AssetReserved,
    #[msg("reservation is invalid")]
    InvalidReservation,
}
