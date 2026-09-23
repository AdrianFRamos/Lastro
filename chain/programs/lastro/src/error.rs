//! Stable on-chain errors. Tests should assert exact variants for adversarial paths.

use anchor_lang::prelude::*;

#[error_code]
pub enum LastroError {
    #[msg("invalid StationEvent bytes")]
    InvalidEvent,
    #[msg("Station signature precompile binding missing or invalid")]
    InvalidStationProof,
    #[msg("custodian signer does not match canonical state")]
    InvalidCustodian,
    #[msg("event sequence does not advance exactly once")]
    InvalidSequence,
    #[msg("identity revision is invalid for this action")]
    InvalidRevision,
    #[msg("previous event hash does not match canonical state")]
    InvalidPredecessor,
    #[msg("RFID transition is inconsistent with canonical state")]
    InvalidRfidTransition,
    #[msg("RFID has already been bound in this deployment")]
    RfidAlreadyBound,
    #[msg("RFID binding is not active for this AnimalID")]
    RfidBindingMismatch,
    #[msg("origin already exists for this AnimalID")]
    OriginAlreadyExists,
}
