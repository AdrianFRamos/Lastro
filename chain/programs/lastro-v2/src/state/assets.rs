//! Canonical state shared by animals, lots, carcasses and products.

use anchor_lang::prelude::*;

#[account]
pub struct AssetState {
    pub asset_id: [u8; 32],
    pub asset_type: u8,
    pub status: u8,
    pub deployment_id: [u8; 32],
    pub custodian: Pubkey,
    pub parent_root: [u8; 32],
    pub lineage_root: [u8; 32],
    pub current_lot_id: [u8; 32],
    pub available_weight_grams: u64,
    pub reserved_weight_grams: u64,
    pub event_sequence: u64,
    pub state_version: u64,
    pub last_event_hash: [u8; 32],
    pub reserved_by: [u8; 32],
    pub reserved_until: i64,
    pub flags: u16,
    pub bump: u8,
}

impl AssetState {
    pub const SPACE: usize = 8 + (32 * 7) + 1 + 1 + (8 * 5) + 2 + 1;

    pub fn is_closed(&self) -> bool {
        matches!(self.status, 4 | 7)
    }
}
