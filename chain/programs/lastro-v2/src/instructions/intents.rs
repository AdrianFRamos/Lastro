//! On-chain intent lifecycle for replay and stale-state protection.

use anchor_lang::prelude::*;

use crate::{
    constants::{
        CONFIG_V2_SEED, INTENT_SEED, INTENT_STATUS_CANCELLED, INTENT_STATUS_CONSUMED,
        INTENT_STATUS_EXPIRED, INTENT_STATUS_OPEN,
    },
    error::LastroV2Error,
    state::{IntentState, ProtocolConfigV2},
};

pub fn handler(
    ctx: Context<CreateIntent>,
    intent_id: [u8; 32],
    subject_id: [u8; 32],
    intent_type: u16,
    expected_state_version: u64,
    nonce: u64,
    expires_at: i64,
    payload_hash: [u8; 32],
) -> Result<()> {
    require_nonzero(&intent_id)?;
    require_nonzero(&subject_id)?;
    require_nonzero(&payload_hash)?;
    require!(matches!(intent_type, 1..=5), LastroV2Error::InvalidIntentType);

    let now = Clock::get()?.unix_timestamp;
    validate_expiry(&ctx.accounts.config, now, expires_at)?;

    let intent = &mut ctx.accounts.intent;
    intent.intent_id = intent_id;
    intent.subject_id = subject_id;
    intent.intent_type = intent_type;
    intent.expected_state_version = expected_state_version;
    intent.nonce = nonce;
    intent.actor = ctx.accounts.actor.key();
    intent.status = INTENT_STATUS_OPEN;
    intent.expires_at = expires_at;
    intent.consumed_at = 0;
    intent.payload_hash = payload_hash;
    intent.bump = ctx.bumps.intent;
    Ok(())
}

pub fn cancel_handler(ctx: Context<CancelIntent>) -> Result<()> {
    require!(
        ctx.accounts.intent.actor == ctx.accounts.actor.key(),
        LastroV2Error::UnauthorizedActor
    );
    require!(
        ctx.accounts.intent.status == INTENT_STATUS_OPEN,
        LastroV2Error::IntentNotOpen
    );
    ctx.accounts.intent.status = INTENT_STATUS_CANCELLED;
    ctx.accounts.intent.consumed_at = Clock::get()?.unix_timestamp;
    Ok(())
}

pub fn expire_handler(ctx: Context<ExpireIntent>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.intent.status == INTENT_STATUS_OPEN,
        LastroV2Error::IntentNotOpen
    );
    require!(now >= ctx.accounts.intent.expires_at, LastroV2Error::IntentExpired);
    ctx.accounts.intent.status = INTENT_STATUS_EXPIRED;
    ctx.accounts.intent.consumed_at = now;
    Ok(())
}

pub fn consume_handler(
    ctx: Context<ConsumeIntent>,
    expected_state_version: u64,
    payload_hash: [u8; 32],
) -> Result<()> {
    let intent = &mut ctx.accounts.intent;
    require!(intent.actor == ctx.accounts.actor.key(), LastroV2Error::UnauthorizedActor);
    require!(intent.status == INTENT_STATUS_OPEN, LastroV2Error::IntentNotOpen);
    require!(Clock::get()?.unix_timestamp < intent.expires_at, LastroV2Error::IntentExpired);
    require!(intent.expected_state_version == expected_state_version, LastroV2Error::InvalidStateVersion);
    require!(intent.payload_hash == payload_hash, LastroV2Error::IntentPayloadMismatch);
    intent.status = INTENT_STATUS_CONSUMED;
    intent.consumed_at = Clock::get()?.unix_timestamp;
    Ok(())
}

fn validate_expiry(config: &ProtocolConfigV2, now: i64, expires_at: i64) -> Result<()> {
    require!(expires_at > now, LastroV2Error::InvalidIntentExpiration);
    let max_expiry = now
        .checked_add(config.max_event_age_seconds as i64)
        .ok_or(error!(LastroV2Error::InvalidIntentExpiration))?;
    require!(expires_at <= max_expiry, LastroV2Error::InvalidIntentExpiration);
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
#[instruction(intent_id: [u8; 32], subject_id: [u8; 32])]
pub struct CreateIntent<'info> {
    #[account(mut)]
    pub actor: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        init,
        payer = actor,
        space = IntentState::SPACE,
        seeds = [INTENT_SEED, &config.deployment_id, &subject_id, &intent_id],
        bump
    )]
    pub intent: Account<'info, IntentState>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelIntent<'info> {
    pub actor: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        mut,
        seeds = [INTENT_SEED, &config.deployment_id, &intent.subject_id, &intent.intent_id],
        bump = intent.bump,
    )]
    pub intent: Account<'info, IntentState>,
}

#[derive(Accounts)]
pub struct ExpireIntent<'info> {
    pub caller: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        mut,
        seeds = [INTENT_SEED, &config.deployment_id, &intent.subject_id, &intent.intent_id],
        bump = intent.bump,
    )]
    pub intent: Account<'info, IntentState>,
}

#[derive(Accounts)]
pub struct ConsumeIntent<'info> {
    pub actor: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
    )]
    pub config: Account<'info, ProtocolConfigV2>,
    #[account(
        mut,
        seeds = [INTENT_SEED, &config.deployment_id, &intent.subject_id, &intent.intent_id],
        bump = intent.bump,
    )]
    pub intent: Account<'info, IntentState>,
}

#[allow(dead_code)]
const _: u8 = INTENT_STATUS_CANCELLED;
