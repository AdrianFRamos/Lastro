//! Protocol constants duplicated here only where on-chain account validation needs them.
//! Cross-layer repository tests must fail if these drift from `lastro-protocol`.

pub const STATION_EVENT_LEN: usize = 276;
pub const PROTOCOL_CONFIG_SEED: &[u8] = b"config";
pub const ANIMAL_STATE_SEED: &[u8] = b"animal";
pub const RFID_BINDING_SEED: &[u8] = b"rfid";
pub const RFID_STATUS_ACTIVE: u8 = 1;
pub const RFID_STATUS_RETIRED: u8 = 2;
