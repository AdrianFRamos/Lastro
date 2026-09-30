//! Physical identifier (RFID) payload committed by the Station for v2 identity events.
//!
//! The Station signs a domain envelope whose `payload_hash` commits to these 64 bytes:
//! `old_rfid_hash || new_rfid_hash`. IDENTIFIER_BOUND uses an all-zero old hash;
//! IDENTIFIER_REPLACED retires `old` and binds `new` to the same AssetID.

use super::hash::payload_hash;

pub const IDENTIFIER_PAYLOAD_LEN: usize = 64;

pub fn identifier_payload(
    old_rfid_hash: [u8; 32],
    new_rfid_hash: [u8; 32],
) -> [u8; IDENTIFIER_PAYLOAD_LEN] {
    let mut bytes = [0u8; IDENTIFIER_PAYLOAD_LEN];
    bytes[..32].copy_from_slice(&old_rfid_hash);
    bytes[32..].copy_from_slice(&new_rfid_hash);
    bytes
}

pub fn identifier_payload_hash(old_rfid_hash: [u8; 32], new_rfid_hash: [u8; 32]) -> [u8; 32] {
    payload_hash(&identifier_payload(old_rfid_hash, new_rfid_hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_is_ordered_and_position_sensitive() {
        let bound = identifier_payload_hash([0; 32], [7; 32]);
        assert_ne!(bound, identifier_payload_hash([7; 32], [0; 32]));
        assert_ne!(bound, identifier_payload_hash([0; 32], [8; 32]));
        assert_eq!(&identifier_payload([1; 32], [2; 32])[32..], &[2; 32]);
    }
}
