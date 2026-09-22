//! Animal registration and recovery rules.
//!
//! `POST /animals` creates only the off-chain mapping `{random AnimalID,
//! visual_recovery_id}`. It MUST NOT create `AnimalState`, invent a custodian or
//! assign RFID; ORIGIN remains a separate physically observed/on-chain transition.
//!
//! Recovery semantics:
//! - visual ID present: local unique lookup can recover AnimalID;
//! - known current RFID hash: query canonical `RfidBinding`, require ACTIVE, then resolve
//!   its AnimalID/projection. The hackathon does not add a separate Station "observation-only"
//!   command solely for recovery; acquiring a hash from a fresh physical read is a later UX/integration;
//! - retired binding: never return it as current;
//! - both identifiers unavailable: return UNRESOLVED; do not infer identity from
//!   operator guess, location or other metadata.

use rand::RngCore;
use lastro_protocol::ids::AnimalId;

pub fn generate_animal_id() -> AnimalId {
    let mut out = [0u8; 32];
    rand::rng().fill_bytes(&mut out);
    out
}
