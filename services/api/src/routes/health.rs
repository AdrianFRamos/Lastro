//! `GET /api/health` reports PostgreSQL and canonical Solana dependencies independently.

use axum::{Json, extract::State};
use serde::Serialize;

use crate::{routes::captures::require_usable_station, state::AppState};

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub database: &'static str,
    pub rpc: &'static str,
}

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let database_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();
    // Healthy only when the deployment's v2 ProtocolConfig is readable at finalized commitment
    // and the configured Station can currently have its evidence anchored.
    let rpc_ok = state
        .rpc
        .v2_protocol_config(state.config.deployment_id)
        .await
        .is_ok_and(|config| config.is_some())
        && require_usable_station(&state, 0).await.is_ok();
    Json(HealthResponse {
        status: if database_ok && rpc_ok {
            "ok"
        } else {
            "degraded"
        },
        database: if database_ok { "ok" } else { "error" },
        rpc: if rpc_ok { "ok" } else { "error" },
    })
}
