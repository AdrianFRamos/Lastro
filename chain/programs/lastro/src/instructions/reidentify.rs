//! REIDENTIFY preserves AnimalID/custodian while replacing the physical RFID binding.

use anchor_lang::{prelude::*, solana_program::sysvar};

use crate::{
    constants::{ANIMAL_STATE_SEED, PROTOCOL_CONFIG_SEED, RFID_BINDING_SEED, RFID_STATUS_ACTIVE},
    error::LastroError,
    state::{AnimalState, ProtocolConfig, RfidBinding},
    verify::{event::{parse_event, station_id_from_pubkey, ACTION_REIDENTIFY}, secp256r1::verify_station_precompile_binding},
};

pub fn handler(ctx: Context<'_, '_, '_, '_, Reidentify<'_>>, event: [u8; 276]) -> Result<()> {
    let parsed = parse_event(&event)?;
    require!(parsed.action() == ACTION_REIDENTIFY, LastroError::InvalidEvent);
    require!(parsed.deployment_id() == ctx.accounts.protocol_config.deployment_id, LastroError::InvalidEvent);
    require!(parsed.station_id() == station_id_from_pubkey(&ctx.accounts.protocol_config.station_pubkey33), LastroError::InvalidStationProof);
    verify_station_precompile_binding(
        &ctx.accounts.instructions.to_account_info(),
        &ctx.accounts.protocol_config.station_pubkey33,
        parsed.raw,
    )?;

    let animal = &mut ctx.accounts.animal_state;
    require!(animal.animal_id == parsed.animal_id(), LastroError::InvalidEvent);
    require!(parsed.event_sequence() == animal.event_sequence.checked_add(1).ok_or_else(|| error!(LastroError::InvalidSequence))?, LastroError::InvalidSequence);
    require!(parsed.identity_revision() == animal.identity_revision.checked_add(1).ok_or_else(|| error!(LastroError::InvalidRevision))?, LastroError::InvalidRevision);
    require!(parsed.previous_event_hash() == animal.last_event_hash, LastroError::InvalidPredecessor);
    require!(parsed.old_rfid_hash() == animal.current_rfid_hash, LastroError::InvalidRfidTransition);

    let current_custodian = animal.current_custodian;
    require!(ctx.accounts.current_custodian.key() == current_custodian, LastroError::InvalidCustodian);
    require!(Pubkey::new_from_array(parsed.from_custodian()) == current_custodian, LastroError::InvalidCustodian);
    require!(Pubkey::new_from_array(parsed.to_custodian()) == current_custodian, LastroError::InvalidCustodian);

    let old_binding = &mut ctx.accounts.old_rfid_binding;
    require!(old_binding.is_active(), LastroError::RfidBindingMismatch);
    require!(old_binding.animal_id == animal.animal_id && old_binding.rfid_hash == animal.current_rfid_hash, LastroError::RfidBindingMismatch);
    old_binding.retire();

    let new_binding = &mut ctx.accounts.new_rfid_binding;
    new_binding.animal_id = animal.animal_id;
    new_binding.rfid_hash = parsed.new_rfid_hash();
    new_binding.status = RFID_STATUS_ACTIVE;
    new_binding.bump = ctx.bumps.new_rfid_binding;

    animal.current_rfid_hash = parsed.new_rfid_hash();
    animal.identity_revision = parsed.identity_revision();
    animal.event_sequence = parsed.event_sequence();
    animal.last_event_hash = parsed.event_hash();
    Ok(())
}

#[derive(Accounts)]
#[instruction(event: [u8; 276])]
pub struct Reidentify<'info> {
    #[account(mut)]
    pub current_custodian: Signer<'info>,
    #[account(
        seeds = [PROTOCOL_CONFIG_SEED, &event[8..40]],
        bump = protocol_config.bump
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,
    #[account(
        mut,
        seeds = [ANIMAL_STATE_SEED, &event[8..40], &event[40..72]],
        bump = animal_state.bump
    )]
    pub animal_state: Account<'info, AnimalState>,
    #[account(
        mut,
        seeds = [RFID_BINDING_SEED, &event[8..40], &event[148..180]],
        bump = old_rfid_binding.bump
    )]
    pub old_rfid_binding: Account<'info, RfidBinding>,
    #[account(
        init,
        payer = current_custodian,
        space = RfidBinding::SPACE,
        seeds = [RFID_BINDING_SEED, &event[8..40], &event[180..212]],
        bump
    )]
    pub new_rfid_binding: Account<'info, RfidBinding>,
    /// CHECK: address constraint fixes this account to the Instructions sysvar.
    #[account(address = sysvar::instructions::ID)]
    pub instructions: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
