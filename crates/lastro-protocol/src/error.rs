//! Precise protocol errors shared by tests, services, and verifier reason codes.

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum ProtocolError {
    #[error("invalid StationEvent length")]
    InvalidLength,
    #[error("invalid magic")]
    InvalidMagic,
    #[error("unsupported version")]
    UnsupportedVersion,
    #[error("invalid action")]
    InvalidAction,
    #[error("reserved bytes must be zero")]
    ReservedNotZero,
    #[error("invalid event semantics")]
    InvalidSemantics,
    #[error("event is not the canonical successor")]
    InvalidSuccessor,
    #[error("invalid canonical RFID length")]
    InvalidRfidLength,
    #[error("invalid station public key")]
    InvalidStationKey,
    #[error("invalid station signature")]
    InvalidStationSignature,
    #[error("high-S signature rejected")]
    HighSSignature,
    #[error("invalid EvidencePackage")]
    InvalidEvidencePackage,
    #[error("invalid v2 envelope length")]
    InvalidV2EnvelopeLength,
    #[error("unsupported v2 schema version")]
    UnsupportedV2SchemaVersion,
    #[error("unknown v2 enum value")]
    UnknownV2Enum,
    #[error("invalid v2 event time window")]
    InvalidV2TimeWindow,
    #[error("invalid v2 identifier")]
    InvalidV2Identifier,
}
