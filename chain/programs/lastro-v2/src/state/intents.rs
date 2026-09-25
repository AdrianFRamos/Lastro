//! Durable on-chain intents prevent replay and stale supersession.

use anchor_lang::prelude::*;

#[account]
pub struct IntentState {
    pub intent_id: [u8; 32],
    pub subject_id: [u8; 32],
    pub intent_type: u16,
    pub expected_state_version: u64,
    pub nonce: u64,
    pub actor: Pubkey,
    pub status: u8,
    pub expires_at: i64,
    pub consumed_at: i64,
    pub payload_hash: [u8; 32],
    pub bump: u8,
}

impl IntentState {
    pub const SPACE: usize = 8 + 32 + 32 + 2 + 8 + 8 + 32 + 1 + 8 + 8 + 32 + 1;
}
