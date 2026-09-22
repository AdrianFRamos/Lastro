//! Fixed-size protocol identifiers.
//! AnimalID is random and never derived from RFID.

pub type AnimalId = [u8; 32];
pub type DeploymentId = [u8; 32];
pub type StationId = [u8; 32];
pub type RfidHash = [u8; 32];
pub type EventHash = [u8; 32];
pub type Custodian = [u8; 32];
