#![forbid(unsafe_code)]
#![allow(unexpected_cfgs)]
//! Lastro domain program v2.
//!
//! This program is intentionally separate from the legacy v1 program. It anchors
//! compact domain state and commitments; detailed manifests remain off-chain.

use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod verify;

use crate::instructions::{
    CancelIntent, ConsumeIntent, CreateIntent, ExpireIntent, InitializeV2, RecordObservation,
    RegisterAsset, RegisterFacility, RegisterParty, RegisterStation, SetFacilityStatus,
    SetStationStatus,
};

declare_id!("7H5tixrcDMrAFGhbbXJ2sTy9sYPmezQKexhJ6FMBvD8F");

// Keep these aliases at the crate root for Anchor 1.2 generated client helpers.
pub(crate) use instructions::assets::__client_accounts_register_asset;
pub(crate) use instructions::events::__client_accounts_record_observation;
pub(crate) use instructions::facilities::__client_accounts_register_facility;
pub(crate) use instructions::facilities::__client_accounts_set_facility_status;
pub(crate) use instructions::initialize::__client_accounts_initialize_v2;
pub(crate) use instructions::intents::__client_accounts_cancel_intent;
pub(crate) use instructions::intents::__client_accounts_consume_intent;
pub(crate) use instructions::intents::__client_accounts_create_intent;
pub(crate) use instructions::intents::__client_accounts_expire_intent;
pub(crate) use instructions::parties::__client_accounts_register_party;
pub(crate) use instructions::stations::__client_accounts_register_station;
pub(crate) use instructions::stations::__client_accounts_set_station_status;

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

    pub fn register_facility(
        ctx: Context<RegisterFacility>,
        facility_id: [u8; 32],
        owner: Pubkey,
        facility_type: u8,
        credential_hash: [u8; 32],
        valid_from: i64,
        valid_until: i64,
    ) -> Result<()> {
        instructions::facilities::register_handler(
            ctx,
            facility_id,
            owner,
            facility_type,
            credential_hash,
            valid_from,
            valid_until,
        )
    }

    pub fn set_facility_status(ctx: Context<SetFacilityStatus>, status: u8) -> Result<()> {
        instructions::facilities::set_status_handler(ctx, status)
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
}
