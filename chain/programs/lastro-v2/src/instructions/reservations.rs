//! Reserve an asset balance for one open transformation.

use anchor_lang::prelude::*;
use lastro_protocol::v2::LineageRole;

use crate::{
    constants::{
        ASSET_SEED, ASSET_STATUS_ACTIVE, ASSET_STATUS_IN_TRANSIT, CONFIG_V2_SEED,
        FACILITY_REGISTRY_SEED, FACILITY_SEED, TRANSFORMATION_RESERVATION_SEED,
        TRANSFORMATION_SEED, TRANSFORMATION_STATUS_ABORTED, TRANSFORMATION_STATUS_EXPIRED,
        TRANSFORMATION_STATUS_FINALIZED, TRANSFORMATION_STATUS_OPEN,
    },
    error::LastroV2Error,
    instructions::transformations::{require_lineage_leaf, require_operating_facility},
    state::{
        AssetState, FacilityRecord, ProtocolConfigV2, RegistryRoot, TransformationAnchor,
        TransformationReservation,
    },
};

/// Inputs must be held by the facility owner and proven members of the manifest's
/// `input_root`. The reserved weight is the committed leaf weight.
#[allow(clippy::too_many_arguments)]
pub fn reserve_handler(
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
    let now = Clock::get()?.unix_timestamp;
    let operator = ctx.accounts.operator.key();
    require!(
        ctx.accounts.transformation.status == TRANSFORMATION_STATUS_OPEN,
        LastroV2Error::TransformationNotOpen
    );
    require!(
        now <= ctx.accounts.transformation.expires_at,
        LastroV2Error::TransformationExpired
    );
    require_operating_facility(&ctx.accounts.facility, operator, now)?;
    require!(
        ctx.accounts.asset.custodian == operator,
        LastroV2Error::UnauthorizedActor
    );
    require!(
        ctx.accounts.asset.state_version == expected_state_version,
        LastroV2Error::InvalidStateVersion
    );
    require!(
        matches!(
            ctx.accounts.asset.status,
            ASSET_STATUS_ACTIVE | ASSET_STATUS_IN_TRANSIT
        ),
        LastroV2Error::InvalidAssetStatus
    );
    require!(weight_grams > 0, LastroV2Error::InvalidWeight);
    require!(
        weight_grams <= ctx.accounts.asset.available_weight_grams,
        LastroV2Error::InvalidWeight
    );
    require_lineage_leaf(
        ctx.accounts.transformation.input_root,
        asset_id,
        LineageRole::Input,
        leaf_position,
        leaf_quantity,
        weight_grams,
        leaf_index,
        &proof,
    )?;
    let next_count = ctx
        .accounts
        .transformation
        .reserved_input_count
        .checked_add(1)
        .ok_or_else(|| error!(LastroV2Error::InvalidReservation))?;
    let next_weight = ctx
        .accounts
        .transformation
        .reserved_input_weight_grams
        .checked_add(weight_grams)
        .ok_or_else(|| error!(LastroV2Error::InvalidWeight))?;
    // Consumed inputs no longer count as reserved, so bound reserved + consumed.
    require!(
        next_count
            .checked_add(ctx.accounts.transformation.consumed_input_count)
            .is_some_and(|total| total <= ctx.accounts.transformation.input_count)
            && next_weight
                .checked_add(ctx.accounts.transformation.consumed_input_weight_grams)
                .is_some_and(|total| total <= ctx.accounts.transformation.input_weight_grams),
        LastroV2Error::InvalidReservation
    );

    // A stale reservation must be released first so its PDA and counters never orphan.
    require!(
        ctx.accounts.asset.reserved_by == [0; 32],
        LastroV2Error::AssetReserved
    );

    let asset = &mut ctx.accounts.asset;
    asset.reserved_by = transformation_id;
    asset.reserved_weight_grams = weight_grams;
    asset.reserved_until = ctx.accounts.transformation.expires_at;

    let transformation = &mut ctx.accounts.transformation;
    transformation.reserved_input_count = next_count;
    transformation.reserved_input_weight_grams = next_weight;

    let reservation = &mut ctx.accounts.reservation;
    reservation.transformation_id = transformation_id;
    reservation.asset_id = asset_id;
    reservation.weight_grams = weight_grams;
    reservation.expected_state_version = expected_state_version;
    reservation.reserved_until = ctx.accounts.transformation.expires_at;
    reservation.consumed_at = 0;
    reservation.bump = ctx.bumps.reservation;
    Ok(())
}

