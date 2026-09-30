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
    /// Hash of the ACTIVE physical identifier (RFID); zero until IDENTIFIER_BOUND.
    pub current_rfid_hash: [u8; 32],
    pub bump: u8,
}

impl AssetState {
    /// 8 discriminator + 9 x 32-byte fields (asset_id, deployment_id, custodian,
    /// parent_root, lineage_root, current_lot_id, last_event_hash, reserved_by,
    /// current_rfid_hash) + type/status + 5 x 8-byte integers + flags + bump = 341 bytes.
    pub const SPACE: usize = 8 + (32 * 9) + 1 + 1 + (8 * 5) + 2 + 1;

    pub fn is_closed(&self) -> bool {
        matches!(self.status, 4 | 7)
    }

    /// Consumed, closed and retired assets are terminal and accept no further transitions.
    pub fn is_terminal(&self) -> bool {
        matches!(self.status, 3 | 4 | 7)
    }
}
