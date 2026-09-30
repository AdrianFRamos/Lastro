//! PostgreSQL persistence. Repository code never decides canonical Solana state.
//! Projection updates after confirmation run in one SQL transaction.

pub mod captures_v2;
pub mod domain_v2;
pub mod operations;
pub mod processing;
pub mod reconciliation;
pub mod transformations;
