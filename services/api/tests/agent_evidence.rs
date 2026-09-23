//! Agent evidence admission integration contracts.

mod common;

use std::{fs, path::PathBuf, sync::Arc};

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use lastro_api::{
    repository::{animals, captures, events},
    routes,
};
use lastro_protocol::{StationEvent, crypto::derive_station_id, rfid::hash_canonical_rfid};
use serde_json::json;
use tower::ServiceExt;

use common::{
    AGENT_TOKEN, STATION_PUBKEY, TestDb, TestRpc, app_state, sign_station_event_with_test_scalar,
};

const ANIMAL_ID: [u8; 32] = [0x11; 32];
const OTHER_ANIMAL_ID: [u8; 32] = [0x22; 32];
const OBSERVED_RFID_A: [u8; 8] = [0x80, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x01];
const OBSERVED_RFID_B: [u8; 8] = [0x80, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x02];
const ORIGIN_SIGNATURE_HEX: &str = "09579de39fbd8108dea2fc89262240c45b2043d85e6e16854d9bed737e6426e109be548b3eb9d3ba3252aeba61af230a18115aeaf0b74734df88c90955f9184f";
const TRANSFER_SIGNATURE_HEX: &str = "6cd31b25d410d4ad14d1d30b5a9f6203d7f165e5a0f6f81c847dd85cff72f17a46a2d1b94a0f70d93ff6fff812a4b928a5dd8e71b263994398d975bc81ca5f5b";

fn fixture(name: &str) -> [u8; 276] {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name))
        .expect("read fixture")
        .try_into()
        .expect("StationEvent fixture is exactly 276 bytes")
}

fn fixture_event(name: &str) -> StationEvent {
    StationEvent::decode(&fixture(name)).expect("decode StationEvent fixture")
}

fn app(db: &TestDb, rpc: Arc<TestRpc>) -> Router {
    routes::router(app_state(db.pool.clone(), rpc))
}

async fn register(db: &TestDb, animal_id: [u8; 32], recovery_id: &str) {
    animals::insert_registration(&db.pool, animal_id, recovery_id)
        .await
        .expect("register test animal");
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

async fn create_dispatched_capture(
    db: &TestDb,
    value: &captures::NewCapture,
) -> captures::CaptureRecord {
    let pending = captures::insert_pending(&db.pool, value)
        .await
        .expect("insert pending capture");
    let dispatched = captures::claim_next_for_station(&db.pool, value.station_id)
        .await
        .expect("claim capture")
        .expect("capture is available");
    assert_eq!(dispatched.capture_id, pending.capture_id);
    assert_eq!(dispatched.status, captures::CaptureState::Dispatched);
    dispatched
}

async fn submit(
    app: Router,
    capture_id: uuid::Uuid,
    event_bytes: &[u8; 276],
    observed_rfid: &[u8; 8],
    public_key: &[u8; 33],
    signature: &[u8; 64],
) -> StatusCode {
    let body = json!({
        "captureId": capture_id,
        "eventBytesBase64": BASE64.encode(event_bytes),
        "observedRfidHex": hex::encode(observed_rfid),
        "stationPubkeyHex": hex::encode(public_key),
        "stationSignatureHex": hex::encode(signature),
    });
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/agent/evidence")
            .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .expect("build evidence request"),
    )
    .await
    .expect("evidence response")
    .status()
}

fn signature(hex_value: &str) -> [u8; 64] {
    hex::decode(hex_value)
        .expect("valid signature fixture hex")
        .try_into()
        .expect("signature fixture is 64 bytes")
}

async fn assert_no_event_and_dispatched(db: &TestDb, capture_id: uuid::Uuid) {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(&db.pool)
        .await
        .expect("count events");
    assert_eq!(count, 0);
    let capture = captures::find_by_id(&db.pool, capture_id)
        .await
        .expect("load capture")
        .expect("capture exists");
    assert_eq!(capture.status, captures::CaptureState::Dispatched);
}

