#![forbid(unsafe_code)]
//! Agent library boundary.
//!
//! Keep serial parsing, durable outbox, API transport and worker orchestration separated.
//! `main.rs` only wires configuration/tracing/runtime; domain behavior belongs in these modules
//! so retry/restart/serial contracts can be tested without a physical process.

pub mod api_client;
pub mod command;
pub mod config;
pub mod error;
pub mod serial;
pub mod spool;
pub mod worker;
