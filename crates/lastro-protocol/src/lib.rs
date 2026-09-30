//! Pure Lastro protocol types and invariants.
//!
//! This crate is the reference for encoding, hashing, and off-chain evidence checks used
//! by the Agent, API, vectors, and tests. It has no HTTP, database, serial-port, or RPC concerns.

#![forbid(unsafe_code)]

pub mod constants;
pub mod crypto;
pub mod error;
pub mod ids;
pub mod rfid;
pub mod v2;

pub use error::ProtocolError;
pub use ids::*;
