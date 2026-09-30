//! Reservation of an asset balance by one open transformation.
//!
//! A consumed reservation is kept (not closed) with `consumed_at != 0`. Its PDA existing
//! is what stops the same input leaf from being reserved and consumed a second time.

use anchor_lang::prelude::*;

#[account]
pub struct TransformationReservation {
    pub transformation_id: [u8; 32],
    pub asset_id: [u8; 32],
    pub weight_grams: u64,
    pub expected_state_version: u64,
    pub reserved_until: i64,
    pub consumed_at: i64,
    pub bump: u8,
}

impl TransformationReservation {
    pub const SPACE: usize = 8 + (32 * 2) + (8 * 4) + 1;
}
