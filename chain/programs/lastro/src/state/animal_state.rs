//! Canonical state for one persistent AnimalID.

use anchor_lang::prelude::*;

#[account]
pub struct AnimalState {
    pub animal_id: [u8; 32],
    pub current_rfid_hash: [u8; 32],
    pub current_custodian: Pubkey,
    pub identity_revision: u32,
    pub event_sequence: u64,
    pub last_event_hash: [u8; 32],
    pub bump: u8,
}

impl AnimalState {
    pub const SPACE: usize = 8 + 32 + 32 + 32 + 4 + 8 + 32 + 1;
}
