//! Finalized Solana confirmation integration contracts.

mod common;

use std::{fs, path::PathBuf, sync::Arc};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use lastro_api::{
    crypto::verify_agent_evidence,
    model::AgentEvidenceRequest,
    repository::{animals, captures, events},
    routes,
    solana::rpc::{BindingStatus, CanonicalAnimalState, CanonicalRfidBinding},
};
use lastro_protocol::StationEvent;
use serde_json::json;
use sqlx::Row;
use tower::ServiceExt;

use common::{STATION_PUBKEY, TestDb, TestRpc, app_state};

const ANIMAL_ID: [u8; 32] = [0x11; 32];
const OBSERVED_RFID_A: [u8; 8] = [0x80, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x01];
const ORIGIN_SIGNATURE_HEX: &str = "09579de39fbd8108dea2fc89262240c45b2043d85e6e16854d9bed737e6426e109be548b3eb9d3ba3252aeba61af230a18115aeaf0b74734df88c90955f9184f";

fn fixture(name: &str) -> [u8; 276] {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name))
        .expect("read fixture")
        .try_into()
        .expect("StationEvent fixture is exactly 276 bytes")
}

fn app(db: &TestDb, rpc: Arc<TestRpc>) -> Router {
    routes::router(app_state(db.pool.clone(), rpc))
}

fn tx_signature() -> String {
    bs58::encode([0x55u8; 64]).into_string()
}

async fn seed_origin_event(db: &TestDb) -> StationEvent {
    animals::insert_registration(&db.pool, ANIMAL_ID, "VISUAL-CONFIRM")
        .await
        .expect("register test animal");
    let event_bytes = fixture("origin.bin");
    let event = StationEvent::decode(&event_bytes).expect("decode origin fixture");
    let pending = captures::insert_pending(
        &db.pool,
        &captures::NewCapture {
            station_id: event.station_id,
            action: event.action as u8,
            animal_id: event.animal_id,
            event_sequence: event.event_sequence,
            identity_revision: event.identity_revision,
            expected_old_rfid_hash: event.old_rfid_hash,
            from_custodian: event.from_custodian,
            to_custodian: event.to_custodian,
            previous_event_hash: event.previous_event_hash,
        },
    )
    .await
    .expect("insert capture");
    let dispatched = captures::claim_next_for_station(&db.pool, event.station_id)
        .await
        .expect("claim capture")
        .expect("capture available");
    assert_eq!(dispatched.capture_id, pending.capture_id);

    let request = AgentEvidenceRequest {
        capture_id: pending.capture_id,
        event_bytes_base64: BASE64.encode(event_bytes),
        observed_rfid_hex: hex::encode(OBSERVED_RFID_A),
        station_pubkey_hex: hex::encode(STATION_PUBKEY),
        station_signature_hex: ORIGIN_SIGNATURE_HEX.into(),
    };
    let verified =
        verify_agent_evidence(&request, &STATION_PUBKEY).expect("verify fixture evidence");
    let mut tx = db.pool.begin().await.expect("begin evidence transaction");
    assert!(
        events::insert_or_match_exact(&mut tx, pending.capture_id, &verified)
            .await
            .expect("insert event")
    );
    captures::mark_evidence_accepted(&mut tx, pending.capture_id)
        .await
        .expect("advance capture");
    tx.commit().await.expect("commit accepted evidence");
    event
}

fn terminal_state(event: &StationEvent) -> CanonicalAnimalState {
    CanonicalAnimalState {
        animal_id: event.animal_id,
        current_rfid_hash: event.new_rfid_hash,
        current_custodian: event.to_custodian,
        identity_revision: event.identity_revision,
        event_sequence: event.event_sequence,
        last_event_hash: event.event_hash(),
    }
}

fn active_binding(event: &StationEvent) -> CanonicalRfidBinding {
    CanonicalRfidBinding {
        animal_id: event.animal_id,
        rfid_hash: event.new_rfid_hash,
        status: BindingStatus::Active,
    }
}

async fn submit(app: Router, event: &StationEvent, signature: &str) -> StatusCode {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(format!(
                "/api/events/{}/submit",
                hex::encode(event.event_hash())
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"txSignature": signature}).to_string()))
            .expect("build submission request"),
    )
    .await
    .expect("submission response")
    .status()
}

