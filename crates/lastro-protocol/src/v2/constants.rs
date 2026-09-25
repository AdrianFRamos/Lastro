//! Wire-level constants for Lastro domain protocol v2.

pub const SCHEMA_VERSION: u16 = 1;
pub const DOMAIN_EVENT_ENVELOPE_LEN: usize = 220;
pub const DOMAIN_ID_LEN: usize = 32;
pub const MAX_EVENT_AGE_SECONDS: u64 = 86_400;

pub const HASH_DOMAIN_EVENT: &[u8] = b"LASTRO_V2_EVENT\0";
pub const HASH_DOMAIN_PAYLOAD: &[u8] = b"LASTRO_V2_PAYLOAD\0";
pub const HASH_DOMAIN_ASSET: &[u8] = b"LASTRO_V2_ASSET\0";
pub const HASH_DOMAIN_LINEAGE: &[u8] = b"LASTRO_V2_LINEAGE\0";
pub const HASH_DOMAIN_TRANSFORMATION: &[u8] = b"LASTRO_V2_TRANSFORMATION\0";
pub const HASH_DOMAIN_INTENT: &[u8] = b"LASTRO_V2_INTENT\0";
pub const HASH_DOMAIN_MIGRATION: &[u8] = b"LASTRO_V2_MIGRATION\0";

pub const CONFIG_V2_SEED: &[u8] = b"config-v2";
pub const STATION_V2_SEED: &[u8] = b"station-v2";
pub const FACILITY_SEED: &[u8] = b"facility";
pub const PARTY_SEED: &[u8] = b"party";
pub const ASSET_SEED: &[u8] = b"asset";
pub const MIGRATION_SEED: &[u8] = b"migration";
pub const EVENT_SEED: &[u8] = b"event";
pub const INTENT_SEED: &[u8] = b"intent";
pub const LINEAGE_SEED: &[u8] = b"lineage";
pub const TRANSFORMATION_SEED: &[u8] = b"transformation";
pub const TRANSFORMATION_CHUNK_SEED: &[u8] = b"transformation-chunk";
pub const RECALL_SEED: &[u8] = b"recall";
