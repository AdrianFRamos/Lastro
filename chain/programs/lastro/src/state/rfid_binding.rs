//! Global-per-deployment RFID history index.
//!
//! A binding is never closed during the hackathon. REIDENTIFY marks the old account
//! RETIRED and creates a new ACTIVE account. Therefore an RFID that ever participated
//! in canonical history cannot be silently reused for another AnimalID.

use anchor_lang::prelude::*;
use crate::constants::{RFID_STATUS_ACTIVE, RFID_STATUS_RETIRED};

#[account]
pub struct RfidBinding {
    pub animal_id: [u8; 32],
    pub rfid_hash: [u8; 32],
    pub status: u8,
    pub bump: u8,
}

impl RfidBinding {
    pub const SPACE: usize = 8 + 32 + 32 + 1 + 1;
    pub fn is_active(&self) -> bool { self.status == RFID_STATUS_ACTIVE }
    pub fn retire(&mut self) { self.status = RFID_STATUS_RETIRED; }
}
