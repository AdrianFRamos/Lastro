//! Versioned domain protocol for the bovine traceability expansion.

pub mod asset;
pub mod capture;
pub mod constants;
pub mod envelope;
pub mod geo;
pub mod hash;
pub mod identifier;
pub mod intent;
pub mod lineage;
pub mod session;
pub mod transformation;

pub use asset::{
    AssetId, AssetStatus, AssetType, EventId, EventType, FacilityId, FacilityStatus, FacilityType,
    IntentId, IntentStatus, IntentType, LineageRole, PartyId, ProvenanceType, RecallId,
    RecallStatus, StationId, StationStatus, TransformationId, TransformationStatus, UnitCode,
};
pub use capture::{
    CAPTURE_COMMAND_LEN, CAPTURE_EVENT_READY_LEN, CaptureCommand, CaptureEventReady,
    capture_envelope, capture_envelope_for_hash, capture_event_id,
};
pub use envelope::{DomainEventEnvelope, V2_ENVELOPE_LEN};
pub use geo::{FacilityBoundary, VertexE7};
pub use hash::{domain_hash, event_hash, payload_hash};
pub use identifier::{IDENTIFIER_PAYLOAD_LEN, identifier_payload, identifier_payload_hash};
pub use intent::custody_transfer_payload_hash;
pub use lineage::{
    LINEAGE_LEAF_LEN, LineageLeaf, MAX_LINEAGE_PROOF_DEPTH, merkle_proof, merkle_root,
    verify_merkle_proof,
};
pub use session::{
    MAX_BATCH_EVENT_AGE_SECONDS, MAX_BATCH_LEAVES, MAX_SESSION_EVENTS, MAX_SESSION_SECONDS,
    SESSION_ACTION_OBSERVE, SessionAssetContext, batch_leaf_hash, batch_merkle_proof,
    batch_merkle_root, session_capture_id, session_event_id, session_id, verify_batch_proof,
};
pub use transformation::{MassBalance, TransformationManifest};
