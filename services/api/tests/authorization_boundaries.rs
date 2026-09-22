//! API authorization-boundary contracts.

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use lastro_api::{
    config::AppConfig,
    error::ApiError,
    model::{ConfirmEventRequest, CreateCaptureRequest},
    routes,
    solana::rpc::{CanonicalAnimalState, CanonicalRfidBinding, SolanaRpc},
    state::AppState,
};
use lastro_protocol::ids::{AnimalId, DeploymentId, RfidHash};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const STATION_PUBKEY: [u8; 33] = [
    0x03, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63,
    0xa4, 0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39,
    0x45, 0xd8, 0x98, 0xc2, 0x96,
];

struct UnusedRpc;

#[async_trait]
impl SolanaRpc for UnusedRpc {
    async fn protocol_station_pubkey(&self, _: DeploymentId) -> Result<[u8; 33], ApiError> {
        panic!("authorization test must not reach Solana RPC")
    }

    async fn animal_state(
        &self,
        _: DeploymentId,
        _: AnimalId,
    ) -> Result<Option<CanonicalAnimalState>, ApiError> {
        panic!("authorization test must not reach Solana RPC")
    }

    async fn rfid_binding(
        &self,
        _: DeploymentId,
        _: RfidHash,
    ) -> Result<Option<CanonicalRfidBinding>, ApiError> {
        panic!("authorization test must not reach Solana RPC")
    }

    async fn transaction_matches_confirmed(
        &self,
        _: &str,
        _: &lastro_api::model::TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        Ok(false)
    }

    async fn transaction_matches(
        &self,
        _: &str,
        _: &lastro_api::model::TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        panic!("authorization test must not reach Solana RPC")
    }
}

fn test_app() -> axum::Router {
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(25))
        .connect_lazy("postgres://127.0.0.1:1/lastro")
        .expect("valid lazy PostgreSQL URL");
    let config = AppConfig {
        bind_addr: "127.0.0.1:8080".parse().unwrap(),
        database_url: "postgres://127.0.0.1:1/lastro".into(),
        agent_token: TOKEN.into(),
        solana_rpc_url: "http://127.0.0.1:1".into(),
        deployment_id: [0xd0; 32],
        station_pubkey33: STATION_PUBKEY,
        lastro_program_id: "Vote111111111111111111111111111111111111111".into(),
    };
    routes::router(AppState {
        db: pool,
        config: Arc::new(config),
        rpc: Arc::new(UnusedRpc),
    })
}

async fn get_command(authorization: Option<&str>) -> StatusCode {
    let mut builder = Request::builder().uri("/api/agent/commands");
    if let Some(value) = authorization {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    test_app()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

async fn post_evidence(authorization: Option<&str>) -> StatusCode {
    let capture_id = uuid::Uuid::nil();
    let body = serde_json::json!({
        "captureId": capture_id,
        "eventBytesBase64": "AA==",
        "observedRfidHex": "00",
        "stationPubkeyHex": "00",
        "stationSignatureHex": "00"
    })
    .to_string();
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/agent/evidence")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(value) = authorization {
        builder = builder.header(header::AUTHORIZATION, value);
    }
    test_app()
        .oneshot(builder.body(Body::from(body)).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn agent_endpoints_require_the_exact_bearer_token() {
    // PURPOSE: isolate Station/Agent transport endpoints from the public API surface.
    for status in [
        get_command(None).await,
        get_command(Some("Bearer wrong-token")).await,
        get_command(Some("Bearer 1123456789abcdef0123456789abcdef")).await,
        get_command(Some("Bearer 0123456789abcdef0123456789abcdee")).await,
        post_evidence(None).await,
        post_evidence(Some("Bearer wrong-token")).await,
        post_evidence(Some("Bearer 1123456789abcdef0123456789abcdef")).await,
        post_evidence(Some("Bearer 0123456789abcdef0123456789abcdee")).await,
    ] {
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    let authorized = get_command(Some(&format!("Bearer {TOKEN}"))).await;
    assert_ne!(authorized, StatusCode::UNAUTHORIZED);

    // ASSERT: absent/wrong credentials fail before DB/RPC work; the exact token crosses the auth gate.
    // FAILURE MEANS: an untrusted web client could consume commands or inject physical evidence.
}

#[tokio::test]
async fn public_evidence_package_endpoint_does_not_require_agent_authentication() {
    // PURPOSE: independent verification must remain public rather than depending on an operational secret.
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri(format!("/api/animals/{}/evidence-package", "00".repeat(32)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);

    // ASSERT: the request crosses routing/auth without an Agent bearer token (the lazy DB may then fail).
    // FAILURE MEANS: the independent verifier would require a private operational credential.
}

#[test]
fn wallet_private_key_fields_are_rejected_by_request_schemas() {
    // PURPOSE: keep custody authority in the wallet; the API must never accept wallet secrets.
    let capture = serde_json::json!({
        "action": "TRANSFER",
        "animalId": "11".repeat(32),
        "nextCustodian": "22".repeat(32),
        "privateKey": "do-not-accept"
    });
    assert!(serde_json::from_value::<CreateCaptureRequest>(capture).is_err());

    for field in ["privateKey", "secretKey", "seed", "mnemonic"] {
        let mut value = serde_json::json!({"txSignature": "signature"});
        value
            .as_object_mut()
            .unwrap()
            .insert(field.into(), serde_json::Value::String("do-not-accept".into()));
        assert!(serde_json::from_value::<ConfirmEventRequest>(value).is_err());
    }

    // ASSERT: unknown wallet-secret fields fail strict DTO deserialization.
    // FAILURE MEANS: the backend could accidentally become a custodian of private wallet material.
}
