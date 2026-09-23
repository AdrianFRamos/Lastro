//! EvidencePackage export integration contracts.

mod common;

use std::{fs, path::PathBuf, sync::Arc};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use lastro_api::{repository::animals, routes};
use lastro_protocol::{StationEvent, evidence::EvidencePackage};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use common::{DEPLOYMENT_ID, STATION_PUBKEY, TestDb, TestRpc, app_state};

const ANIMAL_ID: [u8; 32] = [0x11; 32];
const RFID_A: [u8; 8] = [0x80, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x01];
const RFID_B: [u8; 8] = [0x80, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x02];
const ORIGIN_SIG: &str = "09579de39fbd8108dea2fc89262240c45b2043d85e6e16854d9bed737e6426e109be548b3eb9d3ba3252aeba61af230a18115aeaf0b74734df88c90955f9184f";
const TRANSFER_SIG: &str = "6cd31b25d410d4ad14d1d30b5a9f6203d7f165e5a0f6f81c847dd85cff72f17a46a2d1b94a0f70d93ff6fff812a4b928a5dd8e71b263994398d975bc81ca5f5b";
const REIDENTIFY_SIG: &str = "8774aeea93911a6527094615709947bf259df02e79b189c9f6cc99c3da1bfe6677fda980f132740d923ea489c8f77de2a2b2abd9b368e3442b2e0d3f8cf4a573";
const TRANSFER_BC_SIG: &str = "74ce09f94984d06cfac65557ed1165bf0902ab94c7fbd57716dff155a12ec74e12212881ea549984f75bc17e2e4c15cf965086f858c1e99b3ea77f868baa5f45";

fn fixture(name: &str) -> [u8; 276] {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name))
        .expect("read fixture")
        .try_into()
        .expect("StationEvent fixture is exactly 276 bytes")
}

fn app(db: &TestDb) -> Router {
    let rpc = Arc::new(TestRpc::with_station_pubkey(STATION_PUBKEY));
    routes::router(app_state(db.pool.clone(), rpc))
}

fn transaction_signature(index: u8) -> String {
    bs58::encode([index; 64]).into_string()
}

async fn insert_finalized(
    db: &TestDb,
    fixture_name: &str,
    observed_rfid: [u8; 8],
    signature_hex: &str,
    tx_index: u8,
) {
    let event_bytes = fixture(fixture_name);
    let event = StationEvent::decode(&event_bytes).expect("decode event fixture");
    let capture_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO captures(
             capture_id,station_id,action,animal_id,event_sequence,identity_revision,
             expected_old_rfid_hash,from_custodian,to_custodian,previous_event_hash,status,expires_at)
           VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'EVIDENCE_ACCEPTED',now()+interval '5 minutes')"#,
    )
    .bind(capture_id)
    .bind(event.station_id.to_vec())
    .bind(i16::from(event.action as u8))
    .bind(event.animal_id.to_vec())
    .bind(i64::try_from(event.event_sequence).unwrap())
    .bind(i32::try_from(event.identity_revision).unwrap())
    .bind(event.old_rfid_hash.to_vec())
    .bind(event.from_custodian.to_vec())
    .bind(event.to_custodian.to_vec())
    .bind(event.previous_event_hash.to_vec())
    .execute(&db.pool)
    .await
    .expect("insert evidence capture");

    sqlx::query(
        r#"INSERT INTO events(
             capture_id,event_hash,animal_id,event_sequence,action,event_bytes,observed_rfid,
             station_pubkey,station_signature,tx_signature,status)
           VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'FINALIZED')"#,
    )
    .bind(capture_id)
    .bind(event.event_hash().to_vec())
    .bind(event.animal_id.to_vec())
    .bind(i64::try_from(event.event_sequence).unwrap())
    .bind(i16::from(event.action as u8))
    .bind(event_bytes.to_vec())
    .bind(observed_rfid.to_vec())
    .bind(STATION_PUBKEY.to_vec())
    .bind(hex::decode(signature_hex).unwrap())
    .bind(transaction_signature(tx_index))
    .execute(&db.pool)
    .await
    .expect("insert finalized event");
}

