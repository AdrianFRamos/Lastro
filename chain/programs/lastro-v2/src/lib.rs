#![forbid(unsafe_code)]
#![allow(unexpected_cfgs)]
// Anchor's generated CPI client mirrors each instruction's argument list.
#![cfg_attr(feature = "cpi", allow(clippy::too_many_arguments))]
//! Lastro domain program v2.
//!
//! Anchors compact domain state and commitments; detailed manifests remain off-chain.

use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod verify;

use crate::instructions::{
    AbortTransformation, AcceptCustodyTransfer, AnchorObservationBatch, BeginTransformation,
    BindIdentifier, CancelIntent, CloseSession, ConsumeIntent, ConsumeTransformationInput,
    CreateIntent, CreateTransformationOutput, ExpireIntent, ExpireTransformation,
    FinalizeTransformation, InitializeV2, OpenSession, RecordObservation, RegisterAsset,
    RegisterFacility, RegisterParty, RegisterStation, ReleaseTransformationInput,
    ReplaceIdentifier, ReserveTransformationInput, RevokeSession, SetFacilityStatus,
    SetPartyStatus, SetStationFacility, SetStationStatus, TransferConfigAuthority,
};

declare_id!("9hYgxhMCrkmasqW99qRaBxSndaFqkgZqwojSmnB9Ab3h");

// Keep these aliases at the crate root for Anchor 1.2 generated client helpers.
pub(crate) use instructions::assets::__client_accounts_register_asset;
pub(crate) use instructions::consumption::__client_accounts_consume_transformation_input;
pub(crate) use instructions::events::__client_accounts_record_observation;
pub(crate) use instructions::facilities::__client_accounts_register_facility;
pub(crate) use instructions::facilities::__client_accounts_set_facility_status;
pub(crate) use instructions::identity::__client_accounts_bind_identifier;
pub(crate) use instructions::identity::__client_accounts_replace_identifier;
pub(crate) use instructions::initialize::__client_accounts_initialize_v2;
pub(crate) use instructions::initialize::__client_accounts_transfer_config_authority;
pub(crate) use instructions::intents::__client_accounts_accept_custody_transfer;
pub(crate) use instructions::intents::__client_accounts_cancel_intent;
pub(crate) use instructions::intents::__client_accounts_consume_intent;
pub(crate) use instructions::intents::__client_accounts_create_intent;
pub(crate) use instructions::intents::__client_accounts_expire_intent;
pub(crate) use instructions::outputs::__client_accounts_create_transformation_output;
pub(crate) use instructions::parties::__client_accounts_register_party;
pub(crate) use instructions::parties::__client_accounts_set_party_status;
pub(crate) use instructions::reservations::__client_accounts_release_transformation_input;
pub(crate) use instructions::reservations::__client_accounts_reserve_transformation_input;
pub(crate) use instructions::sessions::__client_accounts_anchor_observation_batch;
pub(crate) use instructions::sessions::__client_accounts_close_session;
pub(crate) use instructions::sessions::__client_accounts_open_session;
pub(crate) use instructions::sessions::__client_accounts_revoke_session;
pub(crate) use instructions::stations::__client_accounts_register_station;
pub(crate) use instructions::stations::__client_accounts_set_station_facility;
pub(crate) use instructions::stations::__client_accounts_set_station_status;
pub(crate) use instructions::transformations::__client_accounts_abort_transformation;
pub(crate) use instructions::transformations::__client_accounts_begin_transformation;
pub(crate) use instructions::transformations::__client_accounts_expire_transformation;
pub(crate) use instructions::transformations::__client_accounts_finalize_transformation;

