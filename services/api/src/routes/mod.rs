//! Axum router composition. Routes are explicit so authorization boundaries remain reviewable.

pub mod agent;
pub mod animals;
pub mod captures;
pub mod domain_v2;
pub mod events;
pub mod evidence;
pub mod health;
pub mod migration;
pub mod operations;
pub mod processing;
pub mod transformations;

use crate::state::AppState;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::Method,
    routing::{get, post},
};
use tower_http::cors::{Any, CorsLayer};

/// Operational lot admission can carry up to 512 fixed-size asset references. The
/// body stays bounded well below Axum's generic 2 MiB buffer.
pub const MAX_JSON_BODY_BYTES: usize = 64 * 1024;

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
            "/api/v2/events/{eventHash}/transaction-data",
            get(domain_v2::transaction_data),
        )
        .route("/api/v2/events/{eventHash}/submit", post(domain_v2::submit))
        .route(
            "/api/v2/events/{eventHash}/confirm",
            post(domain_v2::confirm),
        )
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
        .route("/api/v2/parties", post(operations::create_party))
        .route("/api/v2/parties/{partyId}", get(operations::get_party))
        .route(
            "/api/v2/parties/{partyId}/status",
            post(operations::set_party_status),
        )
        .route("/api/v2/facilities", post(operations::create_facility))
        .route(
            "/api/v2/facilities/{facilityId}",
            get(operations::get_facility),
        )
        .route(
            "/api/v2/facilities/{facilityId}/status",
            post(operations::set_facility_status),
        )
        .route("/api/v2/lots", post(operations::create_lot))
        .route("/api/v2/lots/{lotId}", get(operations::get_lot))
        .route(
            "/api/v2/custody-transfers",
            post(operations::create_custody_transfer),
        )
        .route(
            "/api/v2/custody-transfers/{transferId}",
            get(operations::get_custody_transfer),
        )
        .route(
            "/api/v2/custody-transfers/{transferId}/accept",
            post(operations::accept_custody_transfer),
        )
        .route("/api/v2/processing", post(processing::create_processing))
        .route(
            "/api/v2/processing/{operationId}",
            get(processing::get_processing),
        )
        .route(
            "/api/v2/processing/{operationId}/ready",
            post(processing::ready_processing),
        )
        .route(
            "/api/v2/processing/{operationId}/finalize",
            post(processing::finalize_processing),
        )
        .route("/api/v2/shipments", post(processing::create_shipment))
        .route(
            "/api/v2/shipments/{shipmentId}",
            get(processing::get_shipment),
        )
        .route(
            "/api/v2/shipments/{shipmentId}/status",
            post(processing::set_shipment_status),
        )
        .route("/api/v2/recalls", post(processing::open_recall))
        .route("/api/v2/recalls/{recallId}", get(processing::get_recall))
        .route(
            "/api/v2/recalls/{recallId}/close",
            post(processing::close_recall),
        )
        .route(
            "/api/v2/authority-grants",
            post(operations::create_authority_grant),
        )
        .route(
            "/api/v2/authority-grants/{grantId}/revoke",
            post(operations::revoke_authority_grant),
        )
        .route("/api/v2/audit", get(operations::list_audit_entries))
        .route("/api/v2/migrations/v1/plan", post(migration::plan))
        .route("/api/v2/migrations/{runId}", get(migration::get))
        .route(
            "/api/v2/migrations/{runId}/candidates/{animalId}/promote",
            post(migration::promote),
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