#[tokio::test]
async fn accepts_evidence_only_when_event_matches_capture_context() {
    // PURPOSE: Admit evidence only when signed bytes match the complete immutable capture context.
    // ASSERT: The exact submitted bytes are stored once and the capture advances to EVIDENCE_ACCEPTED.
    // FAILURE MEANS: Evidence could be reserialized or attached to a different requested operation.
    let db = TestDb::new().await;
    register(&db, ANIMAL_ID, "VISUAL-EVIDENCE-1").await;
    let event_bytes = fixture("origin.bin");
    let event = StationEvent::decode(&event_bytes).unwrap();
    let capture = create_dispatched_capture(&db, &capture_from_event(&event)).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));

    let status = submit(
        app(&db, rpc),
        capture.capture_id,
        &event_bytes,
        &OBSERVED_RFID_A,
        &STATION_PUBKEY,
        &signature(ORIGIN_SIGNATURE_HEX),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let stored = events::find_by_capture_id(&db.pool, capture.capture_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.event_bytes, event_bytes);
    assert_eq!(stored.observed_rfid, OBSERVED_RFID_A);
    assert_eq!(stored.station_pubkey, STATION_PUBKEY);
    assert_eq!(stored.station_signature, signature(ORIGIN_SIGNATURE_HEX));
    assert_eq!(stored.status, "EVIDENCE_ACCEPTED");
    let capture = captures::find_by_id(&db.pool, capture.capture_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(capture.status, captures::CaptureState::EvidenceAccepted);
    db.cleanup().await;
}

#[tokio::test]
async fn rejects_event_with_wrong_animal_id() {
    // PURPOSE: Prevent valid evidence for one AnimalID from being attached to another animal's capture.
    // ASSERT: The API returns conflict, creates no event, and leaves the capture DISPATCHED.
    // FAILURE MEANS: One physical session could be reassigned to a different persistent identity.
    let db = TestDb::new().await;
    register(&db, ANIMAL_ID, "VISUAL-EVIDENCE-2A").await;
    register(&db, OTHER_ANIMAL_ID, "VISUAL-EVIDENCE-2B").await;
    let event = fixture_event("origin.bin");
    let mut context = capture_from_event(&event);
    context.animal_id = OTHER_ANIMAL_ID;
    let capture = create_dispatched_capture(&db, &context).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));

    let status = submit(
        app(&db, rpc),
        capture.capture_id,
        &fixture("origin.bin"),
        &OBSERVED_RFID_A,
        &STATION_PUBKEY,
        &signature(ORIGIN_SIGNATURE_HEX),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_no_event_and_dispatched(&db, capture.capture_id).await;
    db.cleanup().await;
}

#[tokio::test]
async fn rejects_wrong_sequence_revision_or_predecessor() {
    // PURPOSE: Bind accepted evidence to the exact expected canonical predecessor context.
    // ASSERT: A valid signed TRANSFER is rejected when any one of sequence, revision, or predecessor differs from the capture.
    // FAILURE MEANS: Stale or competing history could enter the off-chain pipeline despite a valid Station signature.
    for case in ["sequence", "revision", "predecessor"] {
        let db = TestDb::new().await;
        register(&db, ANIMAL_ID, &format!("VISUAL-EVIDENCE-3-{case}")).await;
        let event = fixture_event("transfer.bin");
        let mut context = capture_from_event(&event);
        match case {
            "sequence" => context.event_sequence += 1,
            "revision" => context.identity_revision += 1,
            "predecessor" => context.previous_event_hash[0] ^= 0x01,
            _ => unreachable!(),
        }
        let capture = create_dispatched_capture(&db, &context).await;
        let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
        let status = submit(
            app(&db, rpc),
            capture.capture_id,
            &fixture("transfer.bin"),
            &OBSERVED_RFID_A,
            &STATION_PUBKEY,
            &signature(TRANSFER_SIGNATURE_HEX),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "case: {case}");
        assert_no_event_and_dispatched(&db, capture.capture_id).await;
        db.cleanup().await;
    }
}

