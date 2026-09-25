//! On-chain commitment for a facility transformation.

use anchor_lang::prelude::*;

#[account]
pub struct TransformationAnchor {
    pub transformation_id: [u8; 32],
    pub facility_id: [u8; 32],
    pub transformation_type: u16,
    pub input_root: [u8; 32],
    pub output_root: [u8; 32],
    pub input_count: u32,
    pub output_count: u32,
    pub reserved_input_count: u32,
    pub input_weight_grams: u64,
    pub reserved_input_weight_grams: u64,
    pub output_weight_grams: u64,
    pub byproduct_weight_grams: u64,
    pub loss_weight_grams: u64,
    pub tolerance_basis_points: u16,
    pub status: u8,
    pub manifest_hash: [u8; 32],
    pub sequence: u64,
    pub expires_at: i64,
    pub bump: u8,
}

impl TransformationAnchor {
    pub const SPACE: usize =
        8 + 32 + 32 + 2 + (32 * 2) + (4 * 3) + (8 * 5) + 2 + 1 + 32 + 8 + 8 + 1;
}
