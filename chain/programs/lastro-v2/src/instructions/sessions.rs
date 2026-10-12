//! Handling sessions: one custodian signature authorizes a Station for a bounded window, and
//! the session's presence proofs are anchored as Merkle batches (SPEC v2.1, S3/S4a/S8).
//!
//! The program does not verify each leaf's Station signature: that is the explicit trade-off
//! of batching. It enforces the session terms (Station, window, capacity, revocation and the
//! offline anchoring deadline); the API and the independent verifier check every leaf.
//! Identity changes and custody transfers never run under a session.

use anchor_lang::prelude::*;
use lastro_protocol::v2::{
    MAX_BATCH_EVENT_AGE_SECONDS, MAX_BATCH_LEAVES, MAX_SESSION_EVENTS, MAX_SESSION_SECONDS,
    SESSION_ACTION_OBSERVE, session_id as derive_session_id,
};

use crate::{
    constants::{
        BATCH_SEED, CONFIG_V2_SEED, FACILITY_SEED, FACILITY_STATUS_ACTIVE, SESSION_SEED,
        SESSION_STATUS_OPEN, SESSION_STATUS_REVOKED, STATION_REGISTRY_SEED, STATION_STATUS_ACTIVE,
        STATION_V2_SEED,
    },
    error::LastroV2Error,
    state::{
        BatchAnchor, FacilityRecord, ProtocolConfigV2, RegistryRoot, SessionGrant, StationRecord,
    },
};

/// A session may start slightly before the transaction lands (clock skew), never earlier.
const MAX_BACKDATE_SECONDS: i64 = 600;

#[allow(clippy::too_many_arguments)]
pub fn open_handler(
    ctx: Context<OpenSession>,
    session_id: [u8; 32],
    station_id: [u8; 32],
    allowed_actions: u16,
    max_events: u32,
    valid_from: i64,
    expires_at: i64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let station = &ctx.accounts.station;
    // The id binds custodian, Station and terms (M1): nobody else can occupy it first.
    require!(
        session_id
            == derive_session_id(
                &ctx.accounts.config.deployment_id,
                &ctx.accounts.custodian.key().to_bytes(),
                &station_id,
                valid_from,
                expires_at,
                max_events,
            ),
        LastroV2Error::InvalidSession
    );
    require!(
        allowed_actions == SESSION_ACTION_OBSERVE
            && (1..=MAX_SESSION_EVENTS).contains(&max_events)
            && valid_from < expires_at
            && expires_at.saturating_sub(valid_from) <= MAX_SESSION_SECONDS
            && valid_from >= now - MAX_BACKDATE_SECONDS
            && expires_at > now,
        LastroV2Error::InvalidSession
    );
    // The Station must stay registered and valid for the whole window, installed at an
    // ACTIVE facility that is itself valid for the window (S5).
    require!(
        station.status == STATION_STATUS_ACTIVE
            && valid_from >= station.valid_from
            && expires_at <= station.valid_until,
        LastroV2Error::InvalidStationProof
    );
    let facility = &ctx.accounts.facility;
    require!(
        station.facility_id != [0; 32]
            && facility.facility_id == station.facility_id
            && facility.status == FACILITY_STATUS_ACTIVE
            && valid_from >= facility.valid_from
            && expires_at <= facility.valid_until,
        LastroV2Error::InvalidFacilityStatus
    );

    let session = &mut ctx.accounts.session;
    session.session_id = session_id;
    session.custodian = ctx.accounts.custodian.key();
    session.station_id = station_id;
    session.allowed_actions = allowed_actions;
    session.max_events = max_events;
    session.events_used = 0;
    session.batch_count = 0;
    session.valid_from = valid_from;
    session.expires_at = expires_at;
    session.revoked_at = 0;
    session.status = SESSION_STATUS_OPEN;
    session.rent_payer = ctx.accounts.payer.key();
    session.facility_id = station.facility_id;
    session.bump = ctx.bumps.session;
    Ok(())
}

/// The custodian stops the session: observations after this instant are no longer admitted.
pub fn revoke_handler(ctx: Context<RevokeSession>) -> Result<()> {
    let session = &mut ctx.accounts.session;
    require!(
        session.status == SESSION_STATUS_OPEN,
        LastroV2Error::SessionNotOpen
    );
    session.status = SESSION_STATUS_REVOKED;
    session.revoked_at = Clock::get()?.unix_timestamp;
    Ok(())
}

/// Closes the grant and refunds its rent. The custodian may close at any time (abandoning
/// anything not yet anchored); anyone may close once the offline anchoring window is over.
/// Anchored batches keep the session terms, so verification does not need the grant.
pub fn close_handler(ctx: Context<CloseSession>) -> Result<()> {
    let session = &ctx.accounts.session;
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.caller.key() == session.custodian
            || now > session.observation_deadline() + MAX_BATCH_EVENT_AGE_SECONDS,
        LastroV2Error::UnauthorizedActor
    );
    Ok(())
}