// With the `cpi` feature Anchor also resolves the CPI client account modules from the root.
#[cfg(feature = "cpi")]
pub(crate) use instructions::assets::__cpi_client_accounts_register_asset;
#[cfg(feature = "cpi")]
pub(crate) use instructions::consumption::__cpi_client_accounts_consume_transformation_input;
#[cfg(feature = "cpi")]
pub(crate) use instructions::events::__cpi_client_accounts_record_observation;
#[cfg(feature = "cpi")]
pub(crate) use instructions::facilities::__cpi_client_accounts_register_facility;
#[cfg(feature = "cpi")]
pub(crate) use instructions::facilities::__cpi_client_accounts_set_facility_status;
#[cfg(feature = "cpi")]
pub(crate) use instructions::identity::__cpi_client_accounts_bind_identifier;
#[cfg(feature = "cpi")]
pub(crate) use instructions::identity::__cpi_client_accounts_replace_identifier;
#[cfg(feature = "cpi")]
pub(crate) use instructions::initialize::__cpi_client_accounts_initialize_v2;
#[cfg(feature = "cpi")]
pub(crate) use instructions::initialize::__cpi_client_accounts_transfer_config_authority;
#[cfg(feature = "cpi")]
pub(crate) use instructions::intents::__cpi_client_accounts_accept_custody_transfer;
#[cfg(feature = "cpi")]
pub(crate) use instructions::intents::__cpi_client_accounts_cancel_intent;
#[cfg(feature = "cpi")]
pub(crate) use instructions::intents::__cpi_client_accounts_consume_intent;
#[cfg(feature = "cpi")]
pub(crate) use instructions::intents::__cpi_client_accounts_create_intent;
#[cfg(feature = "cpi")]
pub(crate) use instructions::intents::__cpi_client_accounts_expire_intent;
#[cfg(feature = "cpi")]
pub(crate) use instructions::outputs::__cpi_client_accounts_create_transformation_output;
#[cfg(feature = "cpi")]
pub(crate) use instructions::parties::__cpi_client_accounts_register_party;
#[cfg(feature = "cpi")]
pub(crate) use instructions::parties::__cpi_client_accounts_set_party_status;
#[cfg(feature = "cpi")]
pub(crate) use instructions::reservations::__cpi_client_accounts_release_transformation_input;
#[cfg(feature = "cpi")]
pub(crate) use instructions::reservations::__cpi_client_accounts_reserve_transformation_input;
#[cfg(feature = "cpi")]
pub(crate) use instructions::sessions::__cpi_client_accounts_anchor_observation_batch;
#[cfg(feature = "cpi")]
pub(crate) use instructions::sessions::__cpi_client_accounts_close_session;
#[cfg(feature = "cpi")]
pub(crate) use instructions::sessions::__cpi_client_accounts_open_session;
#[cfg(feature = "cpi")]
pub(crate) use instructions::sessions::__cpi_client_accounts_revoke_session;
#[cfg(feature = "cpi")]
pub(crate) use instructions::stations::__cpi_client_accounts_register_station;
#[cfg(feature = "cpi")]
pub(crate) use instructions::stations::__cpi_client_accounts_set_station_facility;
#[cfg(feature = "cpi")]
pub(crate) use instructions::stations::__cpi_client_accounts_set_station_status;
#[cfg(feature = "cpi")]
pub(crate) use instructions::transformations::__cpi_client_accounts_abort_transformation;
#[cfg(feature = "cpi")]
pub(crate) use instructions::transformations::__cpi_client_accounts_begin_transformation;
#[cfg(feature = "cpi")]
pub(crate) use instructions::transformations::__cpi_client_accounts_expire_transformation;
#[cfg(feature = "cpi")]
pub(crate) use instructions::transformations::__cpi_client_accounts_finalize_transformation;

#[program]
pub mod lastro_v2 {
    use super::*;

    pub fn initialize_v2(
        ctx: Context<InitializeV2>,
        deployment_id: [u8; 32],
        schema_version: u16,
        max_asset_weight_grams: u64,
        mass_tolerance_basis_points: u16,
        max_event_age_seconds: u64,
    ) -> Result<()> {
        instructions::initialize::handler(
            ctx,
            deployment_id,
            schema_version,
            max_asset_weight_grams,
            mass_tolerance_basis_points,
            max_event_age_seconds,
        )
    }

    pub fn register_station_v2(
        ctx: Context<RegisterStation>,
        station_id: [u8; 32],
        key_id: [u8; 32],
        pubkey33: [u8; 33],
        valid_from: i64,
        valid_until: i64,
        firmware_hash: [u8; 32],
    ) -> Result<()> {
        instructions::stations::register_handler(
            ctx,
            station_id,
            key_id,
            pubkey33,
            valid_from,
            valid_until,
            firmware_hash,
        )
    }

    pub fn set_station_status(ctx: Context<SetStationStatus>, status: u8) -> Result<()> {
        instructions::stations::set_status_handler(ctx, status)
    }

    pub fn extend_station_validity(ctx: Context<SetStationStatus>, valid_until: i64) -> Result<()> {
        instructions::stations::extend_validity_handler(ctx, valid_until)
    }

