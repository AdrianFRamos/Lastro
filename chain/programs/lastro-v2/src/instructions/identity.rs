//! Physical identifier lifecycle: the Station proves which RFID it read; the current
//! custodian authorizes binding it to the persistent AssetID.
//!
//! IDENTIFIER_BOUND binds the first RFID of an asset. IDENTIFIER_REPLACED retires the
//! active RFID and binds a new one to the same AssetID ("the tag changed, the animal did
//! not"). RfidBinding accounts are never closed, so an RFID is never reused.

use anchor_lang::prelude::*;
use lastro_protocol::v2::{EventType, V2_ENVELOPE_LEN, identifier_payload_hash};

use crate::{
    constants::{
        ASSET_SEED, CONFIG_V2_SEED, EVENT_SEED, RFID_BINDING_SEED, STATION_REGISTRY_SEED,
        STATION_V2_SEED,
    },
    error::LastroV2Error,
    instructions::events::{StationEventContext, admit_station_event, apply_station_event},
    state::{
        AssetState, EventAnchor, ProtocolConfigV2, RegistryRoot, RfidBinding, StationRecord,
        rfid_binding::{RFID_STATUS_ACTIVE, RFID_STATUS_RETIRED},
    },
};

const ZERO32: [u8; 32] = [0; 32];

pub fn bind_handler(
    ctx: Context<BindIdentifier>,
    subject_id: [u8; 32],
    event_id: [u8; 32],
    station_id: [u8; 32],
    event: [u8; V2_ENVELOPE_LEN],
    new_rfid_hash: [u8; 32],
) -> Result<()> {
    let (envelope, event_hash) = admit_station_event(
        StationEventContext {
            config: &ctx.accounts.config,
            station: &ctx.accounts.station,
            asset: &ctx.accounts.asset,
            instructions_sysvar: &ctx.accounts.instructions_sysvar,
        },
        EventType::IdentifierBound,
        subject_id,
        event_id,
        station_id,
        &event,
        32,
    )?;
    let asset = &mut ctx.accounts.asset;
    require!(
        asset.custodian == ctx.accounts.custodian.key(),
        LastroV2Error::UnauthorizedActor
    );
    require!(
        asset.current_rfid_hash == ZERO32 && new_rfid_hash != ZERO32,
        LastroV2Error::InvalidIdentifierTransition
    );
    require!(
        envelope.payload_hash == identifier_payload_hash(ZERO32, new_rfid_hash),
        LastroV2Error::IntentPayloadMismatch
    );

    let binding = &mut ctx.accounts.rfid_binding;
    binding.asset_id = asset.asset_id;
    binding.rfid_hash = new_rfid_hash;
    binding.status = RFID_STATUS_ACTIVE;
    binding.bump = ctx.bumps.rfid_binding;

    asset.current_rfid_hash = new_rfid_hash;
    apply_station_event(
        asset,
        &mut ctx.accounts.event_anchor,
        &envelope,
        event_hash,
        ctx.bumps.event_anchor,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn replace_handler(
    ctx: Context<ReplaceIdentifier>,
    subject_id: [u8; 32],
    event_id: [u8; 32],
    station_id: [u8; 32],
    event: [u8; V2_ENVELOPE_LEN],
    old_rfid_hash: [u8; 32],
    new_rfid_hash: [u8; 32],
) -> Result<()> {
    let (envelope, event_hash) = admit_station_event(
        StationEventContext {
            config: &ctx.accounts.config,
            station: &ctx.accounts.station,
            asset: &ctx.accounts.asset,
            instructions_sysvar: &ctx.accounts.instructions_sysvar,
        },
        EventType::IdentifierReplaced,
        subject_id,
        event_id,
        station_id,
        &event,
        64,
    )?;
    let asset = &mut ctx.accounts.asset;
    require!(
        asset.custodian == ctx.accounts.custodian.key(),
        LastroV2Error::UnauthorizedActor
    );
    require!(
        old_rfid_hash != ZERO32
            && new_rfid_hash != ZERO32
            && old_rfid_hash != new_rfid_hash
            && asset.current_rfid_hash == old_rfid_hash,
        LastroV2Error::InvalidIdentifierTransition
    );
    let old_binding = &mut ctx.accounts.old_rfid_binding;
    require!(
        old_binding.status == RFID_STATUS_ACTIVE
            && old_binding.asset_id == asset.asset_id
            && old_binding.rfid_hash == old_rfid_hash,
        LastroV2Error::InvalidIdentifierTransition
    );
    require!(
        envelope.payload_hash == identifier_payload_hash(old_rfid_hash, new_rfid_hash),
        LastroV2Error::IntentPayloadMismatch
    );

    old_binding.status = RFID_STATUS_RETIRED;
    let new_binding = &mut ctx.accounts.new_rfid_binding;
    new_binding.asset_id = asset.asset_id;
    new_binding.rfid_hash = new_rfid_hash;
    new_binding.status = RFID_STATUS_ACTIVE;
    new_binding.bump = ctx.bumps.new_rfid_binding;

    asset.current_rfid_hash = new_rfid_hash;
    apply_station_event(
        asset,
        &mut ctx.accounts.event_anchor,
        &envelope,
        event_hash,
        ctx.bumps.event_anchor,
    );
    Ok(())
}

#[derive(Accounts)]
#[instruction(subject_id: [u8; 32], event_id: [u8; 32], station_id: [u8; 32], event: [u8; V2_ENVELOPE_LEN], new_rfid_hash: [u8; 32])]
pub struct BindIdentifier<'info> {
    #[account(mut)]
    pub custodian: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, ProtocolConfigV2>>,
    #[account(
        address = config.station_registry,
        seeds = [STATION_REGISTRY_SEED, &config.deployment_id],
        bump = station_registry.bump,
    )]
    pub station_registry: Box<Account<'info, RegistryRoot>>,
    #[account(
        seeds = [STATION_V2_SEED, &config.deployment_id, &station_id],
        bump = station.bump,
    )]
    pub station: Box<Account<'info, StationRecord>>,
    #[account(
        mut,
        seeds = [ASSET_SEED, &config.deployment_id, &subject_id],
        bump = asset.bump,
    )]
    pub asset: Box<Account<'info, AssetState>>,
    #[account(
        init,
        payer = custodian,
        space = EventAnchor::SPACE,
        seeds = [EVENT_SEED, &config.deployment_id, &event_id],
        bump,
    )]
    pub event_anchor: Box<Account<'info, EventAnchor>>,
    #[account(
        init,
        payer = custodian,
        space = RfidBinding::SPACE,
        seeds = [RFID_BINDING_SEED, &config.deployment_id, &new_rfid_hash],
        bump,
    )]
    pub rfid_binding: Box<Account<'info, RfidBinding>>,
    /// CHECK: Anchor validates this address and the verifier reads its instruction list.
    #[account(address = solana_sdk_ids::sysvar::instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(subject_id: [u8; 32], event_id: [u8; 32], station_id: [u8; 32], event: [u8; V2_ENVELOPE_LEN], old_rfid_hash: [u8; 32], new_rfid_hash: [u8; 32])]
