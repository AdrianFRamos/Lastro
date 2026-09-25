//! On-chain constants for Lastro v2.

pub const CONFIG_V2_SEED: &[u8] = b"config-v2";
pub const STATION_V2_SEED: &[u8] = b"station-v2";
pub const FACILITY_SEED: &[u8] = b"facility";
pub const PARTY_SEED: &[u8] = b"party";
pub const ASSET_SEED: &[u8] = b"asset";
pub const INTENT_SEED: &[u8] = b"intent";
pub const EVENT_SEED: &[u8] = b"event";

pub const STATION_REGISTRY_SEED: &[u8] = b"station-registry";
pub const FACILITY_REGISTRY_SEED: &[u8] = b"facility-registry";
pub const PARTY_REGISTRY_SEED: &[u8] = b"party-registry";
pub const DOCUMENT_REGISTRY_SEED: &[u8] = b"document-registry";

pub const SCHEMA_VERSION: u16 = 1;

pub const STATION_STATUS_ACTIVE: u8 = 1;
pub const STATION_STATUS_SUSPENDED: u8 = 2;
pub const STATION_STATUS_REVOKED: u8 = 3;
pub const STATION_STATUS_EXPIRED: u8 = 4;

pub const FACILITY_STATUS_ACTIVE: u8 = 1;
pub const FACILITY_STATUS_SUSPENDED: u8 = 2;
pub const FACILITY_STATUS_REVOKED: u8 = 3;
pub const FACILITY_STATUS_EXPIRED: u8 = 4;

pub const ASSET_STATUS_ACTIVE: u8 = 1;
pub const ASSET_STATUS_IN_TRANSIT: u8 = 2;
pub const ASSET_STATUS_CONSUMED: u8 = 3;
pub const ASSET_STATUS_CLOSED: u8 = 4;
pub const ASSET_STATUS_QUALITY_HOLD: u8 = 5;
pub const ASSET_STATUS_RECALLED: u8 = 6;
pub const ASSET_STATUS_RETIRED: u8 = 7;

pub const INTENT_STATUS_OPEN: u8 = 1;
pub const INTENT_STATUS_CANCELLED: u8 = 2;
pub const INTENT_STATUS_CONSUMED: u8 = 3;
pub const INTENT_STATUS_EXPIRED: u8 = 4;

pub const FACILITY_TYPE_FARM: u8 = 1;
pub const FACILITY_TYPE_TRANSPORT_HUB: u8 = 2;
pub const FACILITY_TYPE_SLAUGHTERHOUSE: u8 = 3;
pub const FACILITY_TYPE_PROCESSING_FACILITY: u8 = 4;
pub const FACILITY_TYPE_DISTRIBUTION_CENTER: u8 = 5;
pub const FACILITY_TYPE_RETAIL: u8 = 6;
pub const FACILITY_TYPE_INSPECTION_SITE: u8 = 7;

pub const PARTY_ROLE_PRODUCER: u16 = 1;
pub const PARTY_ROLE_CUSTODIAN: u16 = 2;
pub const PARTY_ROLE_SELLER: u16 = 3;
pub const PARTY_ROLE_BUYER: u16 = 4;
pub const PARTY_ROLE_TRANSPORTER: u16 = 5;
pub const PARTY_ROLE_SLAUGHTERHOUSE: u16 = 6;
pub const PARTY_ROLE_PROCESSING_FACILITY: u16 = 7;
pub const PARTY_ROLE_DISTRIBUTOR: u16 = 8;
pub const PARTY_ROLE_RETAILER: u16 = 9;
pub const PARTY_ROLE_AUDITOR: u16 = 10;
pub const PARTY_ROLE_OFFICIAL_SOURCE: u16 = 11;

pub fn is_valid_asset_type(value: u8) -> bool {
    matches!(value, 1..=8)
}

pub fn is_valid_facility_type(value: u8) -> bool {
    matches!(value, 1..=7)
}

pub fn is_valid_party_role(value: u16) -> bool {
    matches!(value, 1..=11)
}

pub fn is_valid_station_status(value: u8) -> bool {
    matches!(value, STATION_STATUS_ACTIVE..=STATION_STATUS_EXPIRED)
}

pub fn is_valid_facility_status(value: u8) -> bool {
    matches!(value, FACILITY_STATUS_ACTIVE..=FACILITY_STATUS_EXPIRED)
}
