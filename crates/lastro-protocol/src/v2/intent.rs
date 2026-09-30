//! Canonical intent payload commitments shared by the v2 program and its clients.

use super::{constants::HASH_DOMAIN_INTENT, hash::domain_hash};

/// Payload committed by the current custodian when proposing a custody transfer.
/// The receiving wallet must present exactly these values to accept on-chain.
pub fn custody_transfer_payload_hash(
    deployment_id: [u8; 32],
    asset_id: [u8; 32],
    new_custodian: [u8; 32],
    expected_state_version: u64,
    nonce: u64,
) -> [u8; 32] {
    let mut bytes = [0u8; 32 * 3 + 8 + 8];
    bytes[0..32].copy_from_slice(&deployment_id);
    bytes[32..64].copy_from_slice(&asset_id);
    bytes[64..96].copy_from_slice(&new_custodian);
    bytes[96..104].copy_from_slice(&expected_state_version.to_le_bytes());
    bytes[104..112].copy_from_slice(&nonce.to_le_bytes());
    domain_hash(HASH_DOMAIN_INTENT, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_binds_every_field() {
        let base = custody_transfer_payload_hash([1; 32], [2; 32], [3; 32], 4, 5);
        assert_ne!(
            base,
            custody_transfer_payload_hash([9; 32], [2; 32], [3; 32], 4, 5)
        );
        assert_ne!(
            base,
            custody_transfer_payload_hash([1; 32], [9; 32], [3; 32], 4, 5)
        );
        assert_ne!(
            base,
            custody_transfer_payload_hash([1; 32], [2; 32], [9; 32], 4, 5)
        );
        assert_ne!(
            base,
            custody_transfer_payload_hash([1; 32], [2; 32], [3; 32], 9, 5)
        );
        assert_ne!(
            base,
            custody_transfer_payload_hash([1; 32], [2; 32], [3; 32], 4, 9)
        );
    }
}
