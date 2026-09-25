//! Domain-separated hashes for v2 commitments.

use sha2::{Digest, Sha256};

use super::constants::{HASH_DOMAIN_EVENT, HASH_DOMAIN_PAYLOAD};

pub fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    hasher.finalize().into()
}

pub fn event_hash(bytes: &[u8]) -> [u8; 32] {
    domain_hash(HASH_DOMAIN_EVENT, bytes)
}

pub fn payload_hash(canonical_payload: &[u8]) -> [u8; 32] {
    domain_hash(HASH_DOMAIN_PAYLOAD, canonical_payload)
}
