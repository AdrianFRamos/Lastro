//! Capture coordination integration contracts.

mod common;

use std::{fs, path::PathBuf, sync::Arc};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ed25519_dalek::{Signer, SigningKey};
use lastro_api::{
    repository::{animals, captures, events},
    routes,
    solana::rpc::{BindingStatus, CanonicalAnimalState, CanonicalRfidBinding},
};
use lastro_protocol::{Action, StationEvent, crypto::derive_station_id, rfid::hash_canonical_rfid};
use serde_json::{Value, json};
use tower::ServiceExt;

use common::{
    AGENT_TOKEN, DEPLOYMENT_ID, STATION_PUBKEY, TestDb, TestRpc, app_state,
    sign_station_event_with_test_scalar,
};

const ANIMAL_ID: [u8; 32] = [0x11; 32];
const ZERO32: [u8; 32] = [0; 32];
const RFID_A: [u8; 32] = [
    0x8a, 0x60, 0x45, 0x28, 0xf1, 0x90, 0x62, 0xcc, 0x9a, 0xda, 0x87, 0xa5, 0xe2, 0x25, 0xa5, 0xc9,
    0x67, 0xe2, 0xcf, 0x14, 0x8c, 0x85, 0xd8, 0x6d, 0xaf, 0x02, 0xd0, 0x42, 0x12, 0xeb, 0xf6, 0xe1,
];
const H1: [u8; 32] = [
    0x58, 0x45, 0xdc, 0x20, 0xfd, 0x6b, 0x26, 0x6e, 0xc9, 0x83, 0x99, 0xf0, 0xaa, 0x93, 0xc7, 0x36,
    0xec, 0x9a, 0xa7, 0x78, 0xbf, 0x03, 0x8a, 0xf5, 0x29, 0x1e, 0x5d, 0xf8, 0x13, 0x34, 0xb5, 0x31,
];
const OBSERVED_RFID_A: [u8; 8] = [0x80, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x01];
const OBSERVED_RFID_B: [u8; 8] = [0x80, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x02];

fn wallet_a() -> SigningKey {
    SigningKey::from_bytes(&[0x51; 32])
}

fn wallet_b() -> SigningKey {
    SigningKey::from_bytes(&[0x52; 32])
}

fn wallet_a_pubkey() -> [u8; 32] {
    wallet_a().verifying_key().to_bytes()
}

fn wallet_b_pubkey() -> [u8; 32] {
    wallet_b().verifying_key().to_bytes()
}

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name)).expect("read fixture")
}

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
    animals::insert_registration(&db.pool, ANIMAL_ID, "VISUAL-CAPTURE")
        .await
        .expect("register test animal");
}

fn canonical_origin_terminal() -> CanonicalAnimalState {
    CanonicalAnimalState {
        animal_id: ANIMAL_ID,
        current_rfid_hash: RFID_A,
        current_custodian: wallet_a_pubkey(),
        identity_revision: 1,
        event_sequence: 1,
        last_event_hash: H1,
    }
}

async fn post_capture(
    app: Router,
    action: &str,
    next_custodian: Option<[u8; 32]>,
) -> (StatusCode, Value) {
    post_capture_with_supersession(app, action, next_custodian, None).await
}