#[tokio::test]
async fn rejects_station_id_not_derived_from_submitted_pubkey() {
    // PURPOSE: Cryptographically bind StationID to the public key that actually signed the StationEvent.
    // ASSERT: A mathematically valid signature is rejected when event.station_id is not derived from that public key.
    // FAILURE MEANS: Station identity could be forged independently from the signing key.
    let db = TestDb::new().await;
    register(&db, ANIMAL_ID, "VISUAL-EVIDENCE-4").await;
    let original = fixture_event("origin.bin");
    let capture = create_dispatched_capture(&db, &capture_from_event(&original)).await;
    let mut altered = original;
    altered.station_id = [0x99; 32];
    let (event_bytes, public_key, station_signature) =
        sign_station_event_with_test_scalar(&altered, 1);
    assert_eq!(public_key, STATION_PUBKEY);
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));

    let status = submit(
        app(&db, rpc),
        capture.capture_id,
        &event_bytes,
        &OBSERVED_RFID_A,
        &public_key,
        &station_signature,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_no_event_and_dispatched(&db, capture.capture_id).await;
    db.cleanup().await;
}

#[tokio::test]
async fn rejects_pubkey_not_registered_for_deployment() {
    // PURPOSE: Accept evidence only from the Station key registered in canonical ProtocolConfig.
    // ASSERT: A correctly signed event from a different P-256 key returns conflict and is not persisted.
    // FAILURE MEANS: Any device holding an arbitrary valid P-256 key could inject evidence.
    let db = TestDb::new().await;
    register(&db, ANIMAL_ID, "VISUAL-EVIDENCE-5").await;
    let original = fixture_event("origin.bin");
    let capture = create_dispatched_capture(&db, &capture_from_event(&original)).await;

    let (_, other_public_key, _) = sign_station_event_with_test_scalar(&original, 2);
    let mut other_event = original;
    other_event.station_id =
        derive_station_id(&other_public_key).expect("derive alternate StationID");
    let (event_bytes, other_public_key, other_signature) =
        sign_station_event_with_test_scalar(&other_event, 2);
    assert_ne!(other_public_key, STATION_PUBKEY);
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));

    let status = submit(
        app(&db, rpc),
        capture.capture_id,
        &event_bytes,
        &OBSERVED_RFID_A,
        &other_public_key,
        &other_signature,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_no_event_and_dispatched(&db, capture.capture_id).await;
    db.cleanup().await;
}

#[tokio::test]
async fn rejects_rfid_hash_not_matching_observed_rfid() {
    // PURPOSE: Bind the transported physical RFID bytes to new_rfid_hash inside the signed event.
    // ASSERT: Supplying RFID B with a valid event for RFID A returns bad request and persists nothing.
    // FAILURE MEANS: Evidence could name a physical observation that does not explain the signed state transition.
    let db = TestDb::new().await;
    register(&db, ANIMAL_ID, "VISUAL-EVIDENCE-6").await;
    let event = fixture_event("origin.bin");
    let capture = create_dispatched_capture(&db, &capture_from_event(&event)).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));

    let status = submit(
        app(&db, rpc),
        capture.capture_id,
        &fixture("origin.bin"),
        &OBSERVED_RFID_B,
        &STATION_PUBKEY,
        &signature(ORIGIN_SIGNATURE_HEX),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_no_event_and_dispatched(&db, capture.capture_id).await;
    db.cleanup().await;
}

