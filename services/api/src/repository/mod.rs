//! PostgreSQL persistence. Repository code never decides canonical Solana state.
//! Projection updates after confirmation run in one SQL transaction.

pub mod animals;
pub mod capture_authorizations;
pub mod captures;
pub mod events;