async fn post_capture_with_supersession(
    app: Router,
    action: &str,
    next_custodian: Option<[u8; 32]>,
    supersede_capture_id: Option<&str>,
) -> (StatusCode, Value) {
    let mut body = json!({
        "action": action,
        "animalId": hex::encode(ANIMAL_ID),
    });
    if let Some(capture_id) = supersede_capture_id {
        body["supersedeCaptureId"] = json!(capture_id);
    }
    if let Some(custodian) = next_custodian {
        body.as_object_mut().unwrap().insert(
            "nextCustodian".into(),
            Value::String(hex::encode(custodian)),
        );
    }

    let (challenge_status, challenge) = request_json(
        app.clone(),
        Request::builder()
            .method("POST")
            .uri("/api/captures/authorization-challenge")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
    )
    .await;
    if challenge_status != StatusCode::CREATED {
        return (challenge_status, challenge);
    }

    let required_signer = challenge["requiredSigner"].as_str().unwrap();
    let signing_key = if required_signer == bs58::encode(wallet_a_pubkey()).into_string() {
        wallet_a()
    } else if required_signer == bs58::encode(wallet_b_pubkey()).into_string() {
        wallet_b()
    } else {
        panic!("capture challenge requested an unexpected test wallet");
    };
    let message = BASE64
        .decode(challenge["messageBase64"].as_str().unwrap())
        .unwrap();
    let signature = signing_key.sign(&message).to_bytes();
    body.as_object_mut().unwrap().insert(
        "authorization".into(),
        json!({
            "challengeId": challenge["challengeId"],
            "signatureBase64": BASE64.encode(signature),
        }),
    );

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

fn signed_transfer_evidence() -> ([u8; 276], [u8; 64], [u8; 32]) {
    assert_eq!(hash_canonical_rfid(&OBSERVED_RFID_A), RFID_A);
    let mut event = StationEvent::decode(&fixture("transfer.bin")).unwrap();
    event.action = Action::Transfer;
    event.previous_event_hash = H1;
    event.old_rfid_hash = RFID_A;
    event.new_rfid_hash = RFID_A;
    event.from_custodian = wallet_a_pubkey();
    event.to_custodian = wallet_b_pubkey();
    let event_hash = event.event_hash();
    let (event_bytes, station_pubkey, station_signature) =
        sign_station_event_with_test_scalar(&event, 1);
    assert_eq!(station_pubkey, STATION_PUBKEY);
    (event_bytes, station_signature, event_hash)
}

fn signed_superseding_reidentify_evidence() -> ([u8; 276], [u8; 64], [u8; 32]) {
    let new_rfid_hash = hash_canonical_rfid(&OBSERVED_RFID_B);
    let mut event = StationEvent::decode(&fixture("reidentify.bin")).unwrap();
    event.deployment_id = DEPLOYMENT_ID;
    event.animal_id = ANIMAL_ID;
    event.event_sequence = 2;
    event.action = Action::Reidentify;
    event.identity_revision = 2;
    event.previous_event_hash = H1;
    event.old_rfid_hash = RFID_A;
    event.new_rfid_hash = new_rfid_hash;
    event.from_custodian = wallet_a_pubkey();
    event.to_custodian = wallet_a_pubkey();
    let event_hash = event.event_hash();
    let (event_bytes, station_pubkey, station_signature) =
        sign_station_event_with_test_scalar(&event, 1);
    assert_eq!(station_pubkey, STATION_PUBKEY);
    (event_bytes, station_signature, event_hash)
}

async fn create_origin_case(next: Option<[u8; 32]>) -> StatusCode {
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let status = post_capture(app(&db, rpc), "ORIGIN", next).await.0;
    db.cleanup().await;
    status
}

async fn create_transfer_case(next: Option<[u8; 32]>) -> StatusCode {
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_origin_terminal());
    let status = post_capture(app(&db, rpc), "TRANSFER", next).await.0;
    db.cleanup().await;
    status
}

async fn create_reidentify_case(next: Option<[u8; 32]>) -> StatusCode {
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_origin_terminal());
    let status = post_capture(app(&db, rpc), "REIDENTIFY", next).await.0;
    db.cleanup().await;
    status
}

#[tokio::test]
async fn origin_capture_derives_sequence_revision_and_zero_predecessor() {
    // PURPOSE: Derive ORIGIN context only from an unoriginated registered animal and the requested initial custodian.
    // ASSERT: Stored context is sequence 1/revision 1 with zero predecessor/old RFID/source custodian and contains no future RFID.
    // FAILURE MEANS: ORIGIN could inherit nonexistent state or let the client dictate a physical RFID observation.
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let (status, body) = post_capture(app(&db, rpc), "ORIGIN", Some(wallet_a_pubkey())).await;
    assert_eq!(status, StatusCode::CREATED);

    let capture_id = body["captureId"].as_str().unwrap().parse().unwrap();
    let record = captures::find_by_id(&db.pool, capture_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.action, 1);
    assert_eq!(record.animal_id, ANIMAL_ID);
    assert_eq!(record.event_sequence, 1);
    assert_eq!(record.identity_revision, 1);
    assert_eq!(record.expected_old_rfid_hash, ZERO32);
    assert_eq!(record.previous_event_hash, ZERO32);
    assert_eq!(record.from_custodian, ZERO32);
    assert_eq!(record.to_custodian, wallet_a_pubkey());
    assert!(body.get("newRfidHash").is_none());
    assert!(body.get("observedRfid").is_none());
    db.cleanup().await;
}

