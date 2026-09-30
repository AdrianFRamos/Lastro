//! On-chain lineage commitment for animals, lots, carcasses, and products.

use anchor_lang::prelude::*;

#[account]
pub struct LineageAnchor {
    pub asset_id: [u8; 32],
    pub lineage_root: [u8; 32],
    pub parent_root: [u8; 32],
    pub edge_count: u64,
    pub sequence: u64,
    pub last_transformation: [u8; 32],
    pub bump: u8,
}

impl LineageAnchor {
    pub const SPACE: usize = 8 + (32 * 4) + (8 * 2) + 1;
}
