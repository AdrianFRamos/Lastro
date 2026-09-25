//! Versioned domain protocol for the bovine traceability expansion.
//!
//! The v2 wire contract is deliberately separate from the 276-byte StationEvent v1.

pub mod asset;
pub mod constants;
pub mod envelope;
pub mod hash;
pub mod lineage;
pub mod transformation;

pub use asset::{
    AssetId, AssetStatus, AssetType, EventId, EventType, FacilityId, FacilityStatus, FacilityType,
    IntentId, IntentStatus, IntentType, LineageRole, PartyId, ProvenanceType, RecallId,
    RecallStatus, StationId, StationStatus, TransformationId, TransformationStatus, UnitCode,
};
pub use envelope::{DomainEventEnvelope, V2_ENVELOPE_LEN};
pub use hash::{domain_hash, event_hash, payload_hash};
pub use lineage::{LINEAGE_LEAF_LEN, LineageLeaf, merkle_root};
pub use transformation::{MassBalance, TransformationManifest};
