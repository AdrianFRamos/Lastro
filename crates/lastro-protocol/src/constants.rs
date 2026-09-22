//! Frozen wire-protocol constants.

pub const STATION_EVENT_LEN: usize = 276;
pub const CANONICAL_RFID_LEN: usize = 8;
pub const MAGIC: [u8; 4] = *b"LSTR";
pub const VERSION: u8 = 1;
pub const RESERVED: [u8; 2] = [0, 0];

pub const RFID_DOMAIN: &[u8] = b"LASTRO_RFID\0";
pub const STATION_DOMAIN: &[u8] = b"LASTRO_STATION\0";

pub mod offset {
    pub const MAGIC: usize = 0;
    pub const VERSION: usize = 4;
    pub const ACTION: usize = 5;
    pub const RESERVED: usize = 6;
    pub const DEPLOYMENT_ID: usize = 8;
    pub const ANIMAL_ID: usize = 40;
    pub const STATION_ID: usize = 72;
    pub const EVENT_SEQUENCE: usize = 104;
    pub const IDENTITY_REVISION: usize = 112;
    pub const PREVIOUS_EVENT_HASH: usize = 116;
    pub const OLD_RFID_HASH: usize = 148;
    pub const NEW_RFID_HASH: usize = 180;
    pub const FROM_CUSTODIAN: usize = 212;
    pub const TO_CUSTODIAN: usize = 244;
    pub const END: usize = 276;
}
