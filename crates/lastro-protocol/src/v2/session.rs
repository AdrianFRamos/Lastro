//! Handling sessions and batched presence proofs (SPEC v2.1, S3/S4a/S8).
//!
//! A custodian authorizes one Station for a bounded window (`SessionGrant` on-chain). Every
//! presence proof read in that window is an ordinary OBSERVATION_RECORDED capture whose
//! capture id is derived from `(session_id, asset_id)`, so the Station firmware signs it with
//! the existing capture rule, the envelope is bound to its session, and an asset can be
//! observed at most once per session. Observations are anchored in batches: the program
//! stores one Merkle root per batch instead of one account per event, and does not verify each
//! leaf; the API and the independent verifier check every leaf's Station signature, envelope
//! and Merkle proof.

use sha2::{Digest, Sha256};

use super::{
    CaptureCommand, DomainEventEnvelope, EventType, V2_ENVELOPE_LEN, capture::capture_event_id,
    hash::domain_hash,
};
use crate::error::ProtocolError;

/// Longest window one session may authorize.
pub const MAX_SESSION_SECONDS: i64 = 12 * 3_600;
/// Most presence proofs one session may anchor.
pub const MAX_SESSION_EVENTS: u32 = 2_000;
/// Most leaves in one anchored batch (the EvidencePackage history bound).
pub const MAX_BATCH_LEAVES: u32 = 1_024;
/// A batch must be anchored within this time after its last observation (offline queues).
pub const MAX_BATCH_EVENT_AGE_SECONDS: i64 = 7 * 86_400;
/// `SessionGrant.allowed_actions` bit for presence proofs, the only action in v2.1.
pub const SESSION_ACTION_OBSERVE: u16 = 1;

pub const HASH_DOMAIN_SESSION_ID: &[u8] = b"LASTRO_V2_SESSION_ID\0";
pub const HASH_DOMAIN_SESSION_CAPTURE: &[u8] = b"LASTRO_V2_SESSION_CAPTURE\0";
pub const HASH_DOMAIN_BATCH_LEAF: &[u8] = b"LASTRO_V2_BATCH_LEAF\0";
pub const HASH_DOMAIN_BATCH_NODE: &[u8] = b"LASTRO_V2_BATCH_NODE\0";

/// The only id a session with these terms may have:
/// `SHA-256("LASTRO_V2_SESSION_ID\0" || deployment_id || custodian || station_id ||
/// valid_from i64 LE || expires_at i64 LE || max_events u32 LE)`.
///
/// `open_session` requires it, so nobody but the custodian can occupy the id the custodian's
/// Station will sign presence proofs for (the id binds the custodian), and the leaves of one
/// session can never be anchored under another custodian's grant.
pub fn session_id(
    deployment_id: &[u8; 32],
    custodian: &[u8; 32],
    station_id: &[u8; 32],
    valid_from: i64,
    expires_at: i64,
    max_events: u32,
) -> [u8; 32] {
    let mut bytes = [0u8; 32 * 3 + 8 + 8 + 4];
    bytes[..32].copy_from_slice(deployment_id);
    bytes[32..64].copy_from_slice(custodian);
    bytes[64..96].copy_from_slice(station_id);
    bytes[96..104].copy_from_slice(&valid_from.to_le_bytes());
    bytes[104..112].copy_from_slice(&expires_at.to_le_bytes());
    bytes[112..116].copy_from_slice(&max_events.to_le_bytes());
    domain_hash(HASH_DOMAIN_SESSION_ID, &bytes)
}

/// Capture id of the one presence proof `asset_id` may have in `session_id`.
pub fn session_capture_id(session_id: &[u8; 32], asset_id: &[u8; 32]) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(HASH_DOMAIN_SESSION_CAPTURE);
    hasher.update(session_id);
    hasher.update(asset_id);
    let digest: [u8; 32] = hasher.finalize().into();
    digest[..16].try_into().expect("digest has 32 bytes")
}

/// Event id of that presence proof (the Station derives it from the capture id).
pub fn session_event_id(session_id: &[u8; 32], asset_id: &[u8; 32]) -> [u8; 32] {
    capture_event_id(&session_capture_id(session_id, asset_id))
}

