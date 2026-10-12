//! Immutable event commitments anchored by the v2 program.

use anchor_lang::prelude::*;

/// Minimal on-chain commitment to one Station event. The PDA seeds bind the deployment and
/// event id, and `event_hash` commits to every other envelope field, so a verifier holding the
/// 220 envelope bytes recomputes the hash and compares it here instead of reading copies of
/// each field.
#[account]
pub struct EventAnchor {
    pub event_hash: [u8; 32],
    pub subject_id: [u8; 32],
    pub state_version: u64,
    pub event_type: u16,
    pub bump: u8,
}

impl EventAnchor {
    pub const SPACE: usize = 8 + 32 + 32 + 8 + 2 + 1;
}
