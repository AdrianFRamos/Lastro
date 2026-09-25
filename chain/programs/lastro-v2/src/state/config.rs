//! Deployment-scoped v2 configuration.

use anchor_lang::prelude::*;

#[account]
pub struct ProtocolConfigV2 {
    pub authority: Pubkey,
    pub deployment_id: [u8; 32],
    pub schema_version: u16,
    pub station_registry: Pubkey,
    pub facility_registry: Pubkey,
    pub party_registry: Pubkey,
    pub document_registry: Pubkey,
    pub max_asset_weight_grams: u64,
    pub mass_tolerance_basis_points: u16,
    pub max_event_age_seconds: u64,
    pub bump: u8,
}

impl ProtocolConfigV2 {
    pub const SPACE: usize = 8 + 32 + 32 + 2 + (32 * 4) + 8 + 2 + 8 + 1;
}