    pub fn register_facility(
        ctx: Context<RegisterFacility>,
        facility_id: [u8; 32],
        owner: Pubkey,
        facility_type: u8,
        credential_hash: [u8; 32],
        valid_from: i64,
        valid_until: i64,
        geo_commitment: [u8; 32],
    ) -> Result<()> {
        instructions::facilities::register_handler(
            ctx,
            facility_id,
            owner,
            facility_type,
            credential_hash,
            valid_from,
            valid_until,
            geo_commitment,
        )
    }

    pub fn set_station_facility(ctx: Context<SetStationFacility>) -> Result<()> {
        instructions::stations::set_facility_handler(ctx)
    }

    pub fn set_facility_status(ctx: Context<SetFacilityStatus>, status: u8) -> Result<()> {
        instructions::facilities::set_status_handler(ctx, status)
    }

    pub fn transfer_config_authority(ctx: Context<TransferConfigAuthority>) -> Result<()> {
        instructions::initialize::transfer_authority_handler(ctx)
    }

    pub fn set_party_status(ctx: Context<SetPartyStatus>, status: u8) -> Result<()> {
        instructions::parties::set_status_handler(ctx, status)
    }

    pub fn register_party(
        ctx: Context<RegisterParty>,
        party_id: [u8; 32],
        wallet: Pubkey,
        role: u16,
    ) -> Result<()> {
        instructions::parties::register_handler(ctx, party_id, wallet, role)
    }

    pub fn register_asset(
        ctx: Context<RegisterAsset>,
        asset_id: [u8; 32],
        asset_type: u8,
        custodian: Pubkey,
        parent_root: [u8; 32],
        lineage_root: [u8; 32],
        available_weight_grams: u64,
    ) -> Result<()> {
        instructions::assets::register_handler(
            ctx,
            asset_id,
            asset_type,
            custodian,
            parent_root,
            lineage_root,
            available_weight_grams,
        )
    }

    pub fn record_observation(
        ctx: Context<RecordObservation>,
        subject_id: [u8; 32],
        event_id: [u8; 32],
        station_id: [u8; 32],
        event: [u8; lastro_protocol::v2::V2_ENVELOPE_LEN],
    ) -> Result<()> {
        instructions::events::handler(ctx, subject_id, event_id, station_id, event)
    }

