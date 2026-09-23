//! Animal registration and recovery API integration contracts.

mod common;

use std::{fs, path::PathBuf, sync::Arc};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use lastro_api::{
    repository::animals,
    routes,
    solana::rpc::{BindingStatus, CanonicalRfidBinding},
};
use lastro_protocol::StationEvent;
use serde_json::{Value, json};
use tower::ServiceExt;

use common::{STATION_PUBKEY, TestDb, TestRpc, app_state};

const ANIMAL_ID: [u8; 32] = [0x11; 32];
const OLD_RFID_HASH: [u8; 32] = [
    0x8a, 0x60, 0x45, 0x28, 0xf1, 0x90, 0x62, 0xcc, 0x9a, 0xda, 0x87, 0xa5, 0xe2, 0x25, 0xa5, 0xc9,
    0x67, 0xe2, 0xcf, 0x14, 0x8c, 0x85, 0xd8, 0x6d, 0xaf, 0x02, 0xd0, 0x42, 0x12, 0xeb, 0xf6, 0xe1,
];
const NEW_RFID_HASH: [u8; 32] = [
    0x1c, 0x5b, 0xb7, 0x23, 0x58, 0xbd, 0x63, 0x03, 0xc8, 0x6a, 0x49, 0x69, 0xbe, 0xf0, 0x5e, 0xed,
    0x30, 0xf9, 0xa6, 0xb3, 0xda, 0xac, 0x8a, 0xd4, 0x20, 0x7b, 0xc2, 0x7f, 0x4f, 0x40, 0xf9, 0x78,
];

fn fixture_event(name: &str) -> StationEvent {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bytes: [u8; 276] = fs::read(root.join("test-vectors").join(name))
        .expect("read StationEvent fixture")
        .try_into()
        .expect("fixture is exactly 276 bytes");
    StationEvent::decode(&bytes).expect("valid StationEvent fixture")
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

async fn post_animal(app: Router, visual_recovery_id: &str) -> (StatusCode, Value) {
    request_json(
        app,
        Request::builder()
            .method("POST")
            .uri("/api/animals")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({"visualRecoveryId": visual_recovery_id}).to_string(),
            ))
            .unwrap(),
    )
    .await
}

#[tokio::test]
async fn anonymous_registration_stops_at_the_database_backed_minute_budget() {
    // PURPOSE: Anonymous clients must not reserve unbounded PostgreSQL rows.
    // ARRANGE: Simulate 60 registrations within the current minute without network calls.
    // ACTION: Ask the public route for the 61st unoriginated animal.
    // ASSERT: HTTP 429 and no extra row are produced, including after a new Router is built.
    // FAILURE MEANS: An unauthenticated client could exhaust PostgreSQL storage before origin capture.
    let db = TestDb::new().await;
    for index in 0..60u8 {
        let mut animal_id = [0u8; 32];
        animal_id[0] = index + 1;
        animals::insert_registration(&db.pool, animal_id, &format!("BUDGET-{index}"))
            .await
            .unwrap();
    }
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let (status, body) = post_animal(app(&db, rpc), "EXTRA").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["code"], "RATE_LIMITED");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM animals")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 60);
    db.cleanup().await;
}

#[tokio::test]
async fn create_animal_generates_random_32_byte_id() {
    // PURPOSE: Keep AnimalID independent from RFID and unpredictable across registrations.
    // ASSERT: Two registrations return distinct non-zero 32-byte IDs and neither request contains an RFID field.
    // FAILURE MEANS: Logical identity could be derived from a replaceable physical tag or collide predictably.
    let db = TestDb::new().await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));

    let (first_status, first) = post_animal(app(&db, rpc.clone()), "VISUAL-A").await;
    let (second_status, second) = post_animal(app(&db, rpc), "VISUAL-B").await;
    assert_eq!(first_status, StatusCode::CREATED);
    assert_eq!(second_status, StatusCode::CREATED);

    let first_id = first["animalId"].as_str().unwrap();
    let second_id = second["animalId"].as_str().unwrap();
    assert_eq!(first_id.len(), 64);
    assert_eq!(second_id.len(), 64);
    assert_ne!(first_id, "00".repeat(32));
    assert_ne!(second_id, "00".repeat(32));
    assert_ne!(first_id, second_id);
    assert!(first.get("currentRfidHash").unwrap().is_null());
    assert!(second.get("currentRfidHash").unwrap().is_null());

    db.cleanup().await;
}

