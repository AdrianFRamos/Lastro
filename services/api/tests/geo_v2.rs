//! SPEC v2.1 S5: a facility boundary is stored once and its commitment is exactly what the
//! authority's `register_facility` transaction carries.

mod common;

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use lastro_api::{config::AppConfig, routes, state::AppState};
use lastro_protocol::v2::FacilityBoundary;
use serde_json::{Value, json};
use tower::ServiceExt;

use common::{AGENT_TOKEN, DEPLOYMENT_ID, PROGRAM_ID, STATION_PUBKEY, TestDb, TestRpc};

const OPERATOR: &str = "operator-token-0123456789abcdef0123";
const FARM: [u8; 32] = [0xfa; 32];
const OWNER: [u8; 32] = [0x0a; 32];

fn app(db: &TestDb) -> Router {
    routes::router(AppState {
        db: db.pool.clone(),
        config: Arc::new(AppConfig {
            bind_addr: "127.0.0.1:8080".parse().unwrap(),
            database_url: "postgres://test-only".into(),
            agent_token: AGENT_TOKEN.into(),
            operator_token: Some(OPERATOR.into()),
            solana_rpc_url: "http://127.0.0.1:8899".into(),
            deployment_id: DEPLOYMENT_ID,
            station_pubkey33: STATION_PUBKEY,
            lastro_program_id: PROGRAM_ID.into(),
            cors_allowed_origins: Vec::new(),
            priority_fee_micro_lamports: None,
            sponsor_keypair_path: None,
            sponsor_limits: lastro_api::sponsor::SponsorLimits::default(),
            org_key_encryption_key: None,
            otp_log_delivery: false,
        }),
        rpc: Arc::new(TestRpc::configured()),
        sponsor: None,
        vault: None,
    })
}

async fn post(app: &Router, uri: &str, body: Value, operator: bool) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if operator {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {OPERATOR}"));
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn square(west: f64) -> Value {
    json!([[
        [west, -15.5],
        [west + 0.01, -15.5],
        [west + 0.01, -15.49],
        [west, -15.49],
        [west, -15.5]
    ]])
}

#[tokio::test]
async fn a_boundary_is_immutable_and_its_commitment_is_what_the_authority_signs() {
    // PURPOSE: S5 — the polygon the operator registers is exactly the one the chain commits to.
    // ARRANGE: a party and a facility in the projection.
    let db = TestDb::new().await;
    sqlx::query("INSERT INTO v2_parties(party_id,deployment_id,legal_name,wallet,role,status) VALUES($1,$2,'Fazenda Boa Vista',$3,1,'ACTIVE')")
        .bind(OWNER.to_vec())
        .bind(DEPLOYMENT_ID.to_vec())
        .bind([0x0bu8; 32].to_vec())
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO v2_facilities(facility_id,deployment_id,owner_party_id,facility_type,display_name,credential_hash,valid_from,valid_until,status) VALUES($1,$2,$3,1,'Boa Vista',$4,1,4000000000,'ACTIVE')")
        .bind(FARM.to_vec())
        .bind(DEPLOYMENT_ID.to_vec())
        .bind(OWNER.to_vec())
        .bind([0xc3u8; 32].to_vec())
        .execute(&db.pool)
        .await
        .unwrap();
    let app = app(&db);
    let uri = format!("/api/v2/facilities/{}/boundary", hex::encode(FARM));
    let body = json!({"carCode": "MT-5103403-0A1B2C3D4E5F", "rings": square(-56.1)});

    // ACTION: register, repeat, then try to change it.
    let (status, _) = post(&app, &uri, body.clone(), false).await;
    assert!(status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN);
    let (status, created) = post(&app, &uri, body.clone(), true).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let (status, _) = post(&app, &uri, body, true).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = post(
        &app,
        &uri,
        json!({"carCode": "MT-5103403-0A1B2C3D4E5F", "rings": square(-56.2)}),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = post(
        &app,
        &uri,
        json!({"carCode": "MT-1", "rings": [[[-56.1, -15.5], [-56.0, -15.5], [-56.0, -15.4]]]}),
        true,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "an open ring is rejected before storage"
    );

    // ASSERT: the commitment matches the protocol encoding and ends register_facility's data.
    let expected = FacilityBoundary {
        car_code: "MT-5103403-0A1B2C3D4E5F".into(),
        rings: vec![vec![
            (-561_000_000, -155_000_000),
            (-560_900_000, -155_000_000),
            (-560_900_000, -154_900_000),
            (-561_000_000, -154_900_000),
            (-561_000_000, -155_000_000),
        ]],
    }
    .commitment()
    .unwrap();
    assert_eq!(created["geoCommitment"], hex::encode(expected));
    let (status, tx) = post(
        &app,
        &format!("/api/v2/facilities/{}/transaction-data", hex::encode(FARM)),
        json!(null),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{tx}");
    let data = BASE64
        .decode(tx["instructions"][0]["dataBase64"].as_str().unwrap())
        .unwrap();
    assert_eq!(&data[data.len() - 32..], &expected);
    db.cleanup().await;
    // FAILURE MEANS: a facility could be registered on-chain with another polygon than stored.
}
