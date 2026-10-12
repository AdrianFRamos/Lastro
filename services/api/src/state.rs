//! Axum application dependencies.
//!
//! `AppState` is cheap to clone: PgPool is internally shared and configuration/RPC
//! are behind `Arc`. Required dependencies are provided through Axum `State`; do
//! not hide mandatory services in globals or optional request extensions.

use crate::{config::AppConfig, orgs::LocalKeyVault, solana::rpc::SolanaRpc, sponsor::Sponsor};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<AppConfig>,
    pub rpc: Arc<dyn SolanaRpc>,
    /// Platform fee sponsor; `None` when every signer pays for its own transactions.
    pub sponsor: Option<Arc<Sponsor>>,
    /// Organization key vault (S2); `None` disables managed organization wallets.
    pub vault: Option<Arc<LocalKeyVault>>,
}

impl AppState {
    pub fn sponsor(&self) -> Option<&Sponsor> {
        self.sponsor.as_deref()
    }

    pub fn vault(&self) -> Result<&LocalKeyVault, crate::error::ApiError> {
        self.vault.as_deref().ok_or_else(|| {
            crate::error::ApiError::Unavailable(
                "managed organization wallets are not configured".into(),
            )
        })
    }
}
