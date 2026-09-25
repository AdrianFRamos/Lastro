//! Physical observation admission and atomic anchoring.

use anchor_lang::prelude::*;
use lastro_protocol::v2::V2_ENVELOPE_LEN;

use crate::{
    constants::{ASSET_SEED, CONFIG_V2_SEED, EVENT_SEED, STATION_REGISTRY_SEED, STATION_V2_SEED},
    error::LastroV2Error,
    state::{AssetState, EventAnchor, ProtocolConfigV2, RegistryRoot, StationRecord},
    verify::{parse_observation, verify_station_precompile_binding},
};

pub fn handler(
    ctx: Context<RecordObservation>,
    subject_id: [u8; 32],
    event_id: [u8; 32],
    station_id: [u8; 32],
    event: [u8; V2_ENVELOPE_LEN],
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.station.status == crate::constants::STATION_STATUS_ACTIVE,
        LastroV2Error::InvalidStationProof
    );
    require!(
        now >= ctx.accounts.station.valid_from && now <= ctx.accounts.station.valid_until,
        LastroV2Error::InvalidStationProof
    );

    verify_station_precompile_binding(
        &ctx.accounts.instructions_sysvar,
        &ctx.accounts.station.pubkey33,
        &event,
    )?;
    let envelope = parse_observation(
        &event,
        ctx.accounts.config.deployment_id,
        subject_id,
        event_id,
        station_id,
        &ctx.accounts.asset,
        now,
    )?;
    let event_hash = envelope
        .event_hash()
        .map_err(|_| error!(LastroV2Error::InvalidDomainEvent))?;

    let asset = &mut ctx.accounts.asset;
    asset.event_sequence = asset.event_sequence.saturating_add(1);
    asset.state_version = envelope.state_version;
    asset.last_event_hash = event_hash;

    let anchor = &mut ctx.accounts.event_anchor;
    anchor.event_id = envelope.event_id;
    anchor.deployment_id = envelope.deployment_id;
    anchor.subject_id = envelope.subject_id;
    anchor.source_id = envelope.source_id;
    anchor.event_type = envelope.event_type;
    anchor.state_version = envelope.state_version;
    anchor.observed_at = envelope.observed_at;
    anchor.expires_at = envelope.expires_at;
    anchor.expected_previous_hash = envelope.expected_previous_hash;
    anchor.payload_hash = envelope.payload_hash;
    anchor.event_hash = event_hash;
    anchor.bump = ctx.bumps.event_anchor;
    Ok(())
}

#[derive(Accounts)]
#[instruction(subject_id: [u8; 32], event_id: [u8; 32], station_id: [u8; 32], event: [u8; V2_ENVELOPE_LEN])]
pub struct RecordObservation<'info> {
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
        seeds = [STATION_V2_SEED, &config.deployment_id, &station_id],
        bump = station.bump,
    )]
    pub station: Account<'info, StationRecord>,
    #[account(
        mut,
        seeds = [ASSET_SEED, &config.deployment_id, &subject_id],
        bump = asset.bump,
    )]
    pub asset: Account<'info, AssetState>,
    #[account(
        init,
        payer = authority,
        space = EventAnchor::SPACE,
        seeds = [EVENT_SEED, &config.deployment_id, &event_id],
        bump,
    )]
    pub event_anchor: Account<'info, EventAnchor>,
    /// CHECK: Anchor validates this address and the verifier reads its instruction list.
    #[account(address = solana_sdk_ids::sysvar::instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
