//! Deployment-wide RFID history index.
//!
//! A binding is never closed. IDENTIFIER_REPLACED marks the old account RETIRED and creates a
//! new ACTIVE one, so an RFID that ever identified an asset can never identify another.

use anchor_lang::prelude::*;

pub const RFID_STATUS_ACTIVE: u8 = 1;
pub const RFID_STATUS_RETIRED: u8 = 2;

#[account]
pub struct RfidBinding {
    pub asset_id: [u8; 32],
    pub rfid_hash: [u8; 32],
    pub status: u8,
    pub bump: u8,
}

impl RfidBinding {
    pub const SPACE: usize = 8 + 32 + 32 + 1 + 1;
}
