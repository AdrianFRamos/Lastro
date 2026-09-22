#![forbid(unsafe_code)]
//! Lastro coordination/projection API library.
//!
//! Canonical custody/current-RFID state lives on Solana. PostgreSQL is an append-only
//! evidence store plus query projection and must never be used to bypass chain rules.

pub mod config;
pub mod crypto;
pub mod db;
pub mod domain;
pub mod error;
pub mod model;
pub mod repository;
pub mod routes;
pub mod solana;
pub mod state;