#[tokio::test]
async fn transfer_capture_derives_context_from_canonical_projection() {
    // PURPOSE: Freeze TRANSFER context from canonical Solana state rather than request-supplied history fields.
    // ASSERT: Sequence/revision/RFID/predecessor/source match canonical state and only the destination comes from user intent.
    // FAILURE MEANS: A client could fabricate custody history or predecessor state before Station signing.
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_origin_terminal());
    let (status, body) = post_capture(app(&db, rpc), "TRANSFER", Some(wallet_b_pubkey())).await;
    assert_eq!(status, StatusCode::CREATED);

    let capture_id = body["captureId"].as_str().unwrap().parse().unwrap();
    let record = captures::find_by_id(&db.pool, capture_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.action, 2);
    assert_eq!(record.event_sequence, 2);
    assert_eq!(record.identity_revision, 1);
    assert_eq!(record.expected_old_rfid_hash, RFID_A);
    assert_eq!(record.previous_event_hash, H1);
    assert_eq!(record.from_custodian, wallet_a_pubkey());
    assert_eq!(record.to_custodian, wallet_b_pubkey());
    db.cleanup().await;
}

#[tokio::test]
async fn reidentify_capture_has_no_new_rfid_from_backend() {
    // PURPOSE: Keep the replacement RFID exclusively in the Station's physical observation path.
    // ASSERT: REIDENTIFY carries the old RFID and incremented revision, while capture/Agent JSON contains no new-RFID field.
    // FAILURE MEANS: The backend could predetermine the physical identifier the Station is supposed to observe.
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_origin_terminal());
    let (status, capture) = post_capture(app(&db, rpc.clone()), "REIDENTIFY", None).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(capture.get("newRfidHash").is_none());

    let (_, command) = request_json(
        app(&db, rpc),
        Request::builder()
            .uri("/api/agent/commands")
            .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        command["expectedOldRfidHash"].as_str(),
        Some(hex::encode(RFID_A).as_str())
    );
    assert_eq!(command["identityRevision"], 2);
    assert!(command.get("newRfidHash").is_none());
    assert!(command.get("newRfid").is_none());
    assert!(command.get("observedRfid").is_none());
    db.cleanup().await;
}

