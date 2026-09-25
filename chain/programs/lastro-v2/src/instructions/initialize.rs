//! Initialize one immutable deployment-scoped configuration and its registries.

use anchor_lang::prelude::*;

use crate::{
    constants::{
        CONFIG_V2_SEED, DOCUMENT_REGISTRY_SEED, FACILITY_REGISTRY_SEED, PARTY_REGISTRY_SEED,
        SCHEMA_VERSION, STATION_REGISTRY_SEED,
    },
    error::LastroV2Error,
    state::{ProtocolConfigV2, RegistryRoot},
};

pub fn handler(
    ctx: Context<InitializeV2>,
    deployment_id: [u8; 32],
    schema_version: u16,
    max_asset_weight_grams: u64,
    mass_tolerance_basis_points: u16,
    max_event_age_seconds: u64,
) -> Result<()> {
    require!(deployment_id.iter().any(|byte| *byte != 0), LastroV2Error::InvalidDeployment);
    require!(schema_version == SCHEMA_VERSION, LastroV2Error::UnsupportedSchemaVersion);
    require!(max_asset_weight_grams > 0, LastroV2Error::InvalidWeight);
    require!(mass_tolerance_basis_points <= 10_000, LastroV2Error::InvalidWeight);
    require!(max_event_age_seconds > 0, LastroV2Error::InvalidTimeWindow);

    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.deployment_id = deployment_id;
    config.schema_version = schema_version;
    config.station_registry = ctx.accounts.station_registry.key();
    config.facility_registry = ctx.accounts.facility_registry.key();
    config.party_registry = ctx.accounts.party_registry.key();
    config.document_registry = ctx.accounts.document_registry.key();
    config.max_asset_weight_grams = max_asset_weight_grams;
    config.mass_tolerance_basis_points = mass_tolerance_basis_points;
    config.max_event_age_seconds = max_event_age_seconds;
    config.bump = ctx.bumps.config;

    initialize_registry(&mut ctx.accounts.station_registry, deployment_id, 1, ctx.bumps.station_registry);
    initialize_registry(&mut ctx.accounts.facility_registry, deployment_id, 2, ctx.bumps.facility_registry);
    initialize_registry(&mut ctx.accounts.party_registry, deployment_id, 3, ctx.bumps.party_registry);
    initialize_registry(&mut ctx.accounts.document_registry, deployment_id, 4, ctx.bumps.document_registry);
    Ok(())
}

fn initialize_registry(root: &mut Account<RegistryRoot>, deployment_id: [u8; 32], registry_type: u8, bump: u8) {
    root.deployment_id = deployment_id;
    root.registry_type = registry_type;
    root.bump = bump;
}

#[derive(Accounts)]
#[instruction(deployment_id: [u8; 32])]
pub struct InitializeV2<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = ProtocolConfigV2::SPACE,
        seeds = [CONFIG_V2_SEED, &deployment_id],
        bump
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        init,
        payer = authority,
        space = RegistryRoot::SPACE,
        seeds = [STATION_REGISTRY_SEED, &deployment_id],
        bump
    )]
    pub station_registry: Account<'info, RegistryRoot>,
    #[account(
        init,
        payer = authority,
        space = RegistryRoot::SPACE,
        seeds = [FACILITY_REGISTRY_SEED, &deployment_id],
        bump
    )]
    pub facility_registry: Account<'info, RegistryRoot>,
    #[account(
        init,
        payer = authority,
        space = RegistryRoot::SPACE,
        seeds = [PARTY_REGISTRY_SEED, &deployment_id],
        bump
    )]
    pub party_registry: Account<'info, RegistryRoot>,
    #[account(
        init,
        payer = authority,
        space = RegistryRoot::SPACE,
        seeds = [DOCUMENT_REGISTRY_SEED, &deployment_id],
        bump
    )]
    pub document_registry: Account<'info, RegistryRoot>,
    pub system_program: Program<'info, System>,
}
