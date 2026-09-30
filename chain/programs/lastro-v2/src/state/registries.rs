//! Versioned identity registries used by v2 authorization.

use anchor_lang::prelude::*;

#[account]
pub struct RegistryRoot {
    pub deployment_id: [u8; 32],
    pub registry_type: u8,
    pub bump: u8,
}

impl RegistryRoot {
    pub const SPACE: usize = 8 + 32 + 1 + 1;
}

#[account]
pub struct StationRecord {
    pub station_id: [u8; 32],
    pub key_id: [u8; 32],
    pub pubkey33: [u8; 33],
    pub status: u8,
    pub valid_from: i64,
    pub valid_until: i64,
    pub firmware_hash: [u8; 32],
    pub bump: u8,
}

impl StationRecord {
    pub const SPACE: usize = 8 + 32 + 32 + 33 + 1 + 8 + 8 + 32 + 1;
}

#[account]
pub struct FacilityRecord {
    pub facility_id: [u8; 32],
    pub owner: Pubkey,
    pub facility_type: u8,
    pub status: u8,
    pub credential_hash: [u8; 32],
    pub valid_from: i64,
    pub valid_until: i64,
    pub bump: u8,
}

impl FacilityRecord {
    pub const SPACE: usize = 8 + 32 + 32 + 1 + 1 + 32 + 8 + 8 + 1;
}

#[account]
pub struct PartyRecord {
    pub party_id: [u8; 32],
    pub wallet: Pubkey,
    pub role: u16,
    pub status: u8,
    pub bump: u8,
}

impl PartyRecord {
    pub const SPACE: usize = 8 + 32 + 32 + 2 + 1 + 1;
}
