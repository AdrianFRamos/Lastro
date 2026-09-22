//! ORIGIN creates the canonical AnimalState and first ACTIVE RfidBinding atomically.

use anchor_lang::{prelude::*, solana_program::sysvar};

use crate::{
    constants::{ANIMAL_STATE_SEED, PROTOCOL_CONFIG_SEED, RFID_BINDING_SEED, RFID_STATUS_ACTIVE},
    error::LastroError,
    state::{AnimalState, ProtocolConfig, RfidBinding},
    verify::{event::{parse_event, station_id_from_pubkey, ACTION_ORIGIN}, secp256r1::verify_station_precompile_binding},
};

pub fn handler(ctx: Context<'_, '_, '_, '_, Origin<'_>>, event: [u8; 276]) -> Result<()> {
    let parsed = parse_event(&event)?;
    require!(parsed.action() == ACTION_ORIGIN, LastroError::InvalidEvent);
    require!(parsed.deployment_id() == ctx.accounts.protocol_config.deployment_id, LastroError::InvalidEvent);
    require!(parsed.station_id() == station_id_from_pubkey(&ctx.accounts.protocol_config.station_pubkey33), LastroError::InvalidStationProof);
    verify_station_precompile_binding(
        &ctx.accounts.instructions.to_account_info(),
        &ctx.accounts.protocol_config.station_pubkey33,
        parsed.raw,
    )?;

    let custodian = Pubkey::new_from_array(parsed.to_custodian());
    require!(ctx.accounts.custodian.key() == custodian, LastroError::InvalidCustodian);

    let animal = &mut ctx.accounts.animal_state;
    animal.animal_id = parsed.animal_id();
    animal.current_rfid_hash = parsed.new_rfid_hash();
    animal.current_custodian = custodian;
    animal.identity_revision = parsed.identity_revision();
    animal.event_sequence = parsed.event_sequence();
    animal.last_event_hash = parsed.event_hash();
    animal.bump = ctx.bumps.animal_state;

    let binding = &mut ctx.accounts.rfid_binding;
    binding.animal_id = parsed.animal_id();
    binding.rfid_hash = parsed.new_rfid_hash();
    binding.status = RFID_STATUS_ACTIVE;
    binding.bump = ctx.bumps.rfid_binding;
    Ok(())
}

#[derive(Accounts)]
#[instruction(event: [u8; 276])]
pub struct Origin<'info> {
    #[account(mut)]
    pub custodian: Signer<'info>,
    #[account(
        seeds = [PROTOCOL_CONFIG_SEED, &event[8..40]],
        bump = protocol_config.bump
    )]
    pub protocol_config: Account<'info, ProtocolConfig>,
    #[account(
        init,
        payer = custodian,
        space = AnimalState::SPACE,
        seeds = [ANIMAL_STATE_SEED, &event[8..40], &event[40..72]],
        bump
    )]
    pub animal_state: Account<'info, AnimalState>,
    #[account(
        init,
        payer = custodian,
        space = RfidBinding::SPACE,
        seeds = [RFID_BINDING_SEED, &event[8..40], &event[180..212]],
        bump
    )]
    pub rfid_binding: Account<'info, RfidBinding>,
    /// CHECK: address constraint fixes this account to the Instructions sysvar.
    #[account(address = sysvar::instructions::ID)]
    pub instructions: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}
