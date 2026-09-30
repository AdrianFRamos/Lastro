//! Canonical RFID representation and hashing.
//!
//! The reader adapter must first decode the logical unsigned 64-bit FDX-B identifier.
//! Lastro then serializes that value as exactly eight big-endian bytes. This is a Lastro
//! protocol convention, not a statement about RF transmission byte order.

use sha2::{Digest, Sha256};

use crate::{
    constants::{CANONICAL_RFID_LEN, RFID_DOMAIN},
    error::ProtocolError,
    ids::RfidHash,
};

pub type CanonicalRfid = [u8; CANONICAL_RFID_LEN];

pub fn canonical_rfid_from_slice(value: &[u8]) -> Result<CanonicalRfid, ProtocolError> {
    value
        .try_into()
        .map_err(|_| ProtocolError::InvalidRfidLength)
}

pub fn canonical_rfid_from_u64(value: u64) -> CanonicalRfid {
    value.to_be_bytes()
}

pub fn hash_canonical_rfid(canonical_rfid: &CanonicalRfid) -> RfidHash {
    let mut hasher = Sha256::new();
    hasher.update(RFID_DOMAIN);
    hasher.update(canonical_rfid);
    hasher.finalize().into()
}
