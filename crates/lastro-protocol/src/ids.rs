//! Fixed-size protocol identifiers.
//! AssetIDs are random and never derived from RFID.

pub type DeploymentId = [u8; 32];
pub type StationId = [u8; 32];
pub type RfidHash = [u8; 32];
