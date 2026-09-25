//! Immutable event commitments anchored by the v2 program.

use anchor_lang::prelude::*;

#[account]
pub struct EventAnchor {
    pub event_id: [u8; 32],
    pub deployment_id: [u8; 32],
    pub subject_id: [u8; 32],
    pub source_id: [u8; 32],
    pub event_type: u16,
    pub state_version: u64,
    pub observed_at: i64,
    pub expires_at: i64,
    pub expected_previous_hash: [u8; 32],
    pub payload_hash: [u8; 32],
    pub event_hash: [u8; 32],
    pub bump: u8,
}

impl EventAnchor {
    pub const SPACE: usize = 8 + (32 * 7) + 2 + 8 + 8 + 8 + 1;
}