/// Context of one asset in a session manifest: the canonical head the observation extends.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionAssetContext {
    pub deployment_id: [u8; 32],
    pub session_id: [u8; 32],
    pub asset_id: [u8; 32],
    pub state_version: u64,
    pub previous_event_hash: [u8; 32],
    pub rfid_hash: [u8; 32],
}

impl SessionAssetContext {
    /// The capture command the Agent sends the Station for this asset; byte-compatible with
    /// every existing Station firmware.
    pub fn command(
        &self,
        observed_at: i64,
        expires_at: i64,
    ) -> Result<CaptureCommand, ProtocolError> {
        let capture_id = session_capture_id(&self.session_id, &self.asset_id);
        let command = CaptureCommand {
            capture_id,
            event_type: EventType::ObservationRecorded,
            deployment_id: self.deployment_id,
            asset_id: self.asset_id,
            event_id: capture_event_id(&capture_id),
            state_version: self.state_version,
            previous_event_hash: self.previous_event_hash,
            expected_rfid_hash: self.rfid_hash,
            observed_at,
            expires_at,
        };
        command.validate()?;
        Ok(command)
    }

    /// Whether `envelope` is exactly this asset's session observation (any time window).
    pub fn matches(&self, envelope: &DomainEventEnvelope) -> bool {
        self.command(envelope.observed_at, envelope.expires_at)
            .ok()
            .and_then(|command| {
                super::capture::capture_envelope_for_hash(
                    &command,
                    self.rfid_hash,
                    envelope.source_id,
                )
                .ok()
            })
            .is_some_and(|expected| expected == *envelope)
    }
}

/// Leaf committed by a batch root: the exact signed bytes of one presence proof.
pub fn batch_leaf_hash(envelope: &[u8; V2_ENVELOPE_LEN], station_signature: &[u8; 64]) -> [u8; 32] {
    let mut bytes = [0u8; V2_ENVELOPE_LEN + 64];
    bytes[..V2_ENVELOPE_LEN].copy_from_slice(envelope);
    bytes[V2_ENVELOPE_LEN..].copy_from_slice(station_signature);
    domain_hash(HASH_DOMAIN_BATCH_LEAF, &bytes)
}

fn batch_node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut bytes = [0u8; 64];
    bytes[..32].copy_from_slice(left);
    bytes[32..].copy_from_slice(right);
    domain_hash(HASH_DOMAIN_BATCH_NODE, &bytes)
}

fn check_leaf_count(len: usize) -> Result<(), ProtocolError> {
    if len == 0 || len > MAX_BATCH_LEAVES as usize {
        return Err(ProtocolError::InvalidBatch);
    }
    Ok(())
}

/// Merkle root over leaf hashes in batch order; an unpaired last node is paired with itself.
pub fn batch_merkle_root(leaves: &[[u8; 32]]) -> Result<[u8; 32], ProtocolError> {
    check_leaf_count(leaves.len())?;
    let mut level = leaves.to_vec();
    while level.len() > 1 {
        level = level
            .chunks(2)
            .map(|pair| batch_node(&pair[0], pair.get(1).unwrap_or(&pair[0])))
            .collect();
    }
    Ok(level[0])
}

/// Sibling path of `index`, bottom-up, for [`verify_batch_proof`].
pub fn batch_merkle_proof(
    leaves: &[[u8; 32]],
    index: usize,
) -> Result<Vec<[u8; 32]>, ProtocolError> {
    check_leaf_count(leaves.len())?;
    if index >= leaves.len() {
        return Err(ProtocolError::InvalidBatch);
    }
    let mut level = leaves.to_vec();
    let mut at = index;
    let mut proof = Vec::new();
    while level.len() > 1 {
        proof.push(*level.get(at ^ 1).unwrap_or(&level[at]));
        level = level
            .chunks(2)
            .map(|pair| batch_node(&pair[0], pair.get(1).unwrap_or(&pair[0])))
            .collect();
        at /= 2;
    }
    Ok(proof)
}

