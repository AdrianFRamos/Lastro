//! Station registry lifecycle for physical observations.

use anchor_lang::prelude::*;
use p256::PublicKey;
use sha2::{Digest, Sha256};

use crate::{
    constants::{
        is_valid_station_status, CONFIG_V2_SEED, STATION_REGISTRY_SEED, STATION_STATUS_ACTIVE,
        STATION_STATUS_EXPIRED, STATION_STATUS_REVOKED, STATION_V2_SEED,
    },
    error::LastroV2Error,
    state::{ProtocolConfigV2, RegistryRoot, StationRecord},
};

const STATION_DOMAIN: &[u8] = b"LASTRO_STATION\0";

pub fn register_handler(
    ctx: Context<RegisterStation>,
    station_id: [u8; 32],
    key_id: [u8; 32],
    pubkey33: [u8; 33],
    valid_from: i64,
    valid_until: i64,
    firmware_hash: [u8; 32],
) -> Result<()> {
    require_nonzero(&station_id)?;
    require_nonzero(&key_id)?;
    require!(matches!(pubkey33[0], 0x02 | 0x03), LastroV2Error::InvalidPublicKey);
    PublicKey::from_sec1_bytes(&pubkey33).map_err(|_| error!(LastroV2Error::InvalidPublicKey))?;
    require!(derive_station_id(&pubkey33) == station_id, LastroV2Error::InvalidIdentifier);
    require!(valid_from >= 0 && valid_until > valid_from, LastroV2Error::InvalidTimeWindow);

    let station = &mut ctx.accounts.station;
    station.station_id = station_id;
    station.key_id = key_id;
    station.pubkey33 = pubkey33;
    station.status = STATION_STATUS_ACTIVE;
    station.valid_from = valid_from;
    station.valid_until = valid_until;
    station.firmware_hash = firmware_hash;
    station.bump = ctx.bumps.station;
    Ok(())
}

pub fn set_status_handler(ctx: Context<SetStationStatus>, status: u8) -> Result<()> {
    require!(is_valid_station_status(status), LastroV2Error::InvalidStationStatus);
    let station = &mut ctx.accounts.station;
    require!(station.status != STATION_STATUS_REVOKED, LastroV2Error::InvalidStationStatus);
    require!(status != STATION_STATUS_ACTIVE || station.status != STATION_STATUS_EXPIRED, LastroV2Error::InvalidStationStatus);
    station.status = status;
    Ok(())
}

pub fn derive_station_id(pubkey33: &[u8; 33]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(STATION_DOMAIN);
    hasher.update(pubkey33);
    hasher.finalize().into()
}

fn require_nonzero(value: &[u8; 32]) -> Result<()> {
    require!(value.iter().any(|byte| *byte != 0), LastroV2Error::InvalidIdentifier);
    Ok(())
}

#[derive(Accounts)]
#[instruction(station_id: [u8; 32])]
pub struct RegisterStation<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
        has_one = authority,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        address = config.station_registry,
        seeds = [STATION_REGISTRY_SEED, &config.deployment_id],
        bump = station_registry.bump,
    )]
    pub station_registry: Account<'info, RegistryRoot>,
    #[account(
        init,
        payer = authority,
        space = StationRecord::SPACE,
        seeds = [STATION_V2_SEED, &config.deployment_id, &station_id],
        bump
    )]
    pub station: Account<'info, StationRecord>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetStationStatus<'info> {
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
        has_one = authority,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        address = config.station_registry,
        seeds = [STATION_REGISTRY_SEED, &config.deployment_id],
        bump = station_registry.bump,
    )]
    pub station_registry: Account<'info, RegistryRoot>,
    #[account(
        mut,
        seeds = [STATION_V2_SEED, &config.deployment_id, &station.station_id],
        bump = station.bump,
    )]
    pub station: Account<'info, StationRecord>,
}
