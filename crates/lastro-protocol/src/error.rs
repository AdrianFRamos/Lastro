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
}