pub fn anchor_batch_handler(
    ctx: Context<AnchorObservationBatch>,
    merkle_root: [u8; 32],
    leaf_count: u32,
    first_observed_at: i64,
    last_observed_at: i64,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    // A Station that was revoked or suspended anchors nothing more: evidence still waiting
    // offline cannot be told apart from evidence signed with a leaked key (M3).
    require!(
        ctx.accounts.station.status == STATION_STATUS_ACTIVE,
        LastroV2Error::InvalidStationProof
    );
    let session = &mut ctx.accounts.session;
    let relayer = ctx.accounts.relayer.key();
    require!(
        relayer == session.custodian || relayer == ctx.accounts.config.authority,
        LastroV2Error::UnauthorizedActor
    );
    require!(
        session.allowed_actions & SESSION_ACTION_OBSERVE != 0,
        LastroV2Error::InvalidSession
    );
    let events_used = session
        .events_used
        .checked_add(leaf_count)
        .ok_or_else(|| error!(LastroV2Error::InvalidBatch))?;
    require!(
        merkle_root.iter().any(|byte| *byte != 0)
            && (1..=MAX_BATCH_LEAVES).contains(&leaf_count)
            && events_used <= session.max_events
            && session.valid_from <= first_observed_at
            && first_observed_at <= last_observed_at
            && last_observed_at <= session.observation_deadline()
            && last_observed_at <= now
            && now <= last_observed_at + MAX_BATCH_EVENT_AGE_SECONDS,
        LastroV2Error::InvalidBatch
    );

    let batch = &mut ctx.accounts.batch_anchor;
    batch.session_id = session.session_id;
    batch.batch_index = session.batch_count;
    batch.custodian = session.custodian;
    batch.station_id = session.station_id;
    batch.merkle_root = merkle_root;
    batch.leaf_count = leaf_count;
    batch.first_observed_at = first_observed_at;
    batch.last_observed_at = last_observed_at;
    batch.anchored_at = now;
    batch.facility_id = session.facility_id;
    batch.bump = ctx.bumps.batch_anchor;

    session.events_used = events_used;
    session.batch_count = session
        .batch_count
        .checked_add(1)
        .ok_or_else(|| error!(LastroV2Error::InvalidBatch))?;
    Ok(())
}

#[derive(Accounts)]
#[instruction(session_id: [u8; 32], station_id: [u8; 32])]
pub struct OpenSession<'info> {
    pub custodian: Signer<'info>,
    /// Funds the grant's rent and receives it back when the grant closes; never an authority.
    #[account(mut)]
    pub payer: Signer<'info>,
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
        seeds = [FACILITY_SEED, &config.deployment_id, &station.facility_id],
        bump = facility.bump,
    )]
    pub facility: Account<'info, FacilityRecord>,
    #[account(
        init,
        payer = payer,
        space = SessionGrant::SPACE,
        seeds = [SESSION_SEED, &config.deployment_id, &session_id],
        bump,
    )]
    pub session: Account<'info, SessionGrant>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RevokeSession<'info> {
    pub custodian: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        mut,
        has_one = custodian @ LastroV2Error::UnauthorizedActor,
        seeds = [SESSION_SEED, &config.deployment_id, &session.session_id],
        bump = session.bump,
    )]
    pub session: Account<'info, SessionGrant>,
}

#[derive(Accounts)]
pub struct CloseSession<'info> {
    pub caller: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        mut,
        close = rent_payer,
        seeds = [SESSION_SEED, &config.deployment_id, &session.session_id],
        bump = session.bump,
    )]
    pub session: Account<'info, SessionGrant>,
    /// CHECK: only receives the closed grant's rent; pinned to the payer recorded at opening.
    #[account(mut, address = session.rent_payer @ LastroV2Error::InvalidRentPayer)]
    pub rent_payer: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct AnchorObservationBatch<'info> {
    /// The session's custodian, or the deployment authority relaying on its behalf.
    pub relayer: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        mut,
        seeds = [SESSION_SEED, &config.deployment_id, &session.session_id],
        bump = session.bump,
    )]
    pub session: Account<'info, SessionGrant>,
    /// The session's Station; it must still be ACTIVE when its proofs are anchored.
    #[account(
        seeds = [STATION_V2_SEED, &config.deployment_id, &session.station_id],
        bump = station.bump,
    )]
    pub station: Account<'info, StationRecord>,
    #[account(
        init,
        payer = payer,
        space = BatchAnchor::SPACE,
        seeds = [
            BATCH_SEED,
            &config.deployment_id,
            &session.session_id,
            &session.batch_count.to_le_bytes(),
        ],
        bump,
    )]
    pub batch_anchor: Account<'info, BatchAnchor>,
    pub system_program: Program<'info, System>,
}