async fn insert_rejected_sequence_two_attempt(db: &TestDb) -> [u8; 32] {
    let mut event =
        StationEvent::decode(&fixture("transfer.bin")).expect("decode transfer fixture");
    event.to_custodian = [0x44; 32];
    let event_bytes = event.encode();
    let event_hash = event.event_hash();
    let capture_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO captures(
             capture_id,station_id,action,animal_id,event_sequence,identity_revision,
             expected_old_rfid_hash,from_custodian,to_custodian,previous_event_hash,status,expires_at)
           VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'CANCELLED',now()+interval '5 minutes')"#,
    )
    .bind(capture_id)
    .bind(event.station_id.to_vec())
    .bind(i16::from(event.action as u8))
    .bind(event.animal_id.to_vec())
    .bind(i64::try_from(event.event_sequence).unwrap())
    .bind(i32::try_from(event.identity_revision).unwrap())
    .bind(event.old_rfid_hash.to_vec())
    .bind(event.from_custodian.to_vec())
    .bind(event.to_custodian.to_vec())
    .bind(event.previous_event_hash.to_vec())
    .execute(&db.pool)
    .await
    .expect("insert rejected capture");

    sqlx::query(
        r#"INSERT INTO events(
             capture_id,event_hash,animal_id,event_sequence,action,event_bytes,observed_rfid,
             station_pubkey,station_signature,tx_signature,status)
           VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,NULL,'REJECTED')"#,
    )
    .bind(capture_id)
    .bind(event_hash.to_vec())
    .bind(event.animal_id.to_vec())
    .bind(i64::try_from(event.event_sequence).unwrap())
    .bind(i16::from(event.action as u8))
    .bind(event_bytes.to_vec())
    .bind(RFID_A.to_vec())
    .bind(STATION_PUBKEY.to_vec())
    .bind(vec![0x55u8; 64])
    .execute(&db.pool)
    .await
    .expect("insert rejected event");
    event_hash
}

async fn seed_full_history(db: &TestDb, insertion_order: &[u8]) {
    animals::insert_registration(&db.pool, ANIMAL_ID, "VISUAL-EVIDENCE-PACKAGE")
        .await
        .expect("register evidence animal");
    for sequence in insertion_order {
        match sequence {
            1 => insert_finalized(db, "origin.bin", RFID_A, ORIGIN_SIG, 1).await,
            2 => insert_finalized(db, "transfer.bin", RFID_A, TRANSFER_SIG, 2).await,
            3 => insert_finalized(db, "reidentify.bin", RFID_B, REIDENTIFY_SIG, 3).await,
            4 => insert_finalized(db, "transfer-b-to-c.bin", RFID_B, TRANSFER_BC_SIG, 4).await,
            _ => panic!("unsupported fixture sequence"),
        }
    }
}

async fn fetch_package(db: &TestDb) -> (StatusCode, Value) {
    let response = app(db)
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/animals/{}/evidence-package",
                    hex::encode(ANIMAL_ID)
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("evidence package response");
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read package body");
    (status, serde_json::from_slice(&body).expect("package JSON"))
}

#[tokio::test]
async fn evidence_package_orders_events_by_sequence() {
    // PURPOSE: Make exported history deterministic regardless of physical row insertion order.
    // ASSERT: Rows inserted as 4,2,1,3 export exactly as StationEvent sequences 1,2,3,4 with no omissions or duplicates.
    // FAILURE MEANS: A verifier could mistake database query order for canonical event history.
    let db = TestDb::new().await;
    seed_full_history(&db, &[4, 2, 1, 3]).await;
    let (status, body) = fetch_package(&db).await;
    assert_eq!(status, StatusCode::OK);
    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 4);
    let sequences: Vec<u64> = events
        .iter()
        .map(|item| {
            let raw = BASE64
                .decode(item["eventBytesBase64"].as_str().unwrap())
                .unwrap();
            StationEvent::decode(&raw).unwrap().event_sequence
        })
        .collect();
    assert_eq!(sequences, vec![1, 2, 3, 4]);
    db.cleanup().await;
}

#[tokio::test]
async fn evidence_package_ignores_rejected_superseded_attempts() {
    // PURPOSE: Preserve rejected physical evidence for audit without polluting canonical finalized history.
    // ARRANGE: Store a complete finalized 1..4 history plus a rejected alternative event at sequence 2.
    // ACTION: Export the public EvidencePackage.
    // ASSERT: Export remains a contiguous four-event finalized history and excludes the rejected event hash/bytes.
    // FAILURE MEANS: safe supersession would either break public verification or require deleting immutable rejected evidence.
    let db = TestDb::new().await;
    seed_full_history(&db, &[1, 2, 3, 4]).await;
    let rejected_hash = insert_rejected_sequence_two_attempt(&db).await;

    let (status, body) = fetch_package(&db).await;
    assert_eq!(status, StatusCode::OK);
    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 4);

    for item in events {
        let raw = BASE64
            .decode(item["eventBytesBase64"].as_str().unwrap())
            .unwrap();
        let event = StationEvent::decode(&raw).unwrap();
        assert_ne!(event.event_hash(), rejected_hash);
    }

    let rejected_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM events WHERE status='REJECTED'")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(rejected_rows, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn evidence_package_contains_original_event_bytes() {
    // PURPOSE: Export immutable signed StationEvent bytes rather than reconstructing them from projection columns.
    // ASSERT: Every base64 payload decodes byte-for-byte to its committed 276-byte fixture.
    // FAILURE MEANS: The API could silently change the exact message that the Station signed.
    let db = TestDb::new().await;
    seed_full_history(&db, &[1, 2, 3, 4]).await;
    let (status, body) = fetch_package(&db).await;
    assert_eq!(status, StatusCode::OK);
    for (item, fixture_name) in body["events"].as_array().unwrap().iter().zip([
        "origin.bin",
        "transfer.bin",
        "reidentify.bin",
        "transfer-b-to-c.bin",
    ]) {
        let decoded = BASE64
            .decode(item["eventBytesBase64"].as_str().unwrap())
            .unwrap();
        assert_eq!(decoded, fixture(fixture_name));
    }
    db.cleanup().await;
}

