//! Axum router composition. Routes are explicit so authorization boundaries remain reviewable.

pub(crate) mod access;
pub mod agent;
pub mod assets;
pub mod captures;
pub mod domain_v2;
pub mod geo;
pub mod health;
pub mod operations;
pub mod orgs;
pub mod processing;
pub mod sessions;
pub mod sponsor;
pub mod transformations;

use crate::{error::ApiError, state::AppState};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::Method,
    routing::{get, post},
};
use std::time::Duration;
use tower::limit::GlobalConcurrencyLimitLayer;
use tower_http::{
    cors::{AllowOrigin, Any, CorsLayer},
    timeout::TimeoutLayer,
};

/// Requests served at once; further requests wait instead of exhausting the database pool and
/// the Solana RPC quota.
pub const MAX_CONCURRENT_REQUESTS: usize = 64;
/// Upper bound of any request, RPC round-trips included (each RPC call is bounded to 10 s).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Public and Agent routes carry fixed-size protocol payloads. The largest is
/// AgentEvidenceRequest (~620 bytes of compact JSON); see docs/API.md.
pub const MAX_JSON_BODY_BYTES: usize = 1024;

/// Operational lot admission can carry up to 512 fixed-size asset references. The
/// body stays bounded well below Axum's generic 2 MiB buffer.
pub const MAX_OPERATIONS_JSON_BODY_BYTES: usize = 64 * 1024;

/// A sponsored submission carries one legacy transaction (at most 1232 bytes, 1644 Base64
/// characters) plus its purpose; 4 KiB bounds it with room for the JSON envelope.
pub const MAX_SPONSOR_JSON_BODY_BYTES: usize = 4 * 1024;

/// A facility boundary of up to 10 000 vertices as `[lon, lat]` decimal pairs (S5).
pub const MAX_BOUNDARY_JSON_BODY_BYTES: usize = 512 * 1024;

