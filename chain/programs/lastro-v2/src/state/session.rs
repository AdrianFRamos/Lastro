//! Handling sessions and their anchored observation batches (SPEC v2.1, S3/S4a/S8).

use anchor_lang::prelude::*;

/// One custodian's authorization for one Station to record presence proofs in a bounded
/// window. Observations are anchored in batches; the grant tracks how many it has admitted.
#[account]
pub struct SessionGrant {
    pub session_id: [u8; 32],
    pub custodian: Pubkey,
    pub station_id: [u8; 32],
    pub allowed_actions: u16,
    pub max_events: u32,
    pub events_used: u32,
    pub batch_count: u32,
    pub valid_from: i64,
    pub expires_at: i64,
    /// Zero while the custodian has not revoked the session.
    pub revoked_at: i64,
    pub status: u8,
    pub rent_payer: Pubkey,
    /// Facility the Station was bound to when the session opened (S5).
    pub facility_id: [u8; 32],
    pub bump: u8,
}

impl SessionGrant {
    pub const SPACE: usize = 8 + 32 + 32 + 32 + 2 + 4 + 4 + 4 + 8 + 8 + 8 + 1 + 32 + 32 + 1;

    /// Latest instant an admitted observation may carry.
    pub fn observation_deadline(&self) -> i64 {
        if self.revoked_at == 0 {
            self.expires_at
        } else {
            self.revoked_at.min(self.expires_at)
        }
    }
}

/// Merkle commitment to up to 1 024 Station-signed presence proofs of one session. It copies
/// the session terms a verifier needs, so the grant itself can later be closed.
#[account]
pub struct BatchAnchor {
    pub session_id: [u8; 32],
    pub batch_index: u32,
    pub custodian: Pubkey,
    pub station_id: [u8; 32],
    pub merkle_root: [u8; 32],
    pub leaf_count: u32,
    pub first_observed_at: i64,
    pub last_observed_at: i64,
    pub anchored_at: i64,
    /// Facility where the batch's observations were made (copied from the session).
    pub facility_id: [u8; 32],
    pub bump: u8,
}

impl BatchAnchor {
    pub const SPACE: usize = 8 + 32 + 4 + 32 + 32 + 32 + 4 + 8 + 8 + 8 + 32 + 1;
}