    pub fn bind_identifier(
        ctx: Context<BindIdentifier>,
        subject_id: [u8; 32],
        event_id: [u8; 32],
        station_id: [u8; 32],
        event: [u8; lastro_protocol::v2::V2_ENVELOPE_LEN],
        new_rfid_hash: [u8; 32],
    ) -> Result<()> {
        instructions::identity::bind_handler(
            ctx,
            subject_id,
            event_id,
            station_id,
            event,
            new_rfid_hash,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn replace_identifier(
        ctx: Context<ReplaceIdentifier>,
        subject_id: [u8; 32],
        event_id: [u8; 32],
        station_id: [u8; 32],
        event: [u8; lastro_protocol::v2::V2_ENVELOPE_LEN],
        old_rfid_hash: [u8; 32],
        new_rfid_hash: [u8; 32],
    ) -> Result<()> {
        instructions::identity::replace_handler(
            ctx,
            subject_id,
            event_id,
            station_id,
            event,
            old_rfid_hash,
            new_rfid_hash,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn begin_transformation(
        ctx: Context<BeginTransformation>,
        transformation_id: [u8; 32],
        facility_id: [u8; 32],
        transformation_type: u16,
        input_root: [u8; 32],
        output_root: [u8; 32],
        input_count: u32,
        output_count: u32,
        input_weight_grams: u64,
        output_weight_grams: u64,
        byproduct_weight_grams: u64,
        loss_weight_grams: u64,
        tolerance_basis_points: u16,
        manifest_nonce: u64,
        expires_at: i64,
        manifest_hash: [u8; 32],
    ) -> Result<()> {
        instructions::transformations::begin_handler(
            ctx,
            transformation_id,
            facility_id,
            transformation_type,
            input_root,
            output_root,
            input_count,
            output_count,
            input_weight_grams,
            output_weight_grams,
            byproduct_weight_grams,
            loss_weight_grams,
            tolerance_basis_points,
            manifest_nonce,
            expires_at,
            manifest_hash,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_transformation_input(
        ctx: Context<ReserveTransformationInput>,
        transformation_id: [u8; 32],
        asset_id: [u8; 32],
        weight_grams: u64,
        expected_state_version: u64,
        leaf_position: u32,
        leaf_quantity: u64,
        leaf_index: u32,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::reservations::reserve_handler(
            ctx,
            transformation_id,
            asset_id,
            weight_grams,
            expected_state_version,
            leaf_position,
            leaf_quantity,
            leaf_index,
            proof,
        )
    }

    pub fn release_transformation_input(
        ctx: Context<ReleaseTransformationInput>,
        transformation_id: [u8; 32],
        asset_id: [u8; 32],
    ) -> Result<()> {
        instructions::reservations::release_handler(ctx, transformation_id, asset_id)
    }

    pub fn abort_transformation(ctx: Context<AbortTransformation>) -> Result<()> {
        instructions::transformations::abort_handler(ctx)
    }

    pub fn expire_transformation(ctx: Context<ExpireTransformation>) -> Result<()> {
        instructions::transformations::expire_handler(ctx)
    }

    pub fn consume_transformation_input(
        ctx: Context<ConsumeTransformationInput>,
        transformation_id: [u8; 32],
        asset_id: [u8; 32],
        expected_state_version: u64,
    ) -> Result<()> {
        instructions::consumption::consume_handler(
            ctx,
            transformation_id,
            asset_id,
            expected_state_version,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_transformation_output(
        ctx: Context<CreateTransformationOutput>,
        output_id: [u8; 32],
        asset_type: u8,
        lineage_root: [u8; 32],
        weight_grams: u64,
        leaf_role: u8,
        leaf_position: u32,
        leaf_quantity: u64,
        leaf_index: u32,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        instructions::outputs::create_handler(
            ctx,
            output_id,
            asset_type,
            lineage_root,
            weight_grams,
            leaf_role,
            leaf_position,
            leaf_quantity,
            leaf_index,
            proof,
        )
    }

    pub fn finalize_transformation(ctx: Context<FinalizeTransformation>) -> Result<()> {
        instructions::transformations::finalize_handler(ctx)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_intent(
        ctx: Context<CreateIntent>,
        intent_id: [u8; 32],
        subject_id: [u8; 32],
        intent_type: u16,
        expected_state_version: u64,
        nonce: u64,
        expires_at: i64,
        payload_hash: [u8; 32],
    ) -> Result<()> {
        instructions::intents::handler(
            ctx,
            intent_id,
            subject_id,
            intent_type,
            expected_state_version,
            nonce,
            expires_at,
            payload_hash,
        )
    }

    pub fn cancel_intent(ctx: Context<CancelIntent>) -> Result<()> {
        instructions::intents::cancel_handler(ctx)
    }

    pub fn expire_intent(ctx: Context<ExpireIntent>) -> Result<()> {
        instructions::intents::expire_handler(ctx)
    }

    pub fn consume_intent(
        ctx: Context<ConsumeIntent>,
        expected_state_version: u64,
        payload_hash: [u8; 32],
    ) -> Result<()> {
        instructions::intents::consume_handler(ctx, expected_state_version, payload_hash)
    }

    pub fn accept_custody_transfer(
        ctx: Context<AcceptCustodyTransfer>,
        expected_state_version: u64,
    ) -> Result<()> {
        instructions::intents::accept_custody_transfer_handler(ctx, expected_state_version)
    }

    pub fn open_session(
        ctx: Context<OpenSession>,
        session_id: [u8; 32],
        station_id: [u8; 32],
        allowed_actions: u16,
        max_events: u32,
        valid_from: i64,
        expires_at: i64,
    ) -> Result<()> {
        instructions::sessions::open_handler(
            ctx,
            session_id,
            station_id,
            allowed_actions,
            max_events,
            valid_from,
            expires_at,
        )
    }

    pub fn revoke_session(ctx: Context<RevokeSession>) -> Result<()> {
        instructions::sessions::revoke_handler(ctx)
    }

    pub fn close_session(ctx: Context<CloseSession>) -> Result<()> {
        instructions::sessions::close_handler(ctx)
    }

    pub fn anchor_observation_batch(
        ctx: Context<AnchorObservationBatch>,
        merkle_root: [u8; 32],
        leaf_count: u32,
        first_observed_at: i64,
        last_observed_at: i64,
    ) -> Result<()> {
        instructions::sessions::anchor_batch_handler(
            ctx,
            merkle_root,
            leaf_count,
            first_observed_at,
            last_observed_at,
        )
    }
}
