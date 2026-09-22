//! PostgreSQL and HTTP concurrency contracts for capture/evidence admission.

mod common;

use std::{fs, path::PathBuf, sync::Arc};

use axum::{body::Body, http::{header, Request, StatusCode}, Router};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use lastro_api::{
    repository::{animals, captures, events},
    routes,
};
use lastro_protocol::{rfid::hash_canonical_rfid, StationEvent};
use serde_json::json;
use tokio::sync::Barrier;
use tower::ServiceExt;

use common::{
    app_state, sign_station_event_with_test_scalar, TestDb, TestRpc, AGENT_TOKEN, STATION_PUBKEY,
};

const ANIMAL_ID: [u8; 32] = [0x11; 32];
const RFID_A: [u8; 8] = [0x80, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x01];
const RFID_B: [u8; 8] = [0x80, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x02];
const ORIGIN_SIGNATURE_HEX: &str = "09579de39fbd8108dea2fc89262240c45b2043d85e6e16854d9bed737e6426e109be548b3eb9d3ba3252aeba61af230a18115aeaf0b74734df88c90955f9184f";

fn fixture(name: &str) -> [u8; 276] {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name))
        .expect("read fixture")
        .try_into()
        .expect("StationEvent fixture is exactly 276 bytes")
}

fn capture_from_event(event: &StationEvent) -> captures::NewCapture {
    captures::NewCapture {
        station_id: event.station_id,
        action: event.action as u8,
        animal_id: event.animal_id,
        event_sequence: event.event_sequence,
        identity_revision: event.identity_revision,
        expected_old_rfid_hash: event.old_rfid_hash,
        from_custodian: event.from_custodian,
        to_custodian: event.to_custodian,
        previous_event_hash: event.previous_event_hash,
    }
}

async fn register(db: &TestDb) {
    animals::insert_registration(&db.pool, ANIMAL_ID, "VISUAL-CONCURRENCY")
        .await
        .expect("register test animal");
}

async fn seed_dispatched_origin(db: &TestDb) -> (StationEvent, uuid::Uuid) {
    register(db).await;
    let event = StationEvent::decode(&fixture("origin.bin")).unwrap();
    let pending = captures::insert_pending(&db.pool, &capture_from_event(&event))
        .await
        .unwrap();
    let claimed = captures::claim_next_for_station(&db.pool, event.station_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.capture_id, pending.capture_id);
    (event, pending.capture_id)
}

fn evidence_body(
    capture_id: uuid::Uuid,
    event_bytes: &[u8; 276],
    observed_rfid: &[u8; 8],
    public_key: &[u8; 33],
    signature: &[u8; 64],
) -> String {
    json!({
        "captureId": capture_id,
        "eventBytesBase64": BASE64.encode(event_bytes),
        "observedRfidHex": hex::encode(observed_rfid),
        "stationPubkeyHex": hex::encode(public_key),
        "stationSignatureHex": hex::encode(signature),
    })
    .to_string()
}

async fn post_evidence(app: Router, body: String, barrier: Arc<Barrier>) -> StatusCode {
    barrier.wait().await;
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/agent/evidence")
            .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap(),
    )
    .await
    .unwrap()
    .status()
}

