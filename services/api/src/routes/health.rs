//! `GET /api/health` reports PostgreSQL and canonical Solana dependencies independently.

use axum::{extract::State, Json};
use serde::Serialize;

use crate::state::AppState;

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
    let rpc_ok = state
        .rpc
        .protocol_station_pubkey(state.config.deployment_id)
        .await
        .is_ok_and(|key| key == state.config.station_pubkey33);
    Json(HealthResponse {
        status: if database_ok && rpc_ok { "ok" } else { "degraded" },
        database: if database_ok { "ok" } else { "error" },
        rpc: if rpc_ok { "ok" } else { "error" },
    })
}
