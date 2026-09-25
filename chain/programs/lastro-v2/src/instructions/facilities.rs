//! Facility registry lifecycle.

use anchor_lang::prelude::*;

use crate::{
    constants::{
        is_valid_facility_status, is_valid_facility_type, FACILITY_SEED, FACILITY_REGISTRY_SEED,
        FACILITY_STATUS_REVOKED, CONFIG_V2_SEED,
    },
    error::LastroV2Error,
    state::{FacilityRecord, ProtocolConfigV2, RegistryRoot},
};

pub fn register_handler(
    ctx: Context<RegisterFacility>,
    facility_id: [u8; 32],
    owner: Pubkey,
    facility_type: u8,
    credential_hash: [u8; 32],
    valid_from: i64,
    valid_until: i64,
) -> Result<()> {
    require_nonzero(&facility_id)?;
    require!(owner != Pubkey::default(), LastroV2Error::InvalidIdentifier);
    require!(is_valid_facility_type(facility_type), LastroV2Error::InvalidFacilityType);
    require!(valid_from >= 0 && valid_until > valid_from, LastroV2Error::InvalidTimeWindow);
    require!(credential_hash.iter().any(|byte| *byte != 0), LastroV2Error::InvalidIdentifier);

    let facility = &mut ctx.accounts.facility;
    facility.facility_id = facility_id;
    facility.owner = owner;
    facility.facility_type = facility_type;
    facility.status = crate::constants::FACILITY_STATUS_ACTIVE;
    facility.credential_hash = credential_hash;
    facility.valid_from = valid_from;
    facility.valid_until = valid_until;
    facility.bump = ctx.bumps.facility;
    Ok(())
}

pub fn set_status_handler(ctx: Context<SetFacilityStatus>, status: u8) -> Result<()> {
    require!(is_valid_facility_status(status), LastroV2Error::InvalidFacilityStatus);
    let facility = &mut ctx.accounts.facility;
    require!(facility.status != FACILITY_STATUS_REVOKED, LastroV2Error::InvalidFacilityStatus);
    facility.status = status;
    Ok(())
}

fn require_nonzero(value: &[u8; 32]) -> Result<()> {
    require!(value.iter().any(|byte| *byte != 0), LastroV2Error::InvalidIdentifier);
    Ok(())
}

#[derive(Accounts)]
#[instruction(facility_id: [u8; 32])]
pub struct RegisterFacility<'info> {
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
        init,
        payer = authority,
        space = FacilityRecord::SPACE,
        seeds = [FACILITY_SEED, &config.deployment_id, &facility_id],
        bump
    )]
    pub facility: Account<'info, FacilityRecord>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetFacilityStatus<'info> {
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
        seeds = [FACILITY_SEED, &config.deployment_id, &facility.facility_id],
        bump = facility.bump,
    )]
    pub facility: Account<'info, FacilityRecord>,
}
