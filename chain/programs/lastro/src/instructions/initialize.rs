//! Create the immutable ProtocolConfig for one deployment.

use anchor_lang::prelude::*;
use p256::PublicKey;

use crate::{constants::PROTOCOL_CONFIG_SEED, error::LastroError, state::ProtocolConfig};

pub fn handler(
    ctx: Context<Initialize>,
    deployment_id: [u8; 32],
    station_pubkey33: [u8; 33],
) -> Result<()> {
    require!(
        matches!(station_pubkey33[0], 0x02 | 0x03),
        LastroError::InvalidEvent
    );
    PublicKey::from_sec1_bytes(&station_pubkey33).map_err(|_| error!(LastroError::InvalidEvent))?;

    let config = &mut ctx.accounts.protocol_config;
    config.authority = ctx.accounts.authority.key();
    config.deployment_id = deployment_id;
    config.station_pubkey33 = station_pubkey33;
    config.bump = ctx.bumps.protocol_config;
    Ok(())
}

#[derive(Accounts)]
#[instruction(deployment_id: [u8; 32])]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = ProtocolConfig::SPACE,
        seeds = [PROTOCOL_CONFIG_SEED, &deployment_id],
        bump
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,
    pub system_program: Program<'info, System>,
}