#[tokio::test]
async fn rejects_invalid_p256_signature() {
    // PURPOSE: Require authentic Station authorization before evidence reaches durable API history.
    // ASSERT: One altered signature byte returns bad request, creates no event, and does not advance capture state.
    // FAILURE MEANS: Unsigned or tampered bytes could reach wallet transaction preparation.
    let db = TestDb::new().await;
    register(&db, ANIMAL_ID, "VISUAL-EVIDENCE-7").await;
    let event = fixture_event("origin.bin");
    let capture = create_dispatched_capture(&db, &capture_from_event(&event)).await;
    let mut invalid_signature = signature(ORIGIN_SIGNATURE_HEX);
    invalid_signature[0] ^= 0x01;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));

    let status = submit(
        app(&db, rpc),
        capture.capture_id,
        &fixture("origin.bin"),
        &OBSERVED_RFID_A,
        &STATION_PUBKEY,
        &invalid_signature,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_no_event_and_dispatched(&db, capture.capture_id).await;
    db.cleanup().await;
}

#[tokio::test]
async fn identical_duplicate_is_idempotent() {
    // PURPOSE: Make Agent retry safe when the first HTTP response is lost after commit.
    // ASSERT: The first POST returns 201, the identical retry returns 200, and exactly one byte-identical event remains.
    // FAILURE MEANS: Reliable delivery could duplicate history or fail after an ambiguous network outcome.
    let db = TestDb::new().await;
    register(&db, ANIMAL_ID, "VISUAL-EVIDENCE-8").await;
    let event_bytes = fixture("origin.bin");
    let event = StationEvent::decode(&event_bytes).unwrap();
    let capture = create_dispatched_capture(&db, &capture_from_event(&event)).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let station_signature = signature(ORIGIN_SIGNATURE_HEX);

    let first = submit(
        app(&db, rpc.clone()),
        capture.capture_id,
        &event_bytes,
        &OBSERVED_RFID_A,
        &STATION_PUBKEY,
        &station_signature,
    )
    .await;
    let second = submit(
        app(&db, rpc),
        capture.capture_id,
        &event_bytes,
        &OBSERVED_RFID_A,
        &STATION_PUBKEY,
        &station_signature,
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);
    assert_eq!(second, StatusCode::OK);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let stored = events::find_by_capture_id(&db.pool, capture.capture_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.event_bytes, event_bytes);
    assert_eq!(stored.station_signature, station_signature);
    db.cleanup().await;
}

#[tokio::test]
async fn divergent_duplicate_is_conflict() {
    // PURPOSE: Make capture_id an immutable evidence identity after the first accepted physical observation.
    // ASSERT: A second, independently valid RFID-B observation for the same capture returns 409 and cannot replace E1.
    // FAILURE MEANS: A retry or malicious Agent could rewrite already accepted physical evidence.
    let db = TestDb::new().await;
    register(&db, ANIMAL_ID, "VISUAL-EVIDENCE-9").await;
    let original_bytes = fixture("origin.bin");
    let original_event = StationEvent::decode(&original_bytes).unwrap();
    let capture = create_dispatched_capture(&db, &capture_from_event(&original_event)).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let original_signature = signature(ORIGIN_SIGNATURE_HEX);

    let first = submit(
        app(&db, rpc.clone()),
        capture.capture_id,
        &original_bytes,
        &OBSERVED_RFID_A,
        &STATION_PUBKEY,
        &original_signature,
    )
    .await;
    assert_eq!(first, StatusCode::CREATED);

    let mut divergent_event = original_event;
    divergent_event.new_rfid_hash = hash_canonical_rfid(&OBSERVED_RFID_B);
    let (divergent_bytes, divergent_key, divergent_signature) =
        sign_station_event_with_test_scalar(&divergent_event, 1);
    assert_eq!(divergent_key, STATION_PUBKEY);
    let second = submit(
        app(&db, rpc),
        capture.capture_id,
        &divergent_bytes,
        &OBSERVED_RFID_B,
        &divergent_key,
        &divergent_signature,
    )
    .await;
    assert_eq!(second, StatusCode::CONFLICT);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let stored = events::find_by_capture_id(&db.pool, capture.capture_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.event_bytes, original_bytes);
    assert_eq!(stored.observed_rfid, OBSERVED_RFID_A);
    assert_eq!(stored.station_signature, original_signature);
    db.cleanup().await;
}
