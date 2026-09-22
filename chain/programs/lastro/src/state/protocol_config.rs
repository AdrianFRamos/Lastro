//! One immutable configuration account per hackathon deployment.

use anchor_lang::prelude::*;

#[account]
pub struct ProtocolConfig {
    pub authority: Pubkey,
    pub deployment_id: [u8; 32],
    pub station_pubkey33: [u8; 33],
    pub bump: u8,
}

impl ProtocolConfig {
    /// 8-byte Anchor discriminator + exact serialized fields.
    pub const SPACE: usize = 8 + 32 + 32 + 33 + 1;
}
