//! Canonical lineage and composition commitments.
//!
//! A lineage leaf is fixed-width and independent of JSON serialization. Leaves are
//! ordered by their explicit position, and the Merkle tree uses separate domains
//! for leaves and internal nodes. This makes roots reproducible across Rust,
//! TypeScript, and other implementations while still allowing partial proofs.

use crate::error::ProtocolError;

use super::{
    LineageRole,
    asset::{AssetId, require_nonzero_id},
    constants::{HASH_DOMAIN_LINEAGE_LEAF, HASH_DOMAIN_LINEAGE_NODE},
    hash::domain_hash,
};

pub const LINEAGE_LEAF_LEN: usize = 32 + 1 + 4 + 8 + 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineageLeaf {
    pub asset_id: AssetId,
    pub role: LineageRole,
    pub position: u32,
    pub quantity: u64,
    pub weight_grams: u64,
}

impl LineageLeaf {
    pub fn encode(self) -> [u8; LINEAGE_LEAF_LEN] {
        let mut bytes = [0u8; LINEAGE_LEAF_LEN];
        bytes[0..32].copy_from_slice(&self.asset_id);
        bytes[32] = self.role as u8;
        bytes[33..37].copy_from_slice(&self.position.to_le_bytes());
        bytes[37..45].copy_from_slice(&self.quantity.to_le_bytes());
        bytes[45..53].copy_from_slice(&self.weight_grams.to_le_bytes());
        bytes
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        require_nonzero_id(&self.asset_id)?;
        if self.quantity == 0 && self.weight_grams == 0 {
            return Err(ProtocolError::InvalidLineageLeaf);
        }
        Ok(())
    }

    pub fn hash(&self) -> Result<[u8; 32], ProtocolError> {
        self.validate()?;
        Ok(domain_hash(HASH_DOMAIN_LINEAGE_LEAF, &self.encode()))
    }
}

/// Computes a deterministic Merkle root from canonical leaves.
///
/// The caller supplies leaves in any order; the function sorts by position and
/// rejects duplicate positions. The last node is duplicated when a level has an
/// odd number of nodes, which is part of the v2 wire rule.
pub fn merkle_root(leaves: &[LineageLeaf]) -> Result<[u8; 32], ProtocolError> {
    if leaves.is_empty() {
        return Err(ProtocolError::EmptyLineage);
    }

    let mut ordered = leaves.to_vec();
    ordered.sort_by_key(|leaf| leaf.position);
    for pair in ordered.windows(2) {
        if pair[0].position == pair[1].position {
            return Err(ProtocolError::DuplicateLineagePosition);
        }
    }

    let mut level = ordered
        .iter()
        .map(LineageLeaf::hash)
        .collect::<Result<Vec<_>, _>>()?;

    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let right = pair.get(1).copied().unwrap_or(pair[0]);
            let mut bytes = [0u8; 64];
            bytes[..32].copy_from_slice(&pair[0]);
            bytes[32..].copy_from_slice(&right);
            next.push(domain_hash(HASH_DOMAIN_LINEAGE_NODE, &bytes));
        }
        level = next;
    }

    Ok(level[0])
}

/// Maximum proof depth accepted by verifiers (2^32 leaves); bounds on-chain work.
pub const MAX_LINEAGE_PROOF_DEPTH: usize = 32;

