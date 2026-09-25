//! Create one output asset of a transformation.

use anchor_lang::prelude::*;

use crate::{
    constants::{
        ASSET_SEED, ASSET_STATUS_ACTIVE, CONFIG_V2_SEED, TRANSFORMATION_SEED,
        TRANSFORMATION_STATUS_FINALIZING, TRANSFORMATION_STATUS_OPEN, is_valid_asset_type,
    },
    error::LastroV2Error,
    state::{AssetState, ProtocolConfigV2, TransformationAnchor},
};

pub fn create_handler(
    ctx: Context<CreateTransformationOutput>,
    output_id: [u8; 32],
    asset_type: u8,
    custodian: Pubkey,
    lineage_root: [u8; 32],
    weight_grams: u64,
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
    require_nonzero(&output_id)?;
    require!(
        is_valid_asset_type(asset_type),
        LastroV2Error::InvalidAssetType
    );
    require!(custodian != Pubkey::default(), LastroV2Error::InvalidIdentifier);
    require!(
        lineage_root.iter().any(|byte| *byte != 0),
        LastroV2Error::InvalidIdentifier
    );
    require!(weight_grams > 0, LastroV2Error::InvalidWeight);

    let next_count = ctx
        .accounts
        .transformation
        .created_output_count
        .checked_add(1)
        .ok_or_else(|| error!(LastroV2Error::InvalidReservation))?;
    let next_weight = ctx
        .accounts
        .transformation
        .created_output_weight_grams
        .checked_add(weight_grams)
        .ok_or_else(|| error!(LastroV2Error::InvalidWeight))?;
    require!(
        next_count <= ctx.accounts.transformation.output_count
            && next_weight <= ctx.accounts.transformation.output_weight_grams,
        LastroV2Error::InvalidReservation
    );
    require!(
        weight_grams <= ctx.accounts.config.max_asset_weight_grams,
        LastroV2Error::InvalidWeight
    );

    let output = &mut ctx.accounts.output;
    output.asset_id = output_id;
    output.asset_type = asset_type;
    output.status = ASSET_STATUS_ACTIVE;
    output.deployment_id = ctx.accounts.config.deployment_id;
    output.custodian = custodian;
    output.parent_root = ctx.accounts.transformation.input_root;
    output.lineage_root = lineage_root;
    output.current_lot_id = [0; 32];
    output.available_weight_grams = weight_grams;
    output.reserved_weight_grams = 0;
    output.event_sequence = 0;
    output.state_version = 0;
    output.last_event_hash = [0; 32];
    output.reserved_by = [0; 32];
    output.reserved_until = 0;
    output.flags = 0;
    output.bump = ctx.bumps.output;

    let transformation = &mut ctx.accounts.transformation;
    transformation.status = TRANSFORMATION_STATUS_FINALIZING;
    transformation.created_output_count = next_count;
    transformation.created_output_weight_grams = next_weight;
    Ok(())
}

fn require_nonzero(value: &[u8; 32]) -> Result<()> {
    require!(
        value.iter().any(|byte| *byte != 0),
        LastroV2Error::InvalidIdentifier
    );
    Ok(())
}

#[derive(Accounts)]
#[instruction(output_id: [u8; 32])]
pub struct CreateTransformationOutput<'info> {
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
        seeds = [TRANSFORMATION_SEED, &config.deployment_id, &transformation.transformation_id],
        bump = transformation.bump,
    )]
    pub transformation: Account<'info, TransformationAnchor>,
    #[account(
        init,
        payer = authority,
        space = AssetState::SPACE,
        seeds = [ASSET_SEED, &config.deployment_id, &output_id],
        bump,
    )]
    pub output: Account<'info, AssetState>,
    pub system_program: Program<'info, System>,
}