#[tokio::test]
async fn concurrent_create_capture_same_station_has_single_winner() {
    // PURPOSE: Prove the one-active-capture rule under real concurrent PostgreSQL transactions, not only sequential calls.
    // ASSERT: Two simultaneous inserts for one Station produce exactly one success, one conflict, and one active row.
    // FAILURE MEANS: A race could send competing immutable contexts to the same physical Station.
    let db = TestDb::new().await;
    register(&db).await;
    let event = StationEvent::decode(&fixture("origin.bin")).unwrap();
    let value = capture_from_event(&event);
    let barrier = Arc::new(Barrier::new(3));

    let first_pool = db.pool.clone();
    let first_value = value.clone();
    let first_barrier = barrier.clone();
    let first = tokio::spawn(async move {
        first_barrier.wait().await;
        captures::insert_pending(&first_pool, &first_value).await
    });
    let second_pool = db.pool.clone();
    let second_value = value.clone();
    let second_barrier = barrier.clone();
    let second = tokio::spawn(async move {
        second_barrier.wait().await;
        captures::insert_pending(&second_pool, &second_value).await
    });
    barrier.wait().await;

    let first = first.await.unwrap();
    let second = second.await.unwrap();
    assert_eq!((first.is_ok() as usize) + (second.is_ok() as usize), 1);
    assert_eq!((first.is_err() as usize) + (second.is_err() as usize), 1);
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM captures WHERE station_id=$1 AND status IN ('PENDING','DISPATCHED')",
    )
    .bind(event.station_id.to_vec())
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(active, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn concurrent_identical_evidence_is_idempotent() {
    // PURPOSE: Make simultaneous retries of the same immutable evidence converge after an ambiguous network outcome.
    // ASSERT: Concurrent identical requests yield one 201 and one 200, with exactly one byte-identical event row.
    // FAILURE MEANS: Network retry races could duplicate history or incorrectly reject valid recovery.
    let db = TestDb::new().await;
    let (_event, capture_id) = seed_dispatched_origin(&db).await;
    let event_bytes = fixture("origin.bin");
    let signature: [u8; 64] = hex::decode(ORIGIN_SIGNATURE_HEX).unwrap().try_into().unwrap();
    let body = evidence_body(capture_id, &event_bytes, &RFID_A, &STATION_PUBKEY, &signature);
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let app = routes::router(app_state(db.pool.clone(), rpc));
    let barrier = Arc::new(Barrier::new(3));

    let first = tokio::spawn(post_evidence(app.clone(), body.clone(), barrier.clone()));
    let second = tokio::spawn(post_evidence(app, body, barrier.clone()));
    barrier.wait().await;
    let mut statuses = [first.await.unwrap().as_u16(), second.await.unwrap().as_u16()];
    statuses.sort_unstable();
    assert_eq!(statuses, [StatusCode::OK.as_u16(), StatusCode::CREATED.as_u16()]);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM events WHERE capture_id=$1")
        .bind(capture_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let stored = events::find_by_capture_id(&db.pool, capture_id).await.unwrap().unwrap();
    assert_eq!(stored.event_bytes, event_bytes);
    db.cleanup().await;
}

#[tokio::test]
async fn concurrent_divergent_same_sequence_has_single_immutable_result() {
    // PURPOSE: Prevent two independently valid physical observations from branching one capture/sequence under concurrency.
    // ASSERT: Exactly one request is created, the other conflicts, and the sole stored row equals one complete submitted evidence set.
    // FAILURE MEANS: PostgreSQL could contain a fork or a bytewise mixture of competing evidence.
    let db = TestDb::new().await;
    let (origin, capture_id) = seed_dispatched_origin(&db).await;
    let original_bytes = fixture("origin.bin");
    let original_signature: [u8; 64] = hex::decode(ORIGIN_SIGNATURE_HEX).unwrap().try_into().unwrap();

    let mut alternate = origin;
    alternate.new_rfid_hash = hash_canonical_rfid(&RFID_B);
    let (alternate_bytes, alternate_key, alternate_signature) =
        sign_station_event_with_test_scalar(&alternate, 1);
    assert_eq!(alternate_key, STATION_PUBKEY);

    let original_body = evidence_body(
        capture_id,
        &original_bytes,
        &RFID_A,
        &STATION_PUBKEY,
        &original_signature,
    );
    let alternate_body = evidence_body(
        capture_id,
        &alternate_bytes,
        &RFID_B,
        &alternate_key,
        &alternate_signature,
    );
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let app = routes::router(app_state(db.pool.clone(), rpc));
    let barrier = Arc::new(Barrier::new(3));

    let first = tokio::spawn(post_evidence(app.clone(), original_body, barrier.clone()));
    let second = tokio::spawn(post_evidence(app, alternate_body, barrier.clone()));
    barrier.wait().await;
    let mut statuses = [first.await.unwrap().as_u16(), second.await.unwrap().as_u16()];
    statuses.sort_unstable();
    assert_eq!(statuses, [StatusCode::CREATED.as_u16(), StatusCode::CONFLICT.as_u16()]);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM events WHERE animal_id=$1 AND event_sequence=1")
        .bind(ANIMAL_ID.to_vec())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let stored = events::find_by_capture_id(&db.pool, capture_id).await.unwrap().unwrap();
    let original_won = stored.event_bytes == original_bytes
        && stored.observed_rfid == RFID_A
        && stored.station_signature == original_signature;
    let alternate_won = stored.event_bytes == alternate_bytes
        && stored.observed_rfid == RFID_B
        && stored.station_signature == alternate_signature;
    assert!(original_won ^ alternate_won);
    db.cleanup().await;
}
