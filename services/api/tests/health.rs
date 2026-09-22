//! API health/readiness integration contracts.

mod common;

use std::sync::Arc;

use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}, Router};
use serde_json::Value;
use tower::ServiceExt;

use common::{app_state, TestDb, TestRpc, STATION_PUBKEY};

async fn get_health(app: Router) -> (StatusCode, Value) {
    let response = app
        .oneshot(Request::builder().uri("/api/health").body(Body::empty()).unwrap())
        .await
        .expect("health response");
    let status = response.status();
    let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn health_ok_requires_database_and_rpc() {
    // PURPOSE: Report healthy only when durable PostgreSQL and canonical Station configuration are both readable.
    // ASSERT: A live database plus matching ProtocolConfig key returns status=ok with both checks=ok.
    // FAILURE MEANS: Traffic could be sent to an API instance unable to persist or verify canonical state.
    let db = TestDb::new().await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let app = lastro_api::routes::router(app_state(db.pool.clone(), rpc));
    let (status, body) = get_health(app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["database"], "ok");
    assert_eq!(body["rpc"], "ok");
    db.cleanup().await;
}

#[tokio::test]
async fn health_degraded_when_rpc_unavailable() {
    // PURPOSE: Expose loss of the canonical Solana dependency instead of falling back to PostgreSQL as truth.
    // ASSERT: With PostgreSQL healthy and RPC unable to return ProtocolConfig, health is degraded with rpc=error.
    // FAILURE MEANS: The service could advertise readiness while unable to verify canonical state.
    let db = TestDb::new().await;
    let rpc = Arc::new(TestRpc::default());
    let app = lastro_api::routes::router(app_state(db.pool.clone(), rpc));
    let (status, body) = get_health(app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "degraded");
    assert_eq!(body["database"], "ok");
    assert_eq!(body["rpc"], "error");
    db.cleanup().await;
}

#[tokio::test]
async fn health_degraded_when_database_unavailable() {
    // PURPOSE: Expose loss of durable persistence even when canonical Solana configuration remains readable.
    // ASSERT: A closed database pool produces status=degraded with database=error and rpc=ok.
    // FAILURE MEANS: The API could accept work while unable to persist captures, evidence, or projections.
    let db = TestDb::new().await;
    let pool = db.pool.clone();
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let app = lastro_api::routes::router(app_state(pool, rpc));
    db.cleanup().await;

    let (status, body) = get_health(app).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "degraded");
    assert_eq!(body["database"], "error");
    assert_eq!(body["rpc"], "ok");
}
