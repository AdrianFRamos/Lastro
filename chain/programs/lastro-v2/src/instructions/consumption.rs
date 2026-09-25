//! Consume one reserved input of a transformation.

use anchor_lang::prelude::*;

use crate::{
    constants::{
        ASSET_SEED, ASSET_STATUS_CONSUMED, ASSET_STATUS_IN_TRANSIT, CONFIG_V2_SEED,
        TRANSFORMATION_RESERVATION_SEED, TRANSFORMATION_SEED, TRANSFORMATION_STATUS_FINALIZING,
        TRANSFORMATION_STATUS_OPEN,
    },
    error::LastroV2Error,
    state::{AssetState, ProtocolConfigV2, TransformationAnchor, TransformationReservation},
};

pub fn consume_handler(
    ctx: Context<ConsumeTransformationInput>,
    transformation_id: [u8; 32],
    asset_id: [u8; 32],
    expected_state_version: u64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        matches!(
            ctx.accounts.transformation.status,
            TRANSFORMATION_STATUS_OPEN | TRANSFORMATION_STATUS_FINALIZING
        ),
        LastroV2Error::TransformationNotOpen
    );
    require!(
        now <= ctx.accounts.transformation.expires_at,
        LastroV2Error::TransformationExpired
    );
    require!(
        ctx.accounts.reservation.transformation_id == transformation_id
            && ctx.accounts.reservation.asset_id == asset_id,
        LastroV2Error::InvalidReservation
    );
    require!(
        ctx.accounts.reservation.expected_state_version == expected_state_version,
        LastroV2Error::InvalidStateVersion
    );
    require!(
        ctx.accounts.asset.state_version == expected_state_version
            && ctx.accounts.asset.reserved_by == transformation_id
            && ctx.accounts.asset.reserved_weight_grams == ctx.accounts.reservation.weight_grams,
        LastroV2Error::InvalidReservation
    );

    let weight_grams = ctx.accounts.reservation.weight_grams;
    let remaining_weight = ctx
        .accounts
        .asset
        .available_weight_grams
        .checked_sub(weight_grams)
        .ok_or_else(|| error!(LastroV2Error::InvalidWeight))?;
    let next_state_version = expected_state_version
        .checked_add(1)
        .ok_or_else(|| error!(LastroV2Error::InvalidStateVersion))?;
    let next_consumed_count = ctx
        .accounts
        .transformation
        .consumed_input_count
        .checked_add(1)
        .ok_or_else(|| error!(LastroV2Error::InvalidReservation))?;
    let next_consumed_weight = ctx
        .accounts
        .transformation
        .consumed_input_weight_grams
        .checked_add(weight_grams)
        .ok_or_else(|| error!(LastroV2Error::InvalidWeight))?;
    require!(
        next_consumed_count <= ctx.accounts.transformation.input_count
            && next_consumed_weight <= ctx.accounts.transformation.input_weight_grams,
        LastroV2Error::InvalidReservation
    );

    let asset = &mut ctx.accounts.asset;
    asset.available_weight_grams = remaining_weight;
    asset.reserved_weight_grams = 0;
    asset.reserved_by = [0; 32];
    asset.reserved_until = 0;
    asset.state_version = next_state_version;
    if remaining_weight == 0 {
        asset.status = ASSET_STATUS_CONSUMED;
    } else if asset.status != ASSET_STATUS_IN_TRANSIT {
        asset.status = asset.status.max(1);
    }

    let transformation = &mut ctx.accounts.transformation;
    transformation.status = TRANSFORMATION_STATUS_FINALIZING;
    transformation.reserved_input_count = transformation
        .reserved_input_count
        .checked_sub(1)
        .ok_or_else(|| error!(LastroV2Error::InvalidReservation))?;
    transformation.reserved_input_weight_grams = transformation
        .reserved_input_weight_grams
        .checked_sub(weight_grams)
        .ok_or_else(|| error!(LastroV2Error::InvalidReservation))?;
    transformation.consumed_input_count = next_consumed_count;
    transformation.consumed_input_weight_grams = next_consumed_weight;
    Ok(())
}

#[derive(Accounts)]
#[instruction(transformation_id: [u8; 32], asset_id: [u8; 32])]
pub struct ConsumeTransformationInput<'info> {
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