/// Permissionless once the reservation lapsed or its transformation was aborted/expired,
/// so a lost operator key can never lock an asset. Rent returns to the facility owner.
pub fn release_handler(
    ctx: Context<ReleaseTransformationInput>,
    transformation_id: [u8; 32],
    asset_id: [u8; 32],
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.transformation.status != TRANSFORMATION_STATUS_FINALIZED,
        LastroV2Error::InvalidReservation
    );
    require!(
        ctx.accounts.reservation.transformation_id == transformation_id
            && ctx.accounts.reservation.asset_id == asset_id
            && ctx.accounts.reservation.consumed_at == 0,
        LastroV2Error::InvalidReservation
    );
    let lapsed = now >= ctx.accounts.reservation.reserved_until
        || matches!(
            ctx.accounts.transformation.status,
            TRANSFORMATION_STATUS_ABORTED | TRANSFORMATION_STATUS_EXPIRED
        );
    require!(
        lapsed || ctx.accounts.caller.key() == ctx.accounts.facility.owner,
        LastroV2Error::ReservationNotReleasable
    );
    require!(
        ctx.accounts.asset.reserved_by == transformation_id
            && ctx.accounts.asset.reserved_weight_grams == ctx.accounts.reservation.weight_grams,
        LastroV2Error::InvalidReservation
    );

    let weight_grams = ctx.accounts.reservation.weight_grams;
    let asset = &mut ctx.accounts.asset;
    asset.reserved_by = [0; 32];
    asset.reserved_weight_grams = 0;
    asset.reserved_until = 0;

    let transformation = &mut ctx.accounts.transformation;
    transformation.reserved_input_count = transformation
        .reserved_input_count
        .checked_sub(1)
        .ok_or_else(|| error!(LastroV2Error::InvalidReservation))?;
    transformation.reserved_input_weight_grams = transformation
        .reserved_input_weight_grams
        .checked_sub(weight_grams)
        .ok_or_else(|| error!(LastroV2Error::InvalidReservation))?;
    Ok(())
}

#[derive(Accounts)]
#[instruction(transformation_id: [u8; 32], asset_id: [u8; 32])]
pub struct ReserveTransformationInput<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        address = config.facility_registry,
        seeds = [FACILITY_REGISTRY_SEED, &config.deployment_id],
        bump = facility_registry.bump,
    )]
    pub facility_registry: Account<'info, RegistryRoot>,
    #[account(
        mut,
        seeds = [TRANSFORMATION_SEED, &config.deployment_id, &transformation_id],
        bump = transformation.bump,
    )]
    pub transformation: Account<'info, TransformationAnchor>,
    #[account(
        seeds = [FACILITY_SEED, &config.deployment_id, &transformation.facility_id],
        bump = facility.bump,
    )]
    pub facility: Account<'info, FacilityRecord>,
    #[account(
        mut,
        seeds = [ASSET_SEED, &config.deployment_id, &asset_id],
        bump = asset.bump,
    )]
    pub asset: Account<'info, AssetState>,
    #[account(
        init,
        payer = operator,
        space = TransformationReservation::SPACE,
        seeds = [
            TRANSFORMATION_RESERVATION_SEED,
            &config.deployment_id,
            &transformation_id,
            &asset_id,
        ],
        bump,
    )]
    pub reservation: Account<'info, TransformationReservation>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(transformation_id: [u8; 32], asset_id: [u8; 32])]
pub struct ReleaseTransformationInput<'info> {
    pub caller: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        mut,
        seeds = [TRANSFORMATION_SEED, &config.deployment_id, &transformation_id],
        bump = transformation.bump,
    )]
    pub transformation: Account<'info, TransformationAnchor>,
    #[account(
        seeds = [FACILITY_SEED, &config.deployment_id, &transformation.facility_id],
        bump = facility.bump,
    )]
    pub facility: Account<'info, FacilityRecord>,
    /// CHECK: rent refund destination, fixed to the registered facility owner.
    #[account(mut, address = facility.owner)]
    pub facility_owner: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [ASSET_SEED, &config.deployment_id, &asset_id],
        bump = asset.bump,
    )]
    pub asset: Account<'info, AssetState>,
    #[account(
        mut,
        close = facility_owner,
        seeds = [
            TRANSFORMATION_RESERVATION_SEED,
            &config.deployment_id,
            &transformation_id,
            &asset_id,
        ],
        bump = reservation.bump,
    )]
    pub reservation: Account<'info, TransformationReservation>,
}