/// Whether `leaf` sits at `index` of a batch of `leaf_count` leaves with `root`.
pub fn verify_batch_proof(
    leaf: [u8; 32],
    index: u32,
    leaf_count: u32,
    proof: &[[u8; 32]],
    root: [u8; 32],
) -> bool {
    if leaf_count == 0 || leaf_count > MAX_BATCH_LEAVES || index >= leaf_count {
        return false;
    }
    // A batch of n leaves has exactly ceil(log2(n)) levels.
    let depth = (u32::BITS - (leaf_count - 1).leading_zeros()) as usize;
    if proof.len() != depth {
        return false;
    }
    let mut node = leaf;
    let mut at = index;
    for sibling in proof {
        node = if at & 1 == 0 {
            batch_node(&node, sibling)
        } else {
            batch_node(sibling, &node)
        };
        at >>= 1;
    }
    node == root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_id_binds_the_custodian_and_every_term() {
        // PURPOSE: M1 — another wallet can never occupy the id of a custodian's session.
        // FAILURE MEANS: a squatter could open the id first and capture the Station's proofs.
        let id = session_id(&[1; 32], &[2; 32], &[3; 32], 100, 200, 10);
        assert_eq!(
            hex::encode(id),
            hex::encode(domain_hash(
                b"LASTRO_V2_SESSION_ID\0",
                &[
                    [1u8; 32].as_slice(),
                    &[2; 32],
                    &[3; 32],
                    &100i64.to_le_bytes(),
                    &200i64.to_le_bytes(),
                    &10u32.to_le_bytes()
                ]
                .concat()
            ))
        );
        for other in [
            session_id(&[9; 32], &[2; 32], &[3; 32], 100, 200, 10),
            session_id(&[1; 32], &[9; 32], &[3; 32], 100, 200, 10),
            session_id(&[1; 32], &[2; 32], &[9; 32], 100, 200, 10),
            session_id(&[1; 32], &[2; 32], &[3; 32], 101, 200, 10),
            session_id(&[1; 32], &[2; 32], &[3; 32], 100, 201, 10),
            session_id(&[1; 32], &[2; 32], &[3; 32], 100, 200, 11),
        ] {
            assert_ne!(other, id);
        }
    }

    #[test]
    fn every_leaf_of_every_batch_size_proves_against_its_root() {
        // PURPOSE: the API's proofs and the verifier's check agree for odd and even sizes.
        // FAILURE MEANS: valid presence proofs could not be verified, or forged ones could.
        for count in [1usize, 2, 3, 5, 8, 9, 31] {
            let leaves: Vec<[u8; 32]> = (0..count).map(|index| [index as u8 + 1; 32]).collect();
            let root = batch_merkle_root(&leaves).unwrap();
            for (index, leaf) in leaves.iter().enumerate() {
                let proof = batch_merkle_proof(&leaves, index).unwrap();
                assert!(verify_batch_proof(
                    *leaf,
                    index as u32,
                    count as u32,
                    &proof,
                    root
                ));
                assert!(!verify_batch_proof(
                    [0xee; 32],
                    index as u32,
                    count as u32,
                    &proof,
                    root
                ));
                if count > 1 {
                    let other = (index + 1) % count;
                    assert!(!verify_batch_proof(
                        *leaf,
                        other as u32,
                        count as u32,
                        &proof,
                        root
                    ));
                }
            }
        }
        assert!(batch_merkle_root(&[]).is_err());
        assert!(batch_merkle_root(&vec![[1; 32]; MAX_BATCH_LEAVES as usize + 1]).is_err());
    }

    #[test]
    fn a_session_capture_is_bound_to_its_session_and_asset() {
        // PURPOSE: one presence proof per asset per session, recognizable without the API.
        // FAILURE MEANS: an observation could be replayed into another session or asset.
        let a = session_event_id(&[1; 32], &[2; 32]);
        assert_ne!(a, session_event_id(&[3; 32], &[2; 32]));
        assert_ne!(a, session_event_id(&[1; 32], &[4; 32]));
        let context = SessionAssetContext {
            deployment_id: [9; 32],
            session_id: [1; 32],
            asset_id: [2; 32],
            state_version: 3,
            previous_event_hash: [5; 32],
            rfid_hash: [6; 32],
        };
        let command = context.command(100, 200).unwrap();
        assert_eq!(command.event_id, a);
        assert_eq!(command.event_type, EventType::ObservationRecorded);
    }
}
