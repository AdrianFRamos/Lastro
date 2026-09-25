//! Versioned domain protocol for the bovine traceability expansion.
//!
//! The v2 wire contract is deliberately separate from the 276-byte StationEvent v1.

pub mod asset;
pub mod constants;
pub mod envelope;
pub mod hash;

pub use asset::{
    AssetId, AssetStatus, AssetType, EventId, EventType, FacilityId, IntentId, IntentStatus,
    IntentType, PartyId, RecallId, StationId, TransformationId,
};
pub use envelope::{DomainEventEnvelope, V2_ENVELOPE_LEN};
pub use hash::{domain_hash, event_hash, payload_hash};