/// Verifies that `leaf_hash` sits at `leaf_index` (index in the position-sorted leaf
/// list, not the leaf's `position` field) of a tree built by [`merkle_root`].
///
/// `proof` lists the sibling of each level, bottom-up. When a node is the unpaired last
/// node of its level, its sibling is the node itself (the duplication rule).
pub fn verify_merkle_proof(
    leaf_hash: [u8; 32],
    leaf_index: u32,
    proof: &[[u8; 32]],
    root: [u8; 32],
) -> bool {
    if proof.len() > MAX_LINEAGE_PROOF_DEPTH {
        return false;
    }
    let mut node = leaf_hash;
    let mut index = u64::from(leaf_index);
    for sibling in proof {
        let mut bytes = [0u8; 64];
        if index & 1 == 0 {
            bytes[..32].copy_from_slice(&node);
            bytes[32..].copy_from_slice(sibling);
        } else {
            bytes[..32].copy_from_slice(sibling);
            bytes[32..].copy_from_slice(&node);
        }
        node = domain_hash(HASH_DOMAIN_LINEAGE_NODE, &bytes);
        index >>= 1;
    }
    index == 0 && node == root
}

/// Builds the sibling path for the leaf at `leaf_index` of the position-sorted leaves.
/// Off-chain helper used by clients and tests; mirrors [`merkle_root`] exactly.
pub fn merkle_proof(
    leaves: &[LineageLeaf],
    leaf_index: usize,
) -> Result<Vec<[u8; 32]>, ProtocolError> {
    merkle_root(leaves)?;
    let mut ordered = leaves.to_vec();
    ordered.sort_by_key(|leaf| leaf.position);
    if leaf_index >= ordered.len() {
        return Err(ProtocolError::InvalidLineageLeaf);
    }
    let mut level = ordered
        .iter()
        .map(LineageLeaf::hash)
        .collect::<Result<Vec<_>, _>>()?;
    let mut index = leaf_index;
    let mut proof = Vec::new();
    while level.len() > 1 {
        let sibling = level.get(index ^ 1).copied().unwrap_or(level[index]);
        proof.push(sibling);
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let right = pair.get(1).copied().unwrap_or(pair[0]);
            let mut bytes = [0u8; 64];
            bytes[..32].copy_from_slice(&pair[0]);
            bytes[32..].copy_from_slice(&right);
            next.push(domain_hash(HASH_DOMAIN_LINEAGE_NODE, &bytes));
        }
        level = next;
        index /= 2;
    }
    Ok(proof)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: u8, position: u32, weight: u64) -> LineageLeaf {
        LineageLeaf {
            asset_id: [id; 32],
            role: LineageRole::Input,
            position,
            quantity: 1,
            weight_grams: weight,
        }
    }

    #[test]
    fn root_is_independent_of_input_order_but_sensitive_to_content() {
        let a = leaf(1, 0, 100);
        let b = leaf(2, 1, 200);
        assert_eq!(merkle_root(&[a, b]), merkle_root(&[b, a]));
        assert_ne!(merkle_root(&[a, b]), merkle_root(&[a, leaf(2, 1, 201)]));
    }

    #[test]
    fn duplicate_positions_are_rejected() {
        let error = merkle_root(&[leaf(1, 0, 100), leaf(2, 0, 200)]).unwrap_err();
        assert_eq!(error, ProtocolError::DuplicateLineagePosition);
    }

    #[test]
    fn every_leaf_proof_verifies_and_tampering_fails() {
        for count in 1..=9u32 {
            let leaves: Vec<_> = (0..count)
                .map(|i| leaf(i as u8 + 1, i * 3 + 7, 100 + u64::from(i)))
                .collect();
            let root = merkle_root(&leaves).unwrap();
            for (index, item) in leaves.iter().enumerate() {
                let proof = merkle_proof(&leaves, index).unwrap();
                let hash = item.hash().unwrap();
                assert!(verify_merkle_proof(hash, index as u32, &proof, root));
                let out_of_range = (index as u32) | (1 << 20);
                assert!(!verify_merkle_proof(hash, out_of_range, &proof, root));
                let other = leaf(0xee, item.position, item.weight_grams).hash().unwrap();
                assert!(!verify_merkle_proof(other, index as u32, &proof, root));
            }
        }
    }

    #[test]
    fn empty_lineage_is_rejected() {
        assert_eq!(merkle_root(&[]).unwrap_err(), ProtocolError::EmptyLineage);
    }
}
