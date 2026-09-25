//! Reserve an asset balance for one open transformation.

use anchor_lang::prelude::*;

use crate::{
    constants::{
        ASSET_SEED, ASSET_STATUS_ACTIVE, ASSET_STATUS_IN_TRANSIT, CONFIG_V2_SEED,
        FACILITY_REGISTRY_SEED, FACILITY_SEED, FACILITY_STATUS_ACTIVE,
        TRANSFORMATION_RESERVATION_SEED, TRANSFORMATION_SEED, TRANSFORMATION_STATUS_ABORTED,
        TRANSFORMATION_STATUS_EXPIRED, TRANSFORMATION_STATUS_FINALIZED, TRANSFORMATION_STATUS_OPEN,
    },
    error::LastroV2Error,
    state::{
        AssetState, FacilityRecord, ProtocolConfigV2, RegistryRoot, TransformationAnchor,
        TransformationReservation,
    },
};

pub fn reserve_handler(
    ctx: Context<ReserveTransformationInput>,
    transformation_id: [u8; 32],
    asset_id: [u8; 32],
    weight_grams: u64,
    expected_state_version: u64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.transformation.status == TRANSFORMATION_STATUS_OPEN,
        LastroV2Error::TransformationNotOpen
    );
    require!(
        now <= ctx.accounts.transformation.expires_at,
        LastroV2Error::TransformationExpired
    );
    require!(
        ctx.accounts.facility.status == FACILITY_STATUS_ACTIVE,
        LastroV2Error::InvalidFacilityStatus
    );
    require!(
        ctx.accounts.facility.owner == ctx.accounts.authority.key(),
        LastroV2Error::UnauthorizedFacility
    );
    require!(
        ctx.accounts.transformation.facility_id == ctx.accounts.facility.facility_id,
        LastroV2Error::InvalidReservation
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
    require!(
        next_count <= ctx.accounts.transformation.input_count
            && next_weight <= ctx.accounts.transformation.input_weight_grams,
        LastroV2Error::InvalidReservation
    );

    if ctx.accounts.asset.reserved_by != [0; 32] && ctx.accounts.asset.reserved_until >= now {
        return Err(error!(LastroV2Error::AssetReserved));
    }

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
    reservation.bump = ctx.bumps.reservation;
    Ok(())
}

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
            && ctx.accounts.reservation.asset_id == asset_id,
        LastroV2Error::InvalidReservation
    );
    require!(
        now >= ctx.accounts.reservation.reserved_until
            || matches!(
                ctx.accounts.transformation.status,
                TRANSFORMATION_STATUS_ABORTED | TRANSFORMATION_STATUS_EXPIRED
            ),
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
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
        has_one = authority,
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
        payer = authority,
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
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
        has_one = authority,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        mut,
        seeds = [TRANSFORMATION_SEED, &config.deployment_id, &transformation_id],
        bump = transformation.bump,
    )]
    pub transformation: Account<'info, TransformationAnchor>,
    #[account(
        mut,
        seeds = [ASSET_SEED, &config.deployment_id, &asset_id],
        bump = asset.bump,
    )]
    pub asset: Account<'info, AssetState>,
    #[account(
        mut,
        close = authority,
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
