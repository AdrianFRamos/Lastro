//! Axum application dependencies.
//!
//! `AppState` is cheap to clone: PgPool is internally shared and configuration/RPC
//! are behind `Arc`. Required dependencies are provided through Axum `State`; do
//! not hide mandatory services in globals or optional request extensions.

use std::sync::Arc;
use sqlx::PgPool;
use crate::{config::AppConfig, solana::rpc::SolanaRpc};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<AppConfig>,
    pub rpc: Arc<dyn SolanaRpc>,
}