async fn get_json(app: Router, uri: String) -> (StatusCode, serde_json::Value) {
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .expect("GET response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read JSON body");
    let body = serde_json::from_slice(&bytes).expect("JSON response body");
    (status, body)
}

async fn confirm(app: Router, event: &StationEvent, signature: &str) -> StatusCode {
    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(format!(
                "/api/events/{}/confirm",
                hex::encode(event.event_hash())
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({"txSignature": signature}).to_string()))
            .expect("build confirmation request"),
    )
    .await
    .expect("confirmation response")
    .status()
}

async fn assert_unconfirmed(db: &TestDb, event: &StationEvent) {
    let animal = animals::find_by_animal_id(&db.pool, event.animal_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(animal.event_sequence, 0);
    assert_eq!(animal.identity_revision, 0);
    assert_eq!(animal.current_rfid_hash, None);
    assert_eq!(animal.current_custodian, None);
    assert_eq!(animal.last_event_hash, None);
    let stored = events::find_by_hash(&db.pool, event.event_hash())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, "EVIDENCE_ACCEPTED");
    assert_eq!(stored.tx_signature, None);
}

async fn assert_submitted(db: &TestDb, event: &StationEvent, signature: &str) {
    let animal = animals::find_by_animal_id(&db.pool, event.animal_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(animal.event_sequence, 0);
    assert_eq!(animal.identity_revision, 0);
    assert_eq!(animal.current_rfid_hash, None);
    assert_eq!(animal.current_custodian, None);
    assert_eq!(animal.last_event_hash, None);
    let stored = events::find_by_hash(&db.pool, event.event_hash())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, "SUBMITTED");
    assert_eq!(stored.tx_signature.as_deref(), Some(signature));
}

