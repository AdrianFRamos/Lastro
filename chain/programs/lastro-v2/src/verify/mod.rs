//! On-chain verification helpers for physical observations.

pub mod event;
pub mod secp256r1;

pub use event::{parse_observation, parse_station_event};
pub use secp256r1::verify_station_precompile_binding;
