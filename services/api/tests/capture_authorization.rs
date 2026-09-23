//! Wallet authorization contracts for capture creation.

mod common;

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ed25519_dalek::{Signer, SigningKey};
use lastro_api::{
    repository::{
        animals,
        captures::{self, NewCapture},
    },
    routes,
    solana::rpc::CanonicalAnimalState,
};
use lastro_protocol::crypto::derive_station_id;
use serde_json::{Value, json};
use tower::ServiceExt;

use common::{DEPLOYMENT_ID, STATION_PUBKEY, TestDb, TestRpc, app_state};

const ANIMAL_ID: [u8; 32] = [0x11; 32];
const RFID_A: [u8; 32] = [0x22; 32];
const H1: [u8; 32] = [0x33; 32];
const CUSTODIAN_B: [u8; 32] = [0xb2; 32];

fn app(db: &TestDb, rpc: Arc<TestRpc>) -> Router {
    routes::router(app_state(db.pool.clone(), rpc))
}

async fn request_json(app: Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.oneshot(request).await.expect("route response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read response body");
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("JSON response")
    };
    (status, value)
}

async fn register(db: &TestDb) {
    animals::insert_registration(&db.pool, ANIMAL_ID, "VISUAL-AUTH")
        .await
        .expect("register test animal");
}

fn signer() -> SigningKey {
    SigningKey::from_bytes(&[0x41; 32])
}

fn canonical_state(current_custodian: [u8; 32]) -> CanonicalAnimalState {
    CanonicalAnimalState {
        animal_id: ANIMAL_ID,
        current_rfid_hash: RFID_A,
        current_custodian,
        identity_revision: 1,
        event_sequence: 1,
        last_event_hash: H1,
    }
}

