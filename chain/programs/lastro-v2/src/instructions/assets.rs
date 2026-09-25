//! Generic asset registration for the first v2 increment.

use anchor_lang::prelude::*;

use crate::{
    constants::{is_valid_asset_type, ASSET_SEED, ASSET_STATUS_ACTIVE, CONFIG_V2_SEED},
    error::LastroV2Error,
    state::{AssetState, ProtocolConfigV2},
};

pub fn register_handler(
    ctx: Context<RegisterAsset>,
    asset_id: [u8; 32],
    asset_type: u8,
    custodian: Pubkey,
    parent_root: [u8; 32],
    lineage_root: [u8; 32],
    available_weight_grams: u64,
) -> Result<()> {
    require_nonzero(&asset_id)?;
    require!(is_valid_asset_type(asset_type), LastroV2Error::InvalidAssetType);
    require!(custodian != Pubkey::default(), LastroV2Error::InvalidIdentifier);
    require!(
        available_weight_grams <= ctx.accounts.config.max_asset_weight_grams,
        LastroV2Error::InvalidWeight
    );

    let asset = &mut ctx.accounts.asset;
    asset.asset_id = asset_id;
    asset.asset_type = asset_type;
    asset.status = ASSET_STATUS_ACTIVE;
    asset.deployment_id = ctx.accounts.config.deployment_id;
    asset.custodian = custodian;
    asset.parent_root = parent_root;
    asset.lineage_root = lineage_root;
    asset.current_lot_id = [0u8; 32];
    asset.available_weight_grams = available_weight_grams;
    asset.event_sequence = 0;
    asset.state_version = 0;
    asset.last_event_hash = [0u8; 32];
    asset.reserved_by = [0u8; 32];
    asset.reserved_until = 0;
    asset.flags = 0;
    asset.bump = ctx.bumps.asset;
    Ok(())
}

fn require_nonzero(value: &[u8; 32]) -> Result<()> {
    require!(value.iter().any(|byte| *byte != 0), LastroV2Error::InvalidIdentifier);
    Ok(())
}

#[derive(Accounts)]
#[instruction(asset_id: [u8; 32])]
pub struct RegisterAsset<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
        has_one = authority,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        init,
        payer = authority,
        space = AssetState::SPACE,
        seeds = [ASSET_SEED, &config.deployment_id, &asset_id],
        bump
    )]
    pub asset: Account<'info, AssetState>,
    pub system_program: Program<'info, System>,
}
