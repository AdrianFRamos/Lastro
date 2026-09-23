//! Station identity and signature verification.
//! Message = raw StationEvent[276].
//! Algorithm = ECDSA P-256/SHA-256.
//! Public key = SEC1 compressed 33 bytes.
//! Signature = compact r||s 64 bytes with low-S canonicalization.

use p256::ecdsa::{Signature, VerifyingKey, signature::Verifier};
use sha2::{Digest, Sha256};

use crate::{
    constants::{STATION_DOMAIN, STATION_EVENT_LEN},
    error::ProtocolError,
    ids::StationId,
};

pub fn derive_station_id(compressed_pubkey: &[u8]) -> Result<StationId, ProtocolError> {
    if compressed_pubkey.len() != 33 || !matches!(compressed_pubkey[0], 0x02 | 0x03) {
        return Err(ProtocolError::InvalidStationKey);
    }
    VerifyingKey::from_sec1_bytes(compressed_pubkey)
        .map_err(|_| ProtocolError::InvalidStationKey)?;

    let mut hasher = Sha256::new();
    hasher.update(STATION_DOMAIN);
    hasher.update(compressed_pubkey);
    Ok(hasher.finalize().into())
}

pub fn verify_station_signature(
    event_bytes: &[u8; STATION_EVENT_LEN],
    compressed_pubkey: &[u8; 33],
    signature_rs: &[u8; 64],
) -> Result<(), ProtocolError> {
    let verifying_key = VerifyingKey::from_sec1_bytes(compressed_pubkey)
        .map_err(|_| ProtocolError::InvalidStationKey)?;
    let signature =
        Signature::from_slice(signature_rs).map_err(|_| ProtocolError::InvalidStationSignature)?;

    if signature.normalize_s().is_some() {
        return Err(ProtocolError::HighSSignature);
    }

    verifying_key
        .verify(event_bytes, &signature)
        .map_err(|_| ProtocolError::InvalidStationSignature)
}