#[tokio::test]
async fn evidence_package_contains_observed_rfid_pubkey_signature_tx() {
    // PURPOSE: Give an independent verifier every original input needed for local cryptographic checks plus the on-chain reference.
    // ASSERT: RFID, 33-byte key, 64-byte signature, and transaction signature are exported exactly for every finalized event.
    // FAILURE MEANS: Third parties could not recompute physical/hash/signature bindings or locate the canonical transaction.
    let db = TestDb::new().await;
    seed_full_history(&db, &[1, 2, 3, 4]).await;
    let (_, body) = fetch_package(&db).await;
    let expected = [
        (RFID_A, ORIGIN_SIG, transaction_signature(1)),
        (RFID_A, TRANSFER_SIG, transaction_signature(2)),
        (RFID_B, REIDENTIFY_SIG, transaction_signature(3)),
        (RFID_B, TRANSFER_BC_SIG, transaction_signature(4)),
    ];
    for (item, (rfid, signature, tx_signature)) in
        body["events"].as_array().unwrap().iter().zip(expected)
    {
        assert_eq!(
            item["observedRfidHex"].as_str(),
            Some(hex::encode(rfid).as_str())
        );
        assert_eq!(
            item["stationPubkeyHex"].as_str(),
            Some(hex::encode(STATION_PUBKEY).as_str())
        );
        assert_eq!(item["stationSignatureHex"].as_str(), Some(signature));
        assert_eq!(item["txSignature"].as_str(), Some(tx_signature.as_str()));
    }
    db.cleanup().await;
}

#[tokio::test]
async fn evidence_package_excludes_backend_valid_boolean() {
    // PURPOSE: Keep the API from becoming a trust oracle for the independent verifier.
    // ASSERT: Export contains evidence only and has no valid, complianceScore, backendVerdict, or equivalent verdict field.
    // FAILURE MEANS: Consumers could accidentally trust the backend's conclusion instead of recomputing validity.
    let db = TestDb::new().await;
    seed_full_history(&db, &[1, 2, 3, 4]).await;
    let (_, body) = fetch_package(&db).await;
    let object = body.as_object().unwrap();
    assert_eq!(
        object
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "animalId".to_string(),
            "deploymentId".to_string(),
            "events".to_string(),
            "version".to_string(),
        ]
        .into_iter()
        .collect()
    );
    let serialized = serde_json::to_string(&body).unwrap();
    assert!(!serialized.contains("backendVerdict"));
    assert!(!serialized.contains("complianceScore"));
    assert!(!serialized.contains("\"valid\""));
    db.cleanup().await;
}

#[tokio::test]
async fn evidence_package_matches_schema_contract_and_strict_serde() {
    // PURPOSE: Keep API wire JSON aligned with the frozen EvidencePackage schema and strict shared transport type.
    // ASSERT: Real API output has version 1, exact top-level/event fields and formats, round-trips through deny_unknown_fields, and rejects extras.
    // FAILURE MEANS: API export and independent verifier could evolve incompatibly despite sharing the same protocol name.
    let db = TestDb::new().await;
    seed_full_history(&db, &[1, 2, 3, 4]).await;
    let (_, body) = fetch_package(&db).await;
    assert_eq!(body["version"], 1);
    assert_eq!(
        body["deploymentId"].as_str(),
        Some(hex::encode(DEPLOYMENT_ID).as_str())
    );
    assert_eq!(
        body["animalId"].as_str(),
        Some(hex::encode(ANIMAL_ID).as_str())
    );
    assert_eq!(body["deploymentId"].as_str().unwrap().len(), 64);
    assert_eq!(body["animalId"].as_str().unwrap().len(), 64);
    for item in body["events"].as_array().unwrap() {
        let keys: std::collections::BTreeSet<_> = item
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "eventBytesBase64",
                "observedRfidHex",
                "stationPubkeyHex",
                "stationSignatureHex",
                "txSignature",
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            BASE64
                .decode(item["eventBytesBase64"].as_str().unwrap())
                .unwrap()
                .len(),
            276
        );
        assert_eq!(item["observedRfidHex"].as_str().unwrap().len(), 16);
        assert_eq!(item["stationPubkeyHex"].as_str().unwrap().len(), 66);
        assert_eq!(item["stationSignatureHex"].as_str().unwrap().len(), 128);
        assert!(item["txSignature"].as_str().is_some());
    }

    let package: EvidencePackage =
        serde_json::from_value(body.clone()).expect("strict EvidencePackage decode");
    assert_eq!(serde_json::to_value(&package).unwrap(), body);
    package
        .validate_off_chain_chain()
        .expect("real API package validates off-chain");

    let mut with_extra = body;
    with_extra
        .as_object_mut()
        .unwrap()
        .insert("valid".into(), Value::Bool(true));
    assert!(serde_json::from_value::<EvidencePackage>(with_extra).is_err());
    db.cleanup().await;
}