pub struct ReplaceIdentifier<'info> {
    #[account(mut)]
    pub custodian: Signer<'info>,
    #[account(
        seeds = [CONFIG_V2_SEED, &config.deployment_id],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, ProtocolConfigV2>>,
    #[account(
        address = config.station_registry,
        seeds = [STATION_REGISTRY_SEED, &config.deployment_id],
        bump = station_registry.bump,
    )]
    pub station_registry: Box<Account<'info, RegistryRoot>>,
    #[account(
        seeds = [STATION_V2_SEED, &config.deployment_id, &station_id],
        bump = station.bump,
    )]
    pub station: Box<Account<'info, StationRecord>>,
    #[account(
        mut,
        seeds = [ASSET_SEED, &config.deployment_id, &subject_id],
        bump = asset.bump,
    )]
    pub asset: Box<Account<'info, AssetState>>,
    #[account(
        init,
        payer = custodian,
        space = EventAnchor::SPACE,
        seeds = [EVENT_SEED, &config.deployment_id, &event_id],
        bump,
    )]
    pub event_anchor: Box<Account<'info, EventAnchor>>,
    #[account(
        mut,
        seeds = [RFID_BINDING_SEED, &config.deployment_id, &old_rfid_hash],
        bump = old_rfid_binding.bump,
    )]
    pub old_rfid_binding: Box<Account<'info, RfidBinding>>,
    #[account(
        init,
        payer = custodian,
        space = RfidBinding::SPACE,
        seeds = [RFID_BINDING_SEED, &config.deployment_id, &new_rfid_hash],
        bump,
    )]
    pub new_rfid_binding: Box<Account<'info, RfidBinding>>,
    /// CHECK: Anchor validates this address and the verifier reads its instruction list.
    #[account(address = solana_sdk_ids::sysvar::instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
