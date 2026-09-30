//! Physical observation admission and atomic anchoring.

use anchor_lang::prelude::*;
use lastro_protocol::v2::{DomainEventEnvelope, EventType, V2_ENVELOPE_LEN};

use crate::{
    constants::{ASSET_SEED, CONFIG_V2_SEED, EVENT_SEED, STATION_REGISTRY_SEED, STATION_V2_SEED},
    error::LastroV2Error,
    state::{AssetState, EventAnchor, ProtocolConfigV2, RegistryRoot, StationRecord},
    verify::{parse_station_event, verify_station_precompile_binding},
};

pub fn handler(
    ctx: Context<RecordObservation>,
    subject_id: [u8; 32],
    event_id: [u8; 32],
    station_id: [u8; 32],
    event: [u8; V2_ENVELOPE_LEN],
) -> Result<()> {
    let (envelope, event_hash) = admit_station_event(
        StationEventContext {
            config: &ctx.accounts.config,
            station: &ctx.accounts.station,
            asset: &ctx.accounts.asset,
            instructions_sysvar: &ctx.accounts.instructions_sysvar,
        },
        EventType::ObservationRecorded,
        subject_id,
        event_id,
        station_id,
        &event,
        0,
    )?;
    apply_station_event(
        &mut ctx.accounts.asset,
        &mut ctx.accounts.event_anchor,
        &envelope,
        event_hash,
        ctx.bumps.event_anchor,
    );
    Ok(())
}

pub struct StationEventContext<'a, 'info> {
    pub config: &'a ProtocolConfigV2,
    pub station: &'a StationRecord,
    pub asset: &'a AssetState,
    pub instructions_sysvar: &'a UncheckedAccount<'info>,
}

/// Admits one Station-signed envelope for `asset`: active Station inside its validity
/// window, Secp256r1 proof over the exact envelope bytes, expected event type, exact
/// predecessor/version, time window, deployment age bound and a non-terminal asset.
pub fn admit_station_event(
    ctx: StationEventContext<'_, '_>,
    expected_type: EventType,
    subject_id: [u8; 32],
    event_id: [u8; 32],
    station_id: [u8; 32],
    event: &[u8; V2_ENVELOPE_LEN],
    trailing_args_len: usize,
) -> Result<(DomainEventEnvelope, [u8; 32])> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.station.status == crate::constants::STATION_STATUS_ACTIVE,
        LastroV2Error::InvalidStationProof
    );
    require!(
        now >= ctx.station.valid_from && now <= ctx.station.valid_until,
        LastroV2Error::InvalidStationProof
    );
    verify_station_precompile_binding(
        ctx.instructions_sysvar,
        &ctx.station.pubkey33,
        event,
        trailing_args_len,
    )?;
    let envelope = parse_station_event(
        event,
        expected_type,
        ctx.config.deployment_id,
        subject_id,
        event_id,
        station_id,
        ctx.asset,
        now,
    )?;
    require!(!ctx.asset.is_terminal(), LastroV2Error::InvalidAssetStatus);
    // The protocol caps event windows at 24 h; each deployment may tighten that bound.
    require!(
        (envelope.expires_at - envelope.observed_at) as u64 <= ctx.config.max_event_age_seconds,
        LastroV2Error::EventOutsideValidityWindow
    );
    let event_hash = envelope
        .event_hash()
        .map_err(|_| error!(LastroV2Error::InvalidDomainEvent))?;
    Ok((envelope, event_hash))
}

/// Advances the asset's event chain and writes the immutable event anchor.
pub fn apply_station_event(
    asset: &mut AssetState,
    anchor: &mut EventAnchor,
    envelope: &DomainEventEnvelope,
    event_hash: [u8; 32],
    anchor_bump: u8,
) {
    asset.event_sequence = asset.event_sequence.saturating_add(1);
    asset.state_version = envelope.state_version;
    asset.last_event_hash = event_hash;

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
    anchor.bump = anchor_bump;
}

#[derive(Accounts)]
#[instruction(subject_id: [u8; 32], event_id: [u8; 32], station_id: [u8; 32], event: [u8; V2_ENVELOPE_LEN])]
pub struct RecordObservation<'info> {
    /// The deployment authority or the asset's current custodian. Letting the custodian anchor
    /// presence proofs keeps the deployment authority key out of day-to-day operation.
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
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
        constraint = authority.key() == config.authority || authority.key() == asset.custodian
            @ LastroV2Error::UnauthorizedActor,
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
