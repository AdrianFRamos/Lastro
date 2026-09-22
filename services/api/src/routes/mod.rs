//! Axum router composition. Routes are explicit so authorization boundaries remain reviewable.

pub mod agent;
pub mod animals;
pub mod captures;
pub mod events;
pub mod evidence;
pub mod health;

use axum::{extract::DefaultBodyLimit, http::Method, routing::{get, post}, Router};
use tower_http::cors::{Any, CorsLayer};
use crate::state::AppState;

/// The largest request is AgentEvidenceRequest: compact canonical JSON is 720 bytes
/// (276-byte StationEvent -> 368 base64 chars plus fixed UUID/hex fields). 1 KiB
/// leaves representation headroom while avoiding Axum's generic 2 MiB JSON buffer.
pub const MAX_JSON_BODY_BYTES: usize = 1024;

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    Router::new()
        .route("/api/health", get(health::health))
        .route("/api/animals", post(animals::create))
        .route("/api/animals/{animalId}", get(animals::get))
        .route("/api/animals/by-recovery/{visualRecoveryId}", get(animals::by_recovery))
        .route("/api/animals/by-rfid/{rfidHash}", get(animals::by_rfid))
        .route("/api/captures", post(captures::create))
        .route("/api/captures/{captureId}", get(captures::get))
        .route("/api/agent/commands", get(agent::poll_command))
        .route("/api/agent/evidence", post(agent::submit_evidence))
        .route("/api/agent/evidence/{eventHash}", get(agent::evidence_status))
        .route("/api/events/{eventHash}/transaction-data", get(events::transaction_data))
        .route("/api/events/{eventHash}/submit", post(events::submit))
        .route("/api/events/{eventHash}/confirm", post(events::confirm))
        .route("/api/animals/{animalId}/evidence-package", get(evidence::package))
        .layer(DefaultBodyLimit::max(MAX_JSON_BODY_BYTES))
        .layer(cors)
        .with_state(state)
}
