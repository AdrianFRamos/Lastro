//! Party and role registry for v2 operations.

use anchor_lang::prelude::*;

use crate::{
    constants::{CONFIG_V2_SEED, PARTY_REGISTRY_SEED, PARTY_SEED, is_valid_party_role},
    error::LastroV2Error,
    state::{PartyRecord, ProtocolConfigV2, RegistryRoot},
};

const PARTY_STATUS_ACTIVE: u8 = 1;

pub fn register_handler(
    ctx: Context<RegisterParty>,
    party_id: [u8; 32],
    wallet: Pubkey,
    role: u16,
) -> Result<()> {
    require_nonzero(&party_id)?;
    require!(
        wallet != Pubkey::default(),
        LastroV2Error::InvalidIdentifier
    );
    require!(is_valid_party_role(role), LastroV2Error::InvalidPartyRole);

    let party = &mut ctx.accounts.party;
    party.party_id = party_id;
    party.wallet = wallet;
    party.role = role;
    party.status = PARTY_STATUS_ACTIVE;
    party.bump = ctx.bumps.party;
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
#[instruction(party_id: [u8; 32])]
pub struct RegisterParty<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
        has_one = authority,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        address = config.party_registry,
        seeds = [PARTY_REGISTRY_SEED, &config.deployment_id],
        bump = party_registry.bump,
    )]
    pub party_registry: Account<'info, RegistryRoot>,
    #[account(
        init,
        payer = authority,
        space = PartyRecord::SPACE,
        seeds = [PARTY_SEED, &config.deployment_id, &party_id],
        bump
    )]
    pub party: Account<'info, PartyRecord>,
    pub system_program: Program<'info, System>,
}
