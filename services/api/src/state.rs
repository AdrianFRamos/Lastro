//! Axum application dependencies.
//!
//! `AppState` is cheap to clone: PgPool is internally shared and configuration/RPC
//! are behind `Arc`. Required dependencies are provided through Axum `State`; do
//! not hide mandatory services in globals or optional request extensions.

use crate::{config::AppConfig, solana::rpc::SolanaRpc};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<AppConfig>,
    pub rpc: Arc<dyn SolanaRpc>,
}
