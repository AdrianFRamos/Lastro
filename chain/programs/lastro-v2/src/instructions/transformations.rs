//! Begin a facility transformation and freeze its canonical commitments.

use anchor_lang::prelude::*;
use lastro_protocol::v2::TransformationManifest;

use crate::{
    constants::{
        CONFIG_V2_SEED, FACILITY_REGISTRY_SEED, FACILITY_SEED, FACILITY_STATUS_ACTIVE,
        FACILITY_TYPE_PROCESSING_FACILITY, TRANSFORMATION_SEED, TRANSFORMATION_STATUS_ABORTED,
        TRANSFORMATION_STATUS_EXPIRED, TRANSFORMATION_STATUS_FINALIZED,
        TRANSFORMATION_STATUS_FINALIZING, TRANSFORMATION_STATUS_OPEN,
    },
    error::LastroV2Error,
    state::{FacilityRecord, ProtocolConfigV2, RegistryRoot, TransformationAnchor},
};

#[allow(clippy::too_many_arguments)]
pub fn begin_handler(
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
    let now = Clock::get()?.unix_timestamp;
    require_nonzero(&transformation_id)?;
    require_nonzero(&facility_id)?;
    require!(
        ctx.accounts.facility.status == FACILITY_STATUS_ACTIVE,
        LastroV2Error::InvalidFacilityStatus
    );
    require!(
        ctx.accounts.facility.facility_type == FACILITY_TYPE_PROCESSING_FACILITY,
        LastroV2Error::InvalidFacilityType
    );
    require!(
        ctx.accounts.facility.owner == ctx.accounts.authority.key(),
        LastroV2Error::UnauthorizedFacility
    );
    require!(
        expires_at > now && expires_at - now <= ctx.accounts.config.max_event_age_seconds as i64,
        LastroV2Error::InvalidTimeWindow
    );

    let manifest = TransformationManifest {
        transformation_id,
        facility_id,
        transformation_type,
        input_root,
        output_root,
        input_count,
        output_count,
        mass: lastro_protocol::v2::MassBalance {
            input_weight_grams,
            output_weight_grams,
            byproduct_weight_grams,
            loss_weight_grams,
            tolerance_basis_points,
        },
        manifest_nonce,
        expires_at,
    };
    let calculated_hash = manifest
        .manifest_hash()
        .map_err(|_| error!(LastroV2Error::InvalidTransformationManifest))?;
    require!(
        calculated_hash == manifest_hash,
        LastroV2Error::ManifestHashMismatch
    );
    require!(
        tolerance_basis_points <= ctx.accounts.config.mass_tolerance_basis_points,
        LastroV2Error::InvalidWeight
    );

    let transformation = &mut ctx.accounts.transformation;
    transformation.transformation_id = transformation_id;
    transformation.facility_id = facility_id;
    transformation.transformation_type = transformation_type;
    transformation.input_root = input_root;
    transformation.output_root = output_root;
    transformation.input_count = input_count;
    transformation.output_count = output_count;
    transformation.reserved_input_count = 0;
    transformation.consumed_input_count = 0;
    transformation.created_output_count = 0;
    transformation.input_weight_grams = input_weight_grams;
    transformation.reserved_input_weight_grams = 0;
    transformation.consumed_input_weight_grams = 0;
    transformation.output_weight_grams = output_weight_grams;
    transformation.created_output_weight_grams = 0;
    transformation.byproduct_weight_grams = byproduct_weight_grams;
    transformation.loss_weight_grams = loss_weight_grams;
    transformation.tolerance_basis_points = tolerance_basis_points;
    transformation.status = TRANSFORMATION_STATUS_OPEN;
    transformation.manifest_hash = manifest_hash;
    transformation.sequence = 0;
    transformation.expires_at = expires_at;
    transformation.bump = ctx.bumps.transformation;
    Ok(())
}

pub fn abort_handler(ctx: Context<AbortTransformation>) -> Result<()> {
    require!(
        matches!(
            ctx.accounts.transformation.status,
            TRANSFORMATION_STATUS_OPEN | TRANSFORMATION_STATUS_FINALIZING
        ),
        LastroV2Error::TransformationNotAbortable
    );
    ctx.accounts.transformation.status = TRANSFORMATION_STATUS_ABORTED;
    Ok(())
}

pub fn expire_handler(ctx: Context<ExpireTransformation>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        now > ctx.accounts.transformation.expires_at,
        LastroV2Error::TransformationNotExpired
    );
    require!(
        matches!(
            ctx.accounts.transformation.status,
            TRANSFORMATION_STATUS_OPEN | TRANSFORMATION_STATUS_FINALIZING
        ),
        LastroV2Error::TransformationNotAbortable
    );
    ctx.accounts.transformation.status = TRANSFORMATION_STATUS_EXPIRED;
    Ok(())
}

pub fn finalize_handler(ctx: Context<FinalizeTransformation>) -> Result<()> {
    require!(
        matches!(
            ctx.accounts.transformation.status,
            TRANSFORMATION_STATUS_OPEN | TRANSFORMATION_STATUS_FINALIZING
        ),
        LastroV2Error::TransformationNotFinalizable
    );
    require!(
        ctx.accounts.transformation.reserved_input_count == 0
            && ctx.accounts.transformation.reserved_input_weight_grams == 0
            && ctx.accounts.transformation.consumed_input_count
                == ctx.accounts.transformation.input_count
            && ctx.accounts.transformation.consumed_input_weight_grams
                == ctx.accounts.transformation.input_weight_grams
            && ctx.accounts.transformation.created_output_count
                == ctx.accounts.transformation.output_count
            && ctx.accounts.transformation.created_output_weight_grams
                == ctx.accounts.transformation.output_weight_grams,
        LastroV2Error::TransformationNotComplete
    );
    let transformation = &mut ctx.accounts.transformation;
    transformation.status = TRANSFORMATION_STATUS_FINALIZED;
    transformation.sequence = transformation
        .sequence
        .checked_add(1)
        .ok_or_else(|| error!(LastroV2Error::InvalidStateVersion))?;
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
#[instruction(transformation_id: [u8; 32], facility_id: [u8; 32])]
pub struct BeginTransformation<'info> {
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
        seeds = [FACILITY_SEED, &config.deployment_id, &facility_id],
        bump = facility.bump,
    )]
    pub facility: Account<'info, FacilityRecord>,
    #[account(
        init,
        payer = authority,
        space = TransformationAnchor::SPACE,
        seeds = [TRANSFORMATION_SEED, &config.deployment_id, &transformation_id],
        bump,
    )]
    pub transformation: Account<'info, TransformationAnchor>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AbortTransformation<'info> {
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
}

#[derive(Accounts)]
pub struct ExpireTransformation<'info> {
    pub caller: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        mut,
        seeds = [TRANSFORMATION_SEED, &config.deployment_id, &transformation.transformation_id],
        bump = transformation.bump,
    )]
    pub transformation: Account<'info, TransformationAnchor>,
}

#[derive(Accounts)]
pub struct FinalizeTransformation<'info> {
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
}