#[tokio::test]
async fn submit_requires_exact_transaction_at_confirmed_commitment() {
    // PURPOSE: Never persist a caller-supplied transaction signature before confirmed RPC proves the exact Lastro envelope.
    // ARRANGE: Valid accepted Station evidence exists, but confirmed RPC cannot find/match the submitted signature.
    // ACTION: POST the signature to the submission endpoint.
    // ASSERT: The API returns 409 and leaves both the event and local projection unchanged.
    // FAILURE MEANS: An arbitrary signature can become durable workflow state and poison reload recovery.
    let db = TestDb::new().await;
    let event = seed_origin_event(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_confirmed_transaction_match(false);
    let signature = tx_signature();

    assert_eq!(
        submit(app(&db, rpc), &event, &signature).await,
        StatusCode::CONFLICT
    );
    assert_unconfirmed(&db, &event).await;
    db.cleanup().await;
}

#[tokio::test]
async fn submit_persists_verified_signature_without_advancing_projection() {
    // PURPOSE: Make confirmed-but-not-finalized wallet submission recoverable without treating PostgreSQL as canonical.
    // ARRANGE: Accepted evidence and an exact transaction visible at confirmed commitment.
    // ACTION: Submit the transaction signature.
    // ASSERT: Event becomes SUBMITTED with that signature while the animal projection remains at its predecessor.
    // FAILURE MEANS: Either reload loses a verified transaction or confirmed commitment prematurely mutates projected canonical state.
    let db = TestDb::new().await;
    let event = seed_origin_event(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let signature = tx_signature();

    assert_eq!(
        submit(app(&db, rpc), &event, &signature).await,
        StatusCode::OK
    );
    assert_submitted(&db, &event, &signature).await;
    db.cleanup().await;
}

#[tokio::test]
async fn submitted_transaction_is_idempotent_recoverable_and_cannot_be_reprepared() {
    // PURPOSE: Make the durable SUBMITTED state sufficient for reload recovery while preventing a second wallet transaction for the same event.
    // ARRANGE: Accepted evidence is submitted once with an exact confirmed transaction signature.
    // ACTION: Retry the same submission, try a different signature, read the capture, and request transaction-data again.
    // ASSERT: Same-signature submit is idempotent; different signature and re-preparation conflict; capture exposes the verified signature/status.
    // FAILURE MEANS: reload either loses the transaction identity or can authorize the same immutable event twice.
    let db = TestDb::new().await;
    let event = seed_origin_event(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let signature = tx_signature();

    assert_eq!(
        submit(app(&db, rpc.clone()), &event, &signature).await,
        StatusCode::OK
    );
    assert_eq!(
        submit(app(&db, rpc.clone()), &event, &signature).await,
        StatusCode::OK
    );
    let different_signature = bs58::encode([0x56u8; 64]).into_string();
    assert_eq!(
        submit(app(&db, rpc.clone()), &event, &different_signature).await,
        StatusCode::CONFLICT
    );

    let stored = events::find_by_hash(&db.pool, event.event_hash())
        .await
        .unwrap()
        .unwrap();
    let (capture_status, capture) = get_json(
        app(&db, rpc.clone()),
        format!("/api/captures/{}", stored.capture_id),
    )
    .await;
    assert_eq!(capture_status, StatusCode::OK);
    assert_eq!(capture["eventStatus"], "SUBMITTED");
    assert_eq!(capture["txSignature"], signature);
    assert_eq!(capture["eventHash"], hex::encode(event.event_hash()));

    let (prepare_status, _) = get_json(
        app(&db, rpc),
        format!(
            "/api/events/{}/transaction-data",
            hex::encode(event.event_hash())
        ),
    )
    .await;
    assert_eq!(prepare_status, StatusCode::CONFLICT);
    assert_submitted(&db, &event, &signature).await;
    db.cleanup().await;
}

#[tokio::test]
async fn confirm_rejects_transaction_not_preverified_at_confirmed_commitment() {
    // PURPOSE: Enforce the explicit EVIDENCE_ACCEPTED -> SUBMITTED -> FINALIZED lifecycle.
    // ARRANGE: Accepted evidence exists but the submit phase was never completed.
    // ACTION: Call confirm directly with a syntactically valid transaction signature.
    // ASSERT: The API returns 409 without querying terminal state or changing PostgreSQL.
    // FAILURE MEANS: Confirmation can bypass the durable submitted-signature trust boundary.
    let db = TestDb::new().await;
    let event = seed_origin_event(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));

    assert_eq!(
        confirm(app(&db, rpc), &event, &tx_signature()).await,
        StatusCode::CONFLICT
    );
    assert_unconfirmed(&db, &event).await;
    db.cleanup().await;
}

#[tokio::test]
async fn confirm_requires_transaction_to_exist_on_finalized_rpc() {
    // PURPOSE: Never finalize from a confirmed or caller-supplied transaction signature alone.
    // ARRANGE: The exact transaction was verified and persisted as SUBMITTED, but finalized RPC cannot match it.
    // ACTION: Attempt final confirmation.
    // ASSERT: Confirmation returns 409, the event remains SUBMITTED, and projection does not advance.
    // FAILURE MEANS: PostgreSQL could become canonical before Solana finalization.
    let db = TestDb::new().await;
    let event = seed_origin_event(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let signature = tx_signature();
    assert_eq!(
        submit(app(&db, rpc.clone()), &event, &signature).await,
        StatusCode::OK
    );
    rpc.set_transaction_match(false);
    rpc.set_animal(terminal_state(&event));
    rpc.set_binding(active_binding(&event));

    let status = confirm(app(&db, rpc), &event, &signature).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_submitted(&db, &event, &signature).await;
    db.cleanup().await;
}

#[tokio::test]
async fn confirm_reads_animal_state_and_rfid_binding_from_rpc() {
    // PURPOSE: Make canonical Solana accounts, not the transaction signature or PostgreSQL, authoritative for confirmation.
    // ASSERT: A matching transaction plus AnimalState is insufficient without the ACTIVE binding; adding both allows one projection update.
    // FAILURE MEANS: Confirmation could trust transaction existence without proving the canonical terminal accounts.
    let db = TestDb::new().await;
    let event = seed_origin_event(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(terminal_state(&event));
    let signature = tx_signature();
    assert_eq!(
        submit(app(&db, rpc.clone()), &event, &signature).await,
        StatusCode::OK
    );

    let missing_binding = confirm(app(&db, rpc.clone()), &event, &signature).await;
    assert_eq!(missing_binding, StatusCode::CONFLICT);
    assert_submitted(&db, &event, &signature).await;

    rpc.set_binding(active_binding(&event));
    let status = confirm(app(&db, rpc), &event, &signature).await;
    assert_eq!(status, StatusCode::OK);
    let animal = animals::find_by_animal_id(&db.pool, event.animal_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(animal.current_rfid_hash, Some(event.new_rfid_hash));
    assert_eq!(animal.current_custodian, Some(event.to_custodian));
    assert_eq!(animal.identity_revision, event.identity_revision);
    assert_eq!(animal.event_sequence, event.event_sequence);
    assert_eq!(animal.last_event_hash, Some(event.event_hash()));
    let stored = events::find_by_hash(&db.pool, event.event_hash())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, "FINALIZED");
    assert_eq!(stored.tx_signature.as_deref(), Some(signature.as_str()));
    db.cleanup().await;
}

#[tokio::test]
async fn confirm_rejects_rpc_state_not_matching_event_terminal() {
    // PURPOSE: Refuse projection updates when finalized canonical state differs from the signed StationEvent terminal state.
    // ASSERT: A wrong canonical custodian returns 409 and leaves both event and animal projection unchanged.
    // FAILURE MEANS: PostgreSQL could silently diverge from Solana after confirmation.
    let db = TestDb::new().await;
    let event = seed_origin_event(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let mut wrong = terminal_state(&event);
    wrong.current_custodian = [0xee; 32];
    rpc.set_animal(wrong);
    rpc.set_binding(active_binding(&event));
    let signature = tx_signature();
    assert_eq!(
        submit(app(&db, rpc.clone()), &event, &signature).await,
        StatusCode::OK
    );

    let status = confirm(app(&db, rpc), &event, &signature).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_submitted(&db, &event, &signature).await;
    db.cleanup().await;
}

#[tokio::test]
async fn confirm_is_idempotent_for_same_finalized_transaction() {
    // PURPOSE: Make ambiguous confirmation retries safe after the first database commit.
    // ASSERT: Repeating the same finalized transaction succeeds without a second state transition or updated_at mutation.
    // FAILURE MEANS: A retry could advance counters twice or rewrite terminal projection metadata.
    let db = TestDb::new().await;
    let event = seed_origin_event(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_animal(terminal_state(&event));
    rpc.set_binding(active_binding(&event));
    let signature = tx_signature();
    assert_eq!(
        submit(app(&db, rpc.clone()), &event, &signature).await,
        StatusCode::OK
    );

    assert_eq!(
        confirm(app(&db, rpc.clone()), &event, &signature).await,
        StatusCode::OK
    );
    let before = sqlx::query("SELECT event_sequence,identity_revision,updated_at::text AS updated_at FROM animals WHERE animal_id=$1")
        .bind(event.animal_id.to_vec())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let before_sequence: i64 = before.get("event_sequence");
    let before_revision: i32 = before.get("identity_revision");
    let before_updated_at: String = before.get("updated_at");

    assert_eq!(
        confirm(app(&db, rpc), &event, &signature).await,
        StatusCode::OK
    );
    let after = sqlx::query("SELECT event_sequence,identity_revision,updated_at::text AS updated_at FROM animals WHERE animal_id=$1")
        .bind(event.animal_id.to_vec())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(after.get::<i64, _>("event_sequence"), before_sequence);
    assert_eq!(after.get::<i32, _>("identity_revision"), before_revision);
    assert_eq!(after.get::<String, _>("updated_at"), before_updated_at);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM events WHERE animal_id=$1")
        .bind(event.animal_id.to_vec())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn confirm_does_not_accept_wrong_program_id() {
    // PURPOSE: Require the finalized transaction envelope to target the configured Lastro program exactly.
    // ASSERT: An RPC envelope mismatch is rejected before any PostgreSQL projection update; rpc.rs separately tests the wrong-program discriminator itself.
    // FAILURE MEANS: An unrelated finalized transaction could be presented as Lastro authorization.
    let db = TestDb::new().await;
    let event = seed_origin_event(&db).await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_transaction_match(false);
    rpc.set_animal(terminal_state(&event));
    rpc.set_binding(active_binding(&event));
    let signature = tx_signature();
    assert_eq!(
        submit(app(&db, rpc.clone()), &event, &signature).await,
        StatusCode::OK
    );

    let status = confirm(app(&db, rpc), &event, &signature).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_submitted(&db, &event, &signature).await;
    db.cleanup().await;
}
