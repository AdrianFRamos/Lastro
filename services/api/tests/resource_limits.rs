//! API input/resource-boundary contracts that do not require live PostgreSQL or Solana.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use lastro_api::{
    config::AppConfig,
    error::ApiError,
    routes::{self, MAX_JSON_BODY_BYTES},
    solana::rpc::{CanonicalAnimalState, CanonicalRfidBinding, SolanaRpc},
    state::AppState,
};
use lastro_protocol::ids::{AnimalId, DeploymentId, RfidHash};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const STATION_PUBKEY_HEX: &str =
    "036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296";

struct DependencyMustNotRun;

#[async_trait]
impl SolanaRpc for DependencyMustNotRun {
    async fn protocol_station_pubkey(&self, _: DeploymentId) -> Result<[u8; 33], ApiError> {
        panic!("input rejection must happen before Solana RPC")
    }

    async fn animal_state(
        &self,
        _: DeploymentId,
        _: AnimalId,
    ) -> Result<Option<CanonicalAnimalState>, ApiError> {
        panic!("input rejection must happen before Solana RPC")
    }

    async fn rfid_binding(
        &self,
        _: DeploymentId,
        _: RfidHash,
    ) -> Result<Option<CanonicalRfidBinding>, ApiError> {
        panic!("input rejection must happen before Solana RPC")
    }

    async fn transaction_matches_confirmed(
        &self,
        _: &str,
        _: &lastro_api::model::TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        panic!("input rejection must happen before Solana RPC")
    }

    async fn transaction_matches(
        &self,
        _: &str,
        _: &lastro_api::model::TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        panic!("input rejection must happen before Solana RPC")
    }
}

fn test_app() -> axum::Router {
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(25))
        .connect_lazy("postgres://127.0.0.1:1/lastro")
        .expect("valid lazy PostgreSQL URL");
    let station_pubkey33: [u8; 33] = hex::decode(STATION_PUBKEY_HEX).unwrap().try_into().unwrap();
    routes::router(AppState {
        db: pool,
        config: Arc::new(AppConfig {
            bind_addr: "127.0.0.1:8080".parse().unwrap(),
            database_url: "postgres://127.0.0.1:1/lastro".into(),
            agent_token: TOKEN.into(),
            solana_rpc_url: "http://127.0.0.1:1".into(),
            deployment_id: [0xd0; 32],
            station_pubkey33,
            lastro_program_id: "Vote111111111111111111111111111111111111111".into(),
        }),
        rpc: Arc::new(DependencyMustNotRun),
    })
}

async fn send(
    uri: &str,
    body: impl Into<Body>,
    content_type: Option<&str>,
    authorization: Option<&str>,
) -> StatusCode {
    let mut builder = Request::builder().method("POST").uri(uri);
    if let Some(value) = content_type {
        builder = builder.header(header::CONTENT_TYPE, value);
    }
    if let Some(value) = authorization {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    test_app()
        .oneshot(builder.body(body.into()).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn oversized_body_is_rejected_before_json_or_dependencies() {
    // PURPOSE: Bound memory consumed by every JSON request at the HTTP edge.
    let status = send(
        "/api/animals",
        vec![b'a'; MAX_JSON_BODY_BYTES + 1],
        Some("application/json"),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

    // ASSERT: one byte above the derived 1 KiB request envelope returns 413.
    // FAILURE MEANS: attacker-controlled bodies can reach Axum's much larger generic buffering limit.
}

#[tokio::test]
async fn oversized_base64_evidence_is_rejected_by_the_body_limit() {
    // PURPOSE: Prevent an oversized encoded evidence field from consuming decode/crypto/database resources.
    let body = serde_json::json!({
        "captureId": "00000000-0000-0000-0000-000000000000",
        "eventBytesBase64": "A".repeat(MAX_JSON_BODY_BYTES),
        "observedRfidHex": "00".repeat(8),
        "stationPubkeyHex": STATION_PUBKEY_HEX,
        "stationSignatureHex": "00".repeat(64),
    })
    .to_string();

    assert!(body.len() > MAX_JSON_BODY_BYTES);
    let status = send(
        "/api/agent/evidence",
        body,
        Some("application/json"),
        Some(&format!("Bearer {TOKEN}")),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

    // ASSERT: oversized Base64 is rejected before authentication-dependent work or decoding.
    // FAILURE MEANS: evidence ingestion can be used as a large-body resource exhaustion path.
}

#[tokio::test]
async fn field_bounds_and_malformed_encodings_fail_before_dependencies() {
    // PURPOSE: Reject domain-invalid strings before database/RPC work.
    let long_visual = serde_json::json!({"visualRecoveryId": "x".repeat(65)}).to_string();
    assert_eq!(
        send("/api/animals", long_visual, Some("application/json"), None).await,
        StatusCode::BAD_REQUEST
    );

    let bad_hex = serde_json::json!({
        "action": "TRANSFER",
        "animalId": "zz",
        "nextCustodian": "11".repeat(32),
        "authorization": {
            "challengeId": "00000000-0000-0000-0000-000000000000",
            "signatureBase64": "AA".repeat(44),
        },
    })
    .to_string();
    assert_eq!(
        send("/api/captures", bad_hex, Some("application/json"), None).await,
        StatusCode::BAD_REQUEST
    );

    let bad_base64 = serde_json::json!({
        "captureId": "00000000-0000-0000-0000-000000000000",
        "eventBytesBase64": "***",
        "observedRfidHex": "00".repeat(8),
        "stationPubkeyHex": STATION_PUBKEY_HEX,
        "stationSignatureHex": "00".repeat(64),
    })
    .to_string();
    assert_eq!(
        send(
            "/api/agent/evidence",
            bad_base64,
            Some("application/json"),
            Some(&format!("Bearer {TOKEN}")),
        )
        .await,
        StatusCode::BAD_REQUEST
    );

    // ASSERT: visual IDs, fixed hex fields, and Base64 evidence enforce their protocol/domain sizes early.
    // FAILURE MEANS: malformed input can consume persistence or canonical-RPC capacity before rejection.
}

#[tokio::test]
async fn malformed_json_wrong_content_type_and_unexpected_fields_are_rejected() {
    // PURPOSE: Keep the HTTP/DTO grammar strict at the public edge.
    assert_eq!(
        send(
            "/api/animals",
            r#"{"visualRecoveryId":"ok""#,
            Some("application/json"),
            None,
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(
            "/api/animals",
            r#"{"visualRecoveryId":"ok"}"#,
            Some("text/plain"),
            None,
        )
        .await,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    assert_eq!(
        send(
            "/api/animals",
            r#"{"visualRecoveryId":"ok","privateKey":"forbidden"}"#,
            Some("application/json"),
            None,
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // ASSERT: syntax, media type, and undeclared fields each fail with Axum's deterministic rejection status.
    // FAILURE MEANS: ambiguous or secret-bearing request shapes can cross the DTO boundary.
}

#[tokio::test]
async fn repeated_oversized_requests_remain_bounded() {
    // PURPOSE: Exercise the cheap rejection path repeatedly rather than only once.
    for _ in 0..16 {
        let status = send(
            "/api/animals",
            vec![b'x'; MAX_JSON_BODY_BYTES + 1],
            Some("application/json"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    }

    // ASSERT: repeated abusive bodies stay on the 413 path and never require external dependencies.
    // FAILURE MEANS: repeated oversized input can escape the deterministic resource boundary.
}