async fn challenge(
    app: Router,
    action: &str,
    next_custodian: Option<[u8; 32]>,
) -> (StatusCode, Value) {
    let mut body = json!({
        "action": action,
        "animalId": hex::encode(ANIMAL_ID),
    });
    if let Some(next) = next_custodian {
        body.as_object_mut()
            .unwrap()
            .insert("nextCustodian".into(), Value::String(hex::encode(next)));
    }
    request_json(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/captures/authorization-challenge")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
}

async fn authorized_capture(
    app: Router,
    action: &str,
    next_custodian: Option<[u8; 32]>,
    challenge_id: &str,
    signature: &[u8; 64],
) -> (StatusCode, Value) {
    let mut body = json!({
        "action": action,
        "animalId": hex::encode(ANIMAL_ID),
        "authorization": {
            "challengeId": challenge_id,
            "signatureBase64": BASE64.encode(signature),
        },
    });
    if let Some(next) = next_custodian {
        body.as_object_mut()
            .unwrap()
            .insert("nextCustodian".into(), Value::String(hex::encode(next)));
    }
    request_json(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/captures")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await
}

#[tokio::test]
async fn public_capture_challenges_are_bounded_per_custodian() {
    // PURPOSE: An anonymous caller cannot grow challenge rows or trigger unlimited Station work
    // by repeatedly targeting one publicly known animal and custodian.
    // ASSERT: The ninth request returns 429 and persistence remains at eight rows.
    // FAILURE MEANS: a public caller can exhaust the authorization table or canonical RPC.
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_state(signer().verifying_key().to_bytes()));

    for _ in 0..8 {
        let (status, _) = challenge(app(&db, rpc.clone()), "TRANSFER", Some(CUSTODIAN_B)).await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let prior_rpc_calls = rpc.call_count();
    let (status, body) = challenge(app(&db, rpc.clone()), "TRANSFER", Some(CUSTODIAN_B)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(rpc.call_count(), prior_rpc_calls);
    assert_eq!(body["code"], "RATE_LIMITED");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM capture_authorization_challenges")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 8);
    db.cleanup().await;
}

#[tokio::test]
async fn current_custodian_signature_authorizes_transfer_exactly_once() {
    // PURPOSE: A public client must prove current-custodian control before a TRANSFER can reserve the Station.
    // ARRANGE: Canonical sequence 1 belongs to a deterministic Ed25519 wallet and the API issues a short-lived challenge.
    // ACTION: Sign the exact server message, create the transfer, then replay the same challenge.
    // ASSERT: The first request creates one capture; replay conflicts and cannot create a second row.
    // FAILURE MEANS: anyone who knows AnimalID could reserve physical work or replay a prior wallet authorization.
    let db = TestDb::new().await;
    register(&db).await;
    let signing_key = signer();
    let current_custodian = signing_key.verifying_key().to_bytes();
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_state(current_custodian));

    let (challenge_status, issued) =
        challenge(app(&db, rpc.clone()), "TRANSFER", Some(CUSTODIAN_B)).await;
    assert_eq!(challenge_status, StatusCode::CREATED);
    assert_eq!(
        issued["requiredSigner"].as_str(),
        Some(bs58::encode(current_custodian).into_string().as_str()),
    );
    assert_eq!(
        issued["deploymentId"].as_str(),
        Some(hex::encode(DEPLOYMENT_ID).as_str())
    );

    let challenge_id = issued["challengeId"].as_str().unwrap();
    let message = BASE64
        .decode(issued["messageBase64"].as_str().unwrap())
        .unwrap();
    let signature = signing_key.sign(&message).to_bytes();

    let (create_status, _) = authorized_capture(
        app(&db, rpc.clone()),
        "TRANSFER",
        Some(CUSTODIAN_B),
        challenge_id,
        &signature,
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (replay_status, _) = authorized_capture(
        app(&db, rpc),
        "TRANSFER",
        Some(CUSTODIAN_B),
        challenge_id,
        &signature,
    )
    .await;
    assert_eq!(replay_status, StatusCode::CONFLICT);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM captures")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn invalid_wallet_signature_never_reserves_a_capture() {
    // PURPOSE: Reject a forged capture authorization before any Station queue state is created.
    // ARRANGE: Issue a valid TRANSFER challenge for the current custodian, then sign it with a different wallet.
    // ACTION: Submit that forged authorization.
    // ASSERT: HTTP 401 is returned and captures remains empty.
    // FAILURE MEANS: the challenge endpoint adds ceremony without actually protecting physical capture creation.
    let db = TestDb::new().await;
    register(&db).await;
    let current = signer();
    let attacker = SigningKey::from_bytes(&[0x42; 32]);
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_state(current.verifying_key().to_bytes()));

    let (_, issued) = challenge(app(&db, rpc.clone()), "TRANSFER", Some(CUSTODIAN_B)).await;
    let challenge_id = issued["challengeId"].as_str().unwrap();
    let message = BASE64
        .decode(issued["messageBase64"].as_str().unwrap())
        .unwrap();
    let forged = attacker.sign(&message).to_bytes();

    let (status, _) = authorized_capture(
        app(&db, rpc),
        "TRANSFER",
        Some(CUSTODIAN_B),
        challenge_id,
        &forged,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM captures")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    db.cleanup().await;
}

#[tokio::test]
async fn challenge_cannot_authorize_a_different_capture_intent() {
    // PURPOSE: Bind wallet consent to one action, animal, destination and deployment.
    // ARRANGE: The current custodian signs an A->B TRANSFER challenge.
    // ACTION: Reuse the same challenge while requesting REIDENTIFY instead.
    // ASSERT: The API returns conflict and creates no capture.
    // FAILURE MEANS: a valid wallet signature could be replayed for an operation the user did not approve.
    let db = TestDb::new().await;
    register(&db).await;
    let signing_key = signer();
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_state(signing_key.verifying_key().to_bytes()));

    let (_, issued) = challenge(app(&db, rpc.clone()), "TRANSFER", Some(CUSTODIAN_B)).await;
    let challenge_id = issued["challengeId"].as_str().unwrap();
    let message = BASE64
        .decode(issued["messageBase64"].as_str().unwrap())
        .unwrap();
    let signature = signing_key.sign(&message).to_bytes();

    let (status, _) =
        authorized_capture(app(&db, rpc), "REIDENTIFY", None, challenge_id, &signature).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM captures")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    db.cleanup().await;
}

#[tokio::test]
async fn expired_challenge_is_rejected_before_capture_reservation() {
    // PURPOSE: Limit the replay window of off-chain wallet authorization.
    // ARRANGE: Issue a valid challenge, sign it, and move its expiry into the past in the isolated test schema.
    // ACTION: Submit the otherwise valid capture authorization.
    // ASSERT: The expired challenge conflicts and no capture row is created.
    // FAILURE MEANS: leaked old signatures remain usable indefinitely.
    let db = TestDb::new().await;
    register(&db).await;
    let signing_key = signer();
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_state(signing_key.verifying_key().to_bytes()));

    let (_, issued) = challenge(app(&db, rpc.clone()), "TRANSFER", Some(CUSTODIAN_B)).await;
    let challenge_id = issued["challengeId"].as_str().unwrap();
    let message = BASE64
        .decode(issued["messageBase64"].as_str().unwrap())
        .unwrap();
    let signature = signing_key.sign(&message).to_bytes();

    sqlx::query("UPDATE capture_authorization_challenges SET created_at=now()-interval '2 minutes', expires_at=now()-interval '1 second' WHERE challenge_id=$1")
        .bind(challenge_id.parse::<uuid::Uuid>().unwrap())
        .execute(&db.pool)
        .await
        .unwrap();

    let (status, _) = authorized_capture(
        app(&db, rpc),
        "TRANSFER",
        Some(CUSTODIAN_B),
        challenge_id,
        &signature,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM captures")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    db.cleanup().await;
}

#[tokio::test]
async fn failed_capture_reservation_does_not_consume_valid_authorization() {
    // PURPOSE: Couple one-time authorization consumption atomically with Station reservation.
    // ARRANGE: A valid signed TRANSFER challenge exists while another active capture occupies the Station.
    // ACTION: Attempt the authorized capture, cancel the blocker, then retry the same still-valid authorization.
    // ASSERT: The first attempt conflicts without consuming the challenge; the retry creates the capture.
    // FAILURE MEANS: a transient Station reservation conflict can permanently destroy valid wallet consent without creating work.
    let db = TestDb::new().await;
    register(&db).await;
    let signing_key = signer();
    let current_custodian = signing_key.verifying_key().to_bytes();
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_state(current_custodian));

    let (_, issued) = challenge(app(&db, rpc.clone()), "TRANSFER", Some(CUSTODIAN_B)).await;
    let challenge_id = issued["challengeId"].as_str().unwrap();
    let message = BASE64
        .decode(issued["messageBase64"].as_str().unwrap())
        .unwrap();
    let signature = signing_key.sign(&message).to_bytes();

    let blocker = captures::insert_pending(
        &db.pool,
        &NewCapture {
            station_id: derive_station_id(&STATION_PUBKEY).unwrap(),
            action: 2,
            animal_id: ANIMAL_ID,
            event_sequence: 2,
            identity_revision: 1,
            expected_old_rfid_hash: RFID_A,
            from_custodian: current_custodian,
            to_custodian: CUSTODIAN_B,
            previous_event_hash: H1,
        },
    )
    .await
    .unwrap();

    let (blocked_status, _) = authorized_capture(
        app(&db, rpc.clone()),
        "TRANSFER",
        Some(CUSTODIAN_B),
        challenge_id,
        &signature,
    )
    .await;
    assert_eq!(blocked_status, StatusCode::CONFLICT);

    let remains_active: bool = sqlx::query_scalar(
        "SELECT used_at IS NULL FROM capture_authorization_challenges WHERE challenge_id=$1",
    )
    .bind(challenge_id.parse::<uuid::Uuid>().unwrap())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(remains_active);

    sqlx::query("UPDATE captures SET status='CANCELLED' WHERE capture_id=$1")
        .bind(blocker.capture_id)
        .execute(&db.pool)
        .await
        .unwrap();

    let (retry_status, _) = authorized_capture(
        app(&db, rpc),
        "TRANSFER",
        Some(CUSTODIAN_B),
        challenge_id,
        &signature,
    )
    .await;
    assert_eq!(retry_status, StatusCode::CREATED);
    db.cleanup().await;
}
