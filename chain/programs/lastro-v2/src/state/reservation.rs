//! Reservation of an asset balance by one open transformation.

use anchor_lang::prelude::*;

#[account]
pub struct TransformationReservation {
    pub transformation_id: [u8; 32],
    pub asset_id: [u8; 32],
    pub weight_grams: u64,
    pub expected_state_version: u64,
    pub reserved_until: i64,
    pub bump: u8,
}

impl TransformationReservation {
    pub const SPACE: usize = 8 + (32 * 2) + (8 * 2) + 8 + 1;
}