#[tokio::test]
async fn create_animal_does_not_create_onchain_state() {
    // PURPOSE: Keep off-chain registration distinct from the physical ORIGIN transition.
    // ASSERT: Registration creates only a zero-sequence/zero-revision PostgreSQL row and makes no Solana RPC call.
    // FAILURE MEANS: The backend could represent an animal as originated without Station evidence and wallet authorization.
    let db = TestDb::new().await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let (status, body) = post_animal(app(&db, rpc.clone()), "VISUAL-ORIGIN-LATER").await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(rpc.call_count(), 0);

    let animal_id = hex::decode(body["animalId"].as_str().unwrap()).unwrap();
    let record = animals::find_by_animal_id(&db.pool, animal_id.try_into().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.event_sequence, 0);
    assert_eq!(record.identity_revision, 0);
    assert_eq!(record.current_rfid_hash, None);
    assert_eq!(record.current_custodian, None);
    assert_eq!(record.last_event_hash, None);

    db.cleanup().await;
}

#[tokio::test]
async fn duplicate_visual_recovery_id_returns_conflict() {
    // PURPOSE: Keep visual recovery deterministic within the hackathon deployment.
    // ASSERT: The second identical visual ID returns HTTP 409 and the database retains exactly one matching animal.
    // FAILURE MEANS: Visual recovery could resolve to multiple AnimalIDs.
    let db = TestDb::new().await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    assert_eq!(
        post_animal(app(&db, rpc.clone()), "VISUAL-0042").await.0,
        StatusCode::CREATED
    );
    let (status, _) = post_animal(app(&db, rpc), "VISUAL-0042").await;
    assert_eq!(status, StatusCode::CONFLICT);

    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM animals WHERE visual_recovery_id='VISUAL-0042'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn get_by_current_rfid_returns_only_current_binding() {
    // PURPOSE: Make RFID lookup represent only the canonical ACTIVE binding.
    // ASSERT: A RETIRED RFID returns 404 while the ACTIVE RFID returns the original AnimalID and agrees with local projection.
    // FAILURE MEANS: An old physical identifier could be resurrected as current identity.
    let db = TestDb::new().await;
    animals::insert_registration(&db.pool, ANIMAL_ID, "VISUAL-RFID")
        .await
        .unwrap();
    sqlx::query("UPDATE animals SET current_rfid_hash=$2 WHERE animal_id=$1")
        .bind(ANIMAL_ID.to_vec())
        .bind(NEW_RFID_HASH.to_vec())
        .execute(&db.pool)
        .await
        .unwrap();

    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_binding(CanonicalRfidBinding {
        animal_id: ANIMAL_ID,
        rfid_hash: OLD_RFID_HASH,
        status: BindingStatus::Retired,
    });
    rpc.set_binding(CanonicalRfidBinding {
        animal_id: ANIMAL_ID,
        rfid_hash: NEW_RFID_HASH,
        status: BindingStatus::Active,
    });

    let retired = request_json(
        app(&db, rpc.clone()),
        Request::builder()
            .uri(format!(
                "/api/animals/by-rfid/{}",
                hex::encode(OLD_RFID_HASH)
            ))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(retired.0, StatusCode::NOT_FOUND);

    let active = request_json(
        app(&db, rpc),
        Request::builder()
            .uri(format!(
                "/api/animals/by-rfid/{}",
                hex::encode(NEW_RFID_HASH)
            ))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(active.0, StatusCode::OK);
    assert_eq!(
        active.1["animalId"].as_str(),
        Some(hex::encode(ANIMAL_ID).as_str())
    );
    assert_eq!(
        active.1["currentRfidHash"].as_str(),
        Some(hex::encode(NEW_RFID_HASH).as_str())
    );
    db.cleanup().await;
}

#[tokio::test]
async fn get_unknown_animal_returns_not_found() {
    // PURPOSE: Keep absence deterministic and side-effect free.
    // ASSERT: Reading an unknown AnimalID returns HTTP 404 and does not create a PostgreSQL row.
    // FAILURE MEANS: A read path could mask missing identity or create phantom state.
    let db = TestDb::new().await;
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    let (status, body) = request_json(
        app(&db, rpc),
        Request::builder()
            .uri(format!("/api/animals/{}", "77".repeat(32)))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "NOT_FOUND");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM animals")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    db.cleanup().await;
}

#[tokio::test]
async fn retired_rfid_hash_does_not_resolve_as_current_after_reidentify() {
    // PURPOSE: Keep the PostgreSQL projection aligned with REIDENTIFY semantics across the real projection updater.
    // ASSERT: ORIGIN→TRANSFER→REIDENTIFY preserves AnimalID, increments revision once, retires RFID A lookup, and activates RFID B.
    // FAILURE MEANS: Off-chain recovery could diverge from canonical binding transitions after RFID replacement.
    let db = TestDb::new().await;
    animals::insert_registration(&db.pool, ANIMAL_ID, "VISUAL-REIDENTIFY")
        .await
        .unwrap();

    let origin = fixture_event("origin.bin");
    let transfer = fixture_event("transfer.bin");
    let reidentify = fixture_event("reidentify.bin");
    let mut tx = db.pool.begin().await.unwrap();
    animals::apply_confirmed_state(&mut tx, &origin)
        .await
        .unwrap();
    animals::apply_confirmed_state(&mut tx, &transfer)
        .await
        .unwrap();
    let terminal = animals::apply_confirmed_state(&mut tx, &reidentify)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(terminal.animal_id, ANIMAL_ID);
    assert_eq!(terminal.current_rfid_hash, Some(NEW_RFID_HASH));
    assert_eq!(terminal.identity_revision, 2);
    assert_eq!(terminal.event_sequence, 3);

    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    rpc.set_binding(CanonicalRfidBinding {
        animal_id: ANIMAL_ID,
        rfid_hash: OLD_RFID_HASH,
        status: BindingStatus::Retired,
    });
    rpc.set_binding(CanonicalRfidBinding {
        animal_id: ANIMAL_ID,
        rfid_hash: NEW_RFID_HASH,
        status: BindingStatus::Active,
    });

    let old = request_json(
        app(&db, rpc.clone()),
        Request::builder()
            .uri(format!(
                "/api/animals/by-rfid/{}",
                hex::encode(OLD_RFID_HASH)
            ))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let new = request_json(
        app(&db, rpc),
        Request::builder()
            .uri(format!(
                "/api/animals/by-rfid/{}",
                hex::encode(NEW_RFID_HASH)
            ))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(old.0, StatusCode::NOT_FOUND);
    assert_eq!(new.0, StatusCode::OK);
    assert_eq!(
        new.1["animalId"].as_str(),
        Some(hex::encode(ANIMAL_ID).as_str())
    );
    db.cleanup().await;
}

#[tokio::test]
async fn lookup_by_rfid_never_returns_two_animals() {
    // PURPOSE: Preserve deterministic current-RFID recovery under concurrent projection writes.
    // ASSERT: At most one concurrent update can claim one RFID hash and repository lookup returns at most one row.
    // FAILURE MEANS: The same current physical identifier could resolve to multiple AnimalIDs.
    let db = TestDb::new().await;
    let animal_a = [0x31; 32];
    let animal_b = [0x32; 32];
    animals::insert_registration(&db.pool, animal_a, "CONCURRENT-A")
        .await
        .unwrap();
    animals::insert_registration(&db.pool, animal_b, "CONCURRENT-B")
        .await
        .unwrap();

    let first = sqlx::query("UPDATE animals SET current_rfid_hash=$2 WHERE animal_id=$1")
        .bind(animal_a.to_vec())
        .bind(NEW_RFID_HASH.to_vec())
        .execute(&db.pool);
    let second = sqlx::query("UPDATE animals SET current_rfid_hash=$2 WHERE animal_id=$1")
        .bind(animal_b.to_vec())
        .bind(NEW_RFID_HASH.to_vec())
        .execute(&db.pool);
    let (first, second) = tokio::join!(first, second);
    assert_eq!((first.is_ok() as usize) + (second.is_ok() as usize), 1);

    let record = animals::find_by_current_rfid_hash(&db.pool, NEW_RFID_HASH)
        .await
        .unwrap()
        .expect("one current RFID owner");
    assert!(record.animal_id == animal_a || record.animal_id == animal_b);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM animals WHERE current_rfid_hash=$1")
        .bind(NEW_RFID_HASH.to_vec())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    db.cleanup().await;
}