pub fn router(state: AppState) -> Router {
    let origins = &state.config.cors_allowed_origins;
    let allow_origin = if origins.is_empty() {
        AllowOrigin::any()
    } else {
        AllowOrigin::list(origins.iter().cloned())
    };
    let cors = CorsLayer::new()
        .allow_origin(allow_origin)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    // Physical capture, Agent, event and asset routes: fixed-size payloads, 1 KiB.
    let protocol = Router::new()
        .route("/api/health", get(health::health))
        .route(
            "/api/captures/authorization-challenge",
            post(captures::authorization_challenge),
        )
        .route("/api/captures", post(captures::create))
        .route("/api/captures/{captureId}", get(captures::get))
        .route("/api/agent/commands", get(agent::poll_command))
        .route("/api/agent/evidence", post(agent::submit_evidence))
        .route(
            "/api/agent/evidence/{eventHash}",
            get(agent::evidence_status),
        )
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
            "/api/v2/assets/transaction-data",
            post(assets::registration_transaction),
        )
        .route("/api/v2/assets/by-rfid/{rfidHash}", get(assets::by_rfid))
        .route("/api/v2/assets/{assetId}", get(assets::get))
        .route("/api/v2/assets/{assetId}/sync", post(assets::sync))
        .route(
            "/api/v2/assets/{assetId}/evidence-package",
            get(assets::evidence_package),
        )
        .route(
            "/api/v2/assets/{assetId}/timeline",
            get(domain_v2::timeline),
        )
        .route(
            "/api/v2/assets/{assetId}/lineage",
            get(transformations::lineage),
        )
        .route(
            "/api/v2/custody-transfers/{transferId}/transaction-data",
            get(operations::custody_transfer_transaction_data),
        )
        .route(
            "/api/v2/custody-transfers/{transferId}/accept",
            post(operations::accept_custody_transfer),
        )
        .route("/api/v2/sessions", post(sessions::create))
        .route("/api/v2/sessions/{sessionId}", get(sessions::get))
        .route(
            "/api/v2/sessions/{sessionId}/confirm",
            post(sessions::confirm),
        )
        .route(
            "/api/v2/sessions/{sessionId}/manifest",
            get(sessions::manifest),
        )
        .route(
            "/api/v2/sessions/{sessionId}/observations",
            post(sessions::ingest_observation),
        )
        .route(
            "/api/v2/sessions/{sessionId}/batches",
            post(sessions::prepare_batch),
        )
        .route(
            "/api/v2/sessions/{sessionId}/batches/{batchIndex}/confirm",
            post(sessions::confirm_batch),
        )
        .route(
            "/api/v2/sessions/{sessionId}/revoke",
            post(sessions::revoke),
        )
        .route("/api/v2/sessions/{sessionId}/sync", post(sessions::sync))
        .layer(DefaultBodyLimit::max(MAX_JSON_BODY_BYTES));

    // Operator-authenticated registry and processing routes: up to 64 KiB.
    let operations = Router::new()
        .route("/api/v2/transformations", post(transformations::register))
        .route(
            "/api/v2/transformations/{transformationId}",
            get(transformations::get),
        )
        .route(
            "/api/v2/transformations/{transformationId}/sync",
            post(transformations::sync),
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
        .route(
            "/api/v2/facilities/{facilityId}/transaction-data",
            post(geo::facility_transaction),
        )
        .route(
            "/api/v2/stations/{stationId}/facility/transaction-data",
            post(geo::station_facility_transaction),
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
        .layer(DefaultBodyLimit::max(MAX_OPERATIONS_JSON_BODY_BYTES));

    // Fee sponsorship: public (the wallet signature authorizes), bounded by the sponsor policy.
    let sponsorship = Router::new()
        .route("/api/v2/sponsor/submit", post(sponsor::submit))
        .layer(DefaultBodyLimit::max(MAX_SPONSOR_JSON_BODY_BYTES));

    // Organization accounts and managed signing (S2): small JSON bodies.
    let accounts = Router::new()
        .route("/api/v2/organizations", post(orgs::create_organization))
        .route("/api/v2/organizations/{orgId}/users", post(orgs::add_user))
        .route(
            "/api/v2/organizations/{orgId}/users/{userId}/disable",
            post(orgs::disable_user),
        )
        .route("/api/v2/auth/logout", post(orgs::logout))
        .route(
            "/api/v2/organizations/{orgId}/export",
            post(orgs::export_key),
        )
        .route("/api/v2/auth/otp/request", post(orgs::request_code))
        .route("/api/v2/auth/otp/verify", post(orgs::verify_code))
        .route("/api/v2/me", get(orgs::me))
        .route("/api/v2/org/sign", post(orgs::sign))
        .layer(DefaultBodyLimit::max(MAX_JSON_BODY_BYTES));

    // Facility boundaries (operator token): large polygons, bounded separately.
    let boundaries = Router::new()
        .route(
            "/api/v2/facilities/{facilityId}/boundary",
            post(geo::set_boundary),
        )
        .layer(DefaultBodyLimit::max(MAX_BOUNDARY_JSON_BODY_BYTES));

    protocol
        .merge(operations)
        .merge(sponsorship)
        .merge(boundaries)
        .merge(accounts)
        // The timeout wraps the limit, so waiting for a slot counts against the 30 s too.
        .layer(GlobalConcurrencyLimitLayer::new(MAX_CONCURRENT_REQUESTS))
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(cors)
        .with_state(state)
}

pub(crate) fn validate_transaction_signature(signature: &str) -> Result<(), ApiError> {
    // A base58 encoding of a 64-byte Solana signature is never longer than 88 chars.
    // Reject longer attacker-controlled strings before allocating in the decoder.
    if signature.len() > 88 || !signature.is_ascii() {
        return Err(ApiError::Validation(
            "txSignature must be a canonical 64-byte base58 Solana signature".into(),
        ));
    }
    let decoded = bs58::decode(signature).into_vec().map_err(|_| {
        ApiError::Validation("txSignature must be a base58 Solana signature".into())
    })?;
    if decoded.len() != 64 || bs58::encode(&decoded).into_string() != signature {
        return Err(ApiError::Validation(
            "txSignature must be a canonical 64-byte base58 Solana signature".into(),
        ));
    }
    Ok(())
}
