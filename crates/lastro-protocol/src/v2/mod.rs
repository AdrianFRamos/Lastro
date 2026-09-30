//! Versioned domain protocol for the bovine traceability expansion.

pub mod asset;
pub mod capture;
pub mod constants;
pub mod envelope;
pub mod hash;
pub mod identifier;
pub mod intent;
pub mod lineage;
pub mod transformation;

pub use asset::{
    AssetId, AssetStatus, AssetType, EventId, EventType, FacilityId, FacilityStatus, FacilityType,
    IntentId, IntentStatus, IntentType, LineageRole, PartyId, ProvenanceType, RecallId,
    RecallStatus, StationId, StationStatus, TransformationId, TransformationStatus, UnitCode,
};
pub use capture::{
    CAPTURE_COMMAND_LEN, CAPTURE_EVENT_READY_LEN, CaptureCommand, CaptureEventReady,
    capture_envelope, capture_event_id,
};
pub use envelope::{DomainEventEnvelope, V2_ENVELOPE_LEN};
pub use hash::{domain_hash, event_hash, payload_hash};
pub use identifier::{IDENTIFIER_PAYLOAD_LEN, identifier_payload, identifier_payload_hash};
pub use intent::custody_transfer_payload_hash;
pub use lineage::{
    LINEAGE_LEAF_LEN, LineageLeaf, MAX_LINEAGE_PROOF_DEPTH, merkle_proof, merkle_root,
    verify_merkle_proof,
};
pub use transformation::{MassBalance, TransformationManifest};