#[tokio::test]
async fn action_specific_next_custodian_rules_are_enforced() {
    // PURPOSE: Prevent `nextCustodian` from changing the semantics of ORIGIN, TRANSFER, or REIDENTIFY.
    // ASSERT: ORIGIN requires non-zero destination; TRANSFER requires non-zero different destination; REIDENTIFY forbids any destination.
    // FAILURE MEANS: A client could turn one domain action into another or consume sequence with an ambiguous no-op transfer.
    assert_eq!(create_origin_case(None).await, StatusCode::BAD_REQUEST);
    assert_eq!(
        create_origin_case(Some(ZERO32)).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        create_origin_case(Some(wallet_a_pubkey())).await,
        StatusCode::CREATED
    );

    assert_eq!(create_transfer_case(None).await, StatusCode::BAD_REQUEST);
    assert_eq!(
        create_transfer_case(Some(ZERO32)).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        create_transfer_case(Some(wallet_a_pubkey())).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        create_transfer_case(Some(wallet_b_pubkey())).await,
        StatusCode::CREATED
    );

    assert_eq!(
        create_reidentify_case(Some(wallet_b_pubkey())).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(create_reidentify_case(None).await, StatusCode::CREATED);
}

#[tokio::test]
async fn capture_authorization_stays_separate_from_onchain_transaction_authority() {
    // PURPOSE: Require pre-capture wallet proof without confusing it with final on-chain transaction authority.
    // ASSERT: The capture response leaks no authorization material; transaction-data still requires the same canonical current custodian.
    // FAILURE MEANS: off-chain reservation proof could be mistaken for final custody authorization or leak reusable proof material.
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_origin_terminal());
    rpc.set_binding(CanonicalRfidBinding {
        animal_id: ANIMAL_ID,
        rfid_hash: RFID_A,
        status: BindingStatus::Active,
    });

    let (status, capture) =
        post_capture(app(&db, rpc.clone()), "TRANSFER", Some(wallet_b_pubkey())).await;
    assert_eq!(status, StatusCode::CREATED);
    for forbidden in ["authorized", "authorization", "wallet", "requiredSigner"] {
        assert!(capture.get(forbidden).is_none());
    }
    let capture_id = capture["captureId"].as_str().unwrap();

    let (command_status, _) = request_json(
        app(&db, rpc.clone()),
        Request::builder()
            .uri("/api/agent/commands")
            .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(command_status, StatusCode::OK);

    let (event_bytes, station_signature, event_hash) = signed_transfer_evidence();
    let evidence = json!({
        "captureId": capture_id,
        "eventBytesBase64": BASE64.encode(event_bytes),
        "observedRfidHex": hex::encode(OBSERVED_RFID_A),
        "stationPubkeyHex": hex::encode(STATION_PUBKEY),
        "stationSignatureHex": hex::encode(station_signature)
    });
    let evidence_response = app(&db, rpc.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/evidence")
                .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(evidence.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(evidence_response.status(), StatusCode::CREATED);

    let (tx_status, transaction) = request_json(
        app(&db, rpc),
        Request::builder()
            .uri(format!(
                "/api/events/{}/transaction-data",
                hex::encode(event_hash)
            ))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(tx_status, StatusCode::OK);
    let expected_signer = bs58::encode(wallet_a_pubkey()).into_string();
    assert_eq!(
        transaction["requiredSigner"].as_str(),
        Some(expected_signer.as_str())
    );
    assert_eq!(transaction["instructions"].as_array().unwrap().len(), 2);
    db.cleanup().await;
}

#[tokio::test]
async fn second_active_capture_same_station_conflicts() {
    // PURPOSE: Serialize physical work for one Station.
    // ASSERT: A second active capture returns HTTP 409 and exactly one PENDING/DISPATCHED row remains.
    // FAILURE MEANS: The Station could receive two competing immutable contexts for one physical read.
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));

    assert_eq!(
        post_capture(app(&db, rpc.clone()), "ORIGIN", Some(wallet_a_pubkey()))
            .await
            .0,
        StatusCode::CREATED
    );
    assert_eq!(
        post_capture(app(&db, rpc), "ORIGIN", Some(wallet_a_pubkey()))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM captures WHERE status IN ('PENDING','DISPATCHED')",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(active, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn expired_capture_cannot_accept_evidence() {
    // PURPOSE: Reject physically valid evidence that arrives outside the capture's operational lifetime.
    // ASSERT: Expired DISPATCHED capture becomes EXPIRED, evidence POST returns 409, and no event row is created.
    // FAILURE MEANS: Old Station evidence could be attached to a stale coordination session.
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let (status, capture) =
        post_capture(app(&db, rpc.clone()), "ORIGIN", Some(wallet_a_pubkey())).await;
    assert_eq!(status, StatusCode::CREATED);
    let capture_id = capture["captureId"].as_str().unwrap();

    let (claim_status, _) = request_json(
        app(&db, rpc.clone()),
        Request::builder()
            .uri("/api/agent/commands")
            .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(claim_status, StatusCode::OK);
    sqlx::query(
        "UPDATE captures SET expires_at=created_at+interval '1 millisecond' WHERE capture_id=$1",
    )
    .bind(capture_id.parse::<uuid::Uuid>().unwrap())
    .execute(&db.pool)
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    let evidence = json!({
        "captureId": capture_id,
        "eventBytesBase64": BASE64.encode(fixture("origin.bin")),
        "observedRfidHex": "8000130000000001",
        "stationPubkeyHex": hex::encode(STATION_PUBKEY),
        "stationSignatureHex": "09579de39fbd8108dea2fc89262240c45b2043d85e6e16854d9bed737e6426e109be548b3eb9d3ba3252aeba61af230a18115aeaf0b74734df88c90955f9184f"
    });
    let response = app(&db, rpc)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/evidence")
                .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(evidence.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);

    let stored_status: String =
        sqlx::query_scalar("SELECT status FROM captures WHERE capture_id=$1")
            .bind(capture_id.parse::<uuid::Uuid>().unwrap())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(stored_status, "EXPIRED");
    let events: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(events, 0);
    db.cleanup().await;
}

#[test]
fn station_id_fixture_matches_capture_configuration() {
    // PURPOSE: Keep capture tests bound to the same Station identity as the frozen cross-language vectors.
    // ASSERT: The configured compressed P-256 key derives the StationID carried by the vectors.
    // FAILURE MEANS: Integration fixtures would exercise a different Station than production capture validation.
    assert_eq!(
        derive_station_id(&STATION_PUBKEY).unwrap(),
        <[u8; 32]>::try_from(
            hex::decode("56c266d8ab41a37e7a3bb80b33f6249c05bdb965c68bbeb4775303c69594df77")
                .unwrap()
        )
        .unwrap()
    );
    assert_eq!(DEPLOYMENT_ID, [0xd0; 32]);
}

#[tokio::test]
async fn dispatched_capture_is_redelivered_identically_until_evidence_is_accepted() {
    // PURPOSE: Recover an Agent restart that happened after COMMAND dispatch but before durable evidence admission.
    // ARRANGE: Create one ORIGIN capture and poll the authenticated Agent command endpoint once.
    // ACTION: Poll the same endpoint again while the capture remains DISPATCHED and unexpired.
    // ASSERT: Both responses are the same immutable command/capture and the database still contains one active capture.
    // FAILURE MEANS: a pre-LOCAL Agent crash can strand the Station/capture until expiry or require manual database edits.
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let (status, capture) =
        post_capture(app(&db, rpc.clone()), "ORIGIN", Some(wallet_a_pubkey())).await;
    assert_eq!(status, StatusCode::CREATED);

    let request = || {
        Request::builder()
            .uri("/api/agent/commands")
            .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
            .body(Body::empty())
            .unwrap()
    };
    let (first_status, first) = request_json(app(&db, rpc.clone()), request()).await;
    let (second_status, second) = request_json(app(&db, rpc), request()).await;
    assert_eq!(first_status, StatusCode::OK);
    assert_eq!(second_status, StatusCode::OK);
    assert_eq!(first, second);
    assert_eq!(first["captureId"], capture["captureId"]);

    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM captures WHERE status IN ('PENDING','DISPATCHED')",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(active, 1);
    let stored_status: String =
        sqlx::query_scalar("SELECT status FROM captures WHERE capture_id=$1")
            .bind(
                capture["captureId"]
                    .as_str()
                    .unwrap()
                    .parse::<uuid::Uuid>()
                    .unwrap(),
            )
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(stored_status, "DISPATCHED");
    db.cleanup().await;
}

#[tokio::test]
async fn accepted_unsubmitted_event_can_be_superseded_by_new_authorized_intent() {
    // PURPOSE: Recover from valid Station evidence that the custodian intentionally abandons before any transaction is submitted.
    // ARRANGE: Canonical state is sequence 1; accept a valid A->B TRANSFER event for sequence 2 without storing a transaction signature.
    // ACTION: Retry the same intent, then authorize a different REIDENTIFY intent and admit its replacement sequence-2 Station evidence.
    // ASSERT: Same-intent retry remains idempotent; the abandoned event becomes REJECTED, its capture becomes CANCELLED,
    //         the replacement capture is created, and a new sequence-2 event can be accepted without deleting immutable evidence.
    // FAILURE MEANS: accepted-but-unsubmitted evidence can permanently strand the animal or replacement requires destructive history edits.
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_origin_terminal());
    rpc.set_binding(CanonicalRfidBinding {
        animal_id: ANIMAL_ID,
        rfid_hash: RFID_A,
        status: BindingStatus::Active,
    });

    let (create_status, capture) =
        post_capture(app(&db, rpc.clone()), "TRANSFER", Some(wallet_b_pubkey())).await;
    assert_eq!(create_status, StatusCode::CREATED);
    let old_capture_id = capture["captureId"].as_str().unwrap().to_string();

    let (claim_status, _) = request_json(
        app(&db, rpc.clone()),
        Request::builder()
            .uri("/api/agent/commands")
            .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(claim_status, StatusCode::OK);

    let (event_bytes, station_signature, old_event_hash) = signed_transfer_evidence();
    let evidence = json!({
        "captureId": old_capture_id,
        "eventBytesBase64": BASE64.encode(event_bytes),
        "observedRfidHex": hex::encode(OBSERVED_RFID_A),
        "stationPubkeyHex": hex::encode(STATION_PUBKEY),
        "stationSignatureHex": hex::encode(station_signature)
    });
    let evidence_response = app(&db, rpc.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/evidence")
                .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(evidence.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(evidence_response.status(), StatusCode::CREATED);

    let (same_status, same_body) =
        post_capture(app(&db, rpc.clone()), "TRANSFER", Some(wallet_b_pubkey())).await;
    assert_eq!(same_status, StatusCode::OK);
    assert_eq!(same_body["captureId"], capture["captureId"]);
    assert_eq!(same_body["eventHash"], hex::encode(old_event_hash));

    let (replacement_status, replacement) =
        post_capture(app(&db, rpc.clone()), "REIDENTIFY", None).await;
    assert_eq!(replacement_status, StatusCode::CREATED);
    assert_ne!(replacement["captureId"], capture["captureId"]);
    assert_eq!(replacement["status"], "PENDING");

    let old_event_status: String =
        sqlx::query_scalar("SELECT status FROM events WHERE event_hash=$1")
            .bind(old_event_hash.to_vec())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(old_event_status, "REJECTED");
    let old_capture_status: String =
        sqlx::query_scalar("SELECT status FROM captures WHERE capture_id=$1")
            .bind(old_capture_id.parse::<uuid::Uuid>().unwrap())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(old_capture_status, "CANCELLED");

    let (replacement_claim_status, command) = request_json(
        app(&db, rpc.clone()),
        Request::builder()
            .uri("/api/agent/commands")
            .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(replacement_claim_status, StatusCode::OK);
    assert_eq!(command["captureId"], replacement["captureId"]);

    let (replacement_bytes, replacement_signature, replacement_event_hash) =
        signed_superseding_reidentify_evidence();
    let replacement_evidence = json!({
        "captureId": replacement["captureId"],
        "eventBytesBase64": BASE64.encode(replacement_bytes),
        "observedRfidHex": hex::encode(OBSERVED_RFID_B),
        "stationPubkeyHex": hex::encode(STATION_PUBKEY),
        "stationSignatureHex": hex::encode(replacement_signature)
    });
    let replacement_response = app(&db, rpc)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/evidence")
                .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(replacement_evidence.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replacement_response.status(), StatusCode::CREATED);

    let accepted_replacement: String =
        sqlx::query_scalar("SELECT status FROM events WHERE event_hash=$1")
            .bind(replacement_event_hash.to_vec())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(accepted_replacement, "EVIDENCE_ACCEPTED");
    let sequence_two_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM events WHERE animal_id=$1 AND event_sequence=2")
            .bind(ANIMAL_ID.to_vec())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(sequence_two_rows, 2);
    db.cleanup().await;
}

#[tokio::test]
async fn submitted_event_cannot_be_superseded_by_a_different_capture_intent() {
    // PURPOSE: Never abandon an event after a confirmed transaction has been bound to it.
    // ARRANGE: Accept sequence-2 TRANSFER evidence and durably mark it SUBMITTED with one transaction signature.
    // ACTION: Request a different REIDENTIFY capture while canonical Solana still exposes the predecessor state.
    // ASSERT: The authorization challenge conflicts; the submitted event/capture remain unchanged and no replacement capture exists.
    // FAILURE MEANS: the API could race or contradict a transaction that may already finalize on Solana.
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_origin_terminal());
    rpc.set_binding(CanonicalRfidBinding {
        animal_id: ANIMAL_ID,
        rfid_hash: RFID_A,
        status: BindingStatus::Active,
    });

    let (create_status, capture) =
        post_capture(app(&db, rpc.clone()), "TRANSFER", Some(wallet_b_pubkey())).await;
    assert_eq!(create_status, StatusCode::CREATED);
    let capture_id = capture["captureId"].as_str().unwrap();

    let (claim_status, _) = request_json(
        app(&db, rpc.clone()),
        Request::builder()
            .uri("/api/agent/commands")
            .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(claim_status, StatusCode::OK);

    let (event_bytes, station_signature, event_hash) = signed_transfer_evidence();
    let evidence = json!({
        "captureId": capture_id,
        "eventBytesBase64": BASE64.encode(event_bytes),
        "observedRfidHex": hex::encode(OBSERVED_RFID_A),
        "stationPubkeyHex": hex::encode(STATION_PUBKEY),
        "stationSignatureHex": hex::encode(station_signature)
    });
    let response = app(&db, rpc.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent/evidence")
                .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(evidence.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let tx_signature = bs58::encode([0x77u8; 64]).into_string();
    events::mark_submitted(&db.pool, event_hash, &tx_signature)
        .await
        .unwrap();

    let (different_status, different_body) = post_capture(app(&db, rpc), "REIDENTIFY", None).await;
    assert_eq!(different_status, StatusCode::CONFLICT);
    assert!(different_body.to_string().contains("submitted transition"));

    let stored_event = events::find_by_hash(&db.pool, event_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored_event.status, "SUBMITTED");
    assert_eq!(
        stored_event.tx_signature.as_deref(),
        Some(tx_signature.as_str())
    );
    let stored_capture: String =
        sqlx::query_scalar("SELECT status FROM captures WHERE capture_id=$1")
            .bind(capture_id.parse::<uuid::Uuid>().unwrap())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(stored_capture, "EVIDENCE_ACCEPTED");
    let capture_count: i64 = sqlx::query_scalar("SELECT count(*) FROM captures")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(capture_count, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn a_signed_explicit_reidentify_recapture_replaces_the_exact_accepted_evidence() {
    // PURPOSE: A failed RFID choice must not silently reuse the same accepted tag on retry.
    // ARRANGE: Accept REIDENTIFY evidence under a known unsigned capture.
    // ACTION: Repeat the ordinary request, then explicitly sign a request naming that capture.
    // ASSERT: Ordinary retry is idempotent; the explicit retry creates a new pending capture and
    // rejects only the original unsubmitted evidence under the same immutable predecessor.
    // FAILURE MEANS: a custodian cannot correct an accepted REIDENTIFY observation before signing.
    let db = TestDb::new().await;
    register(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(canonical_origin_terminal());
    rpc.set_binding(CanonicalRfidBinding {
        animal_id: ANIMAL_ID,
        rfid_hash: RFID_A,
        status: BindingStatus::Active,
    });
    let (status, first) = post_capture(app(&db, rpc.clone()), "REIDENTIFY", None).await;
    assert_eq!(status, StatusCode::CREATED);
    let first_id = first["captureId"].as_str().unwrap();
    let (claim, _) = request_json(
        app(&db, rpc.clone()),
        Request::builder()
            .uri("/api/agent/commands")
            .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(claim, StatusCode::OK);
    let (event_bytes, station_signature, event_hash) = signed_superseding_reidentify_evidence();
    let (accepted, _) = request_json(
        app(&db, rpc.clone()),
        Request::builder()
            .method("POST")
            .uri("/api/agent/evidence")
            .header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "captureId": first_id,
                    "eventBytesBase64": BASE64.encode(event_bytes),
                    "observedRfidHex": hex::encode(OBSERVED_RFID_B),
                    "stationPubkeyHex": hex::encode(STATION_PUBKEY),
                    "stationSignatureHex": hex::encode(station_signature)
                })
                .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(accepted, StatusCode::CREATED);

    let (same, ordinary) = post_capture(app(&db, rpc.clone()), "REIDENTIFY", None).await;
    assert_eq!(same, StatusCode::OK);
    assert_eq!(ordinary["captureId"], first["captureId"]);

    let (replacement_status, replacement) =
        post_capture_with_supersession(app(&db, rpc.clone()), "REIDENTIFY", None, Some(first_id))
            .await;
    assert_eq!(replacement_status, StatusCode::CREATED);
    assert_ne!(replacement["captureId"], first["captureId"]);
    let old = events::find_by_hash(&db.pool, event_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old.status, "REJECTED");
    let (stale, _) =
        post_capture_with_supersession(app(&db, rpc), "REIDENTIFY", None, Some(first_id)).await;
    assert_eq!(stale, StatusCode::CONFLICT);
    db.cleanup().await;
}
