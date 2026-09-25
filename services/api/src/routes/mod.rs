//! Axum router composition. Routes are explicit so authorization boundaries remain reviewable.

pub mod agent;
pub mod animals;
pub mod captures;
pub mod domain_v2;
pub mod events;
pub mod evidence;
pub mod health;
pub mod transformations;

use crate::state::AppState;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::Method,
    routing::{get, post},
};
use tower_http::cors::{Any, CorsLayer};

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
        .route(
            "/api/animals/by-recovery/{visualRecoveryId}",
            get(animals::by_recovery),
        )
        .route("/api/animals/by-rfid/{rfidHash}", get(animals::by_rfid))
        .route(
            "/api/captures/authorization-challenge",
            post(captures::authorization_challenge),
        )
        .route("/api/captures", post(captures::create))
        .route("/api/captures/{captureId}", get(captures::get))
        .route("/api/agent/commands", get(agent::poll_command))
        .route("/api/agent/evidence", post(agent::submit_evidence))
        .route(
            "/api/v2/agent/observations",
            post(domain_v2::ingest_observation),
        )
        .route("/api/v2/events/{eventHash}", get(domain_v2::get_event))
        .route(
            "/api/v2/assets/{assetId}/timeline",
            get(domain_v2::timeline),
        )
        .route(
            "/api/v2/transformations/{transformationId}",
            get(transformations::get),
        )
        .route(
            "/api/v2/assets/{assetId}/lineage",
            get(transformations::lineage),
        )
        .route(
            "/api/agent/evidence/{eventHash}",
            get(agent::evidence_status),
        )
        .route(
            "/api/events/{eventHash}/transaction-data",
            get(events::transaction_data),
        )
        .route("/api/events/{eventHash}/submit", post(events::submit))
        .route("/api/events/{eventHash}/confirm", post(events::confirm))
        .route(
            "/api/animals/{animalId}/evidence-package",
            get(evidence::package),
        )
        .layer(DefaultBodyLimit::max(MAX_JSON_BODY_BYTES))
        .layer(cors)
        .with_state(state)
}
