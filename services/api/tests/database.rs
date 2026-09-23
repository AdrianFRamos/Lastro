//! PostgreSQL integration contracts for the Lastro API.
//!
//! Every test runs the real SQLx migrations in its own PostgreSQL schema. The CI service provides
//! `LASTRO_DATABASE_URL`; no database behavior is mocked here.

mod common;

use std::{fs, path::PathBuf};

use sqlx::{PgPool, Row};
use uuid::Uuid;

use common::{STATION_PUBKEY, TestDb};

const ANIMAL_A: [u8; 32] = [0x11; 32];
const ANIMAL_B: [u8; 32] = [0x22; 32];
const STATION_ID: [u8; 32] = [0x56; 32];
const RFID_A: [u8; 32] = [0x8a; 32];
const CUSTODIAN_A: [u8; 32] = [0xa1; 32];
const ZERO32: [u8; 32] = [0; 32];
const OBSERVED_RFID: [u8; 8] = [0x80, 0x00, 0x13, 0, 0, 0, 0, 1];
const SIGNATURE64: [u8; 64] = [0x5a; 64];

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name)).expect("read test vector")
}

async fn insert_animal(pool: &PgPool, animal_id: [u8; 32], visual_id: &str) {
    sqlx::query("INSERT INTO animals(animal_id, visual_recovery_id) VALUES($1,$2)")
        .bind(animal_id.to_vec())
        .bind(visual_id)
        .execute(pool)
        .await
        .expect("insert animal fixture");
}

async fn insert_capture(
    pool: &PgPool,
    capture_id: Uuid,
    animal_id: [u8; 32],
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO captures(
             capture_id,station_id,action,animal_id,event_sequence,identity_revision,
             expected_old_rfid_hash,from_custodian,to_custodian,previous_event_hash,status,expires_at)
           VALUES($1,$2,1,$3,1,1,$4,$4,$5,$4,$6,now()+interval '5 minutes')"#,
    )
    .bind(capture_id)
    .bind(STATION_ID.to_vec())
    .bind(animal_id.to_vec())
    .bind(ZERO32.to_vec())
    .bind(CUSTODIAN_A.to_vec())
    .bind(status)
    .execute(pool)
    .await
    .map(|_| ())
}

#[allow(clippy::too_many_arguments)]
async fn insert_event(
    pool: &PgPool,
    capture_id: Uuid,
    event_hash: [u8; 32],
    animal_id: [u8; 32],
    sequence: i64,
    action: i16,
    event_bytes: &[u8],
    observed_rfid: &[u8],
    station_pubkey: &[u8],
    station_signature: &[u8],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO events(
             capture_id,event_hash,animal_id,event_sequence,action,event_bytes,observed_rfid,
             station_pubkey,station_signature,status)
           VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'EVIDENCE_ACCEPTED')"#,
    )
    .bind(capture_id)
    .bind(event_hash.to_vec())
    .bind(animal_id.to_vec())
    .bind(sequence)
    .bind(action)
    .bind(event_bytes)
    .bind(observed_rfid)
    .bind(station_pubkey)
    .bind(station_signature)
    .execute(pool)
    .await
    .map(|_| ())
}

async fn insert_valid_event(pool: &PgPool, animal_id: [u8; 32], sequence: i64) -> (Uuid, [u8; 32]) {
    let capture_id = Uuid::new_v4();
    insert_capture(pool, capture_id, animal_id, "EXPIRED")
        .await
        .expect("insert capture fixture");
    let mut event_hash = [0u8; 32];
    event_hash[..8].copy_from_slice(&(sequence as u64).to_le_bytes());
    event_hash[8] = 0x7e;
    event_hash[9..17].copy_from_slice(&animal_id[..8]);
    insert_event(
        pool,
        capture_id,
        event_hash,
        animal_id,
        sequence,
        1,
        &fixture("origin.bin"),
        &OBSERVED_RFID,
        &STATION_PUBKEY,
        &SIGNATURE64,
    )
    .await
    .expect("insert event fixture");
    (capture_id, event_hash)
}

#[tokio::test]
async fn migration_applies_on_empty_postgres() {
    // PURPOSE: Prove the complete schema can be created from an empty PostgreSQL namespace.
    // ASSERT: Required tables, the active-capture index, and immutable event/capture lifecycle triggers all exist after migrations.
    // FAILURE MEANS: A clean environment cannot reproduce the API persistence contract.
    let db = TestDb::new().await;

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name FROM information_schema.tables WHERE table_schema = current_schema() ORDER BY table_name",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    for required in ["animals", "captures", "events"] {
        assert!(
            tables.iter().any(|table| table == required),
            "missing table {required}"
        );
    }

    let index_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE schemaname=current_schema() AND indexname='captures_one_active_per_station')",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(index_exists);

    let trigger_exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
             SELECT 1 FROM pg_trigger t
             JOIN pg_class c ON c.oid=t.tgrelid
             JOIN pg_namespace n ON n.oid=c.relnamespace
             WHERE n.nspname=current_schema() AND c.relname='events'
               AND t.tgname='events_immutable_evidence' AND NOT t.tgisinternal)"#,
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(trigger_exists);

    for (table_name, trigger_name) in [
        ("captures", "captures_context_lifecycle_guard"),
        ("events", "events_lifecycle_guard"),
        ("events", "events_no_delete"),
    ] {
        let exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(
                 SELECT 1 FROM pg_trigger t
                 JOIN pg_class c ON c.oid=t.tgrelid
                 JOIN pg_namespace n ON n.oid=c.relnamespace
                 WHERE n.nspname=current_schema() AND c.relname=$1
                   AND t.tgname=$2 AND NOT t.tgisinternal)"#,
        )
        .bind(table_name)
        .bind(trigger_name)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert!(exists, "missing trigger {trigger_name}");
    }

    db.cleanup().await;
}

#[tokio::test]
async fn visual_recovery_id_is_unique() {
    // PURPOSE: Keep the demo recovery identifier unambiguous.
    // ASSERT: PostgreSQL accepts the first visual identifier and rejects a second animal with the same exact value.
    // FAILURE MEANS: Physical recovery could resolve one visual identifier to multiple logical identities.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "VISUAL-0042").await;
    let duplicate = sqlx::query("INSERT INTO animals(animal_id,visual_recovery_id) VALUES($1,$2)")
        .bind(ANIMAL_B.to_vec())
        .bind("VISUAL-0042")
        .execute(&db.pool)
        .await;
    assert!(duplicate.is_err());
    db.cleanup().await;
}

#[tokio::test]
async fn current_rfid_hash_is_unique_when_present() {
    // PURPOSE: Preserve unique current-RFID resolution in the PostgreSQL projection.
    // ASSERT: Multiple NULL values are allowed before ORIGIN, but the same non-null RFID hash cannot belong to two animals.
    // FAILURE MEANS: Current RFID lookup could become ambiguous even before consulting canonical Solana state.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "A").await;
    insert_animal(&db.pool, ANIMAL_B, "B").await;

    sqlx::query("UPDATE animals SET current_rfid_hash=$2 WHERE animal_id=$1")
        .bind(ANIMAL_A.to_vec())
        .bind(RFID_A.to_vec())
        .execute(&db.pool)
        .await
        .unwrap();
    let duplicate = sqlx::query("UPDATE animals SET current_rfid_hash=$2 WHERE animal_id=$1")
        .bind(ANIMAL_B.to_vec())
        .bind(RFID_A.to_vec())
        .execute(&db.pool)
        .await;
    assert!(duplicate.is_err());

    let null_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM animals WHERE current_rfid_hash IS NULL")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(null_count, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn event_bytes_requires_exactly_276_bytes() {
    // PURPOSE: Persist only the frozen StationEvent wire length.
    // ASSERT: 275-byte and 277-byte event values fail the CHECK; exactly 276 bytes succeeds.
    // FAILURE MEANS: PostgreSQL could store evidence that no canonical StationEvent decoder can consume.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "A").await;

    for (index, len, expected_ok) in [(1i64, 275usize, false), (2, 276, true), (3, 277, false)] {
        let capture = Uuid::new_v4();
        insert_capture(&db.pool, capture, ANIMAL_A, "EXPIRED")
            .await
            .unwrap();
        let mut hash = [0u8; 32];
        hash[0] = index as u8;
        let result = insert_event(
            &db.pool,
            capture,
            hash,
            ANIMAL_A,
            index,
            1,
            &vec![0u8; len],
            &OBSERVED_RFID,
            &STATION_PUBKEY,
            &SIGNATURE64,
        )
        .await;
        assert_eq!(
            result.is_ok(),
            expected_ok,
            "unexpected result for {len}-byte event"
        );
    }
    db.cleanup().await;
}

#[tokio::test]
async fn event_pubkey_requires_33_bytes() {
    // PURPOSE: Freeze stored Station public keys to compressed SEC1 P-256 shape.
    // ASSERT: Only a 33-byte key passes the database length check; 32-byte and 65-byte values fail.
    // FAILURE MEANS: Evidence transport could persist an ambiguous Station-key encoding.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "A").await;

    for (index, len, expected_ok) in [(1i64, 32usize, false), (2, 33, true), (3, 65, false)] {
        let capture = Uuid::new_v4();
        insert_capture(&db.pool, capture, ANIMAL_A, "EXPIRED")
            .await
            .unwrap();
        let mut hash = [0u8; 32];
        hash[0] = 0x20 + index as u8;
        let result = insert_event(
            &db.pool,
            capture,
            hash,
            ANIMAL_A,
            index,
            1,
            &fixture("origin.bin"),
            &OBSERVED_RFID,
            &vec![0x03; len],
            &SIGNATURE64,
        )
        .await;
        assert_eq!(
            result.is_ok(),
            expected_ok,
            "unexpected result for {len}-byte key"
        );
    }
    db.cleanup().await;
}

#[tokio::test]
async fn event_signature_requires_64_bytes() {
    // PURPOSE: Freeze stored Station signatures to compact P-256 r||s form.
    // ASSERT: Only 64 bytes pass the CHECK; 63-byte and 65-byte signatures fail.
    // FAILURE MEANS: Persistence could disagree with Station/API/verifier signature encoding.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "A").await;

    for (index, len, expected_ok) in [(1i64, 63usize, false), (2, 64, true), (3, 65, false)] {
        let capture = Uuid::new_v4();
        insert_capture(&db.pool, capture, ANIMAL_A, "EXPIRED")
            .await
            .unwrap();
        let mut hash = [0u8; 32];
        hash[0] = 0x30 + index as u8;
        let result = insert_event(
            &db.pool,
            capture,
            hash,
            ANIMAL_A,
            index,
            1,
            &fixture("origin.bin"),
            &OBSERVED_RFID,
            &STATION_PUBKEY,
            &vec![0x44; len],
        )
        .await;
        assert_eq!(
            result.is_ok(),
            expected_ok,
            "unexpected result for {len}-byte signature"
        );
    }
    db.cleanup().await;
}

#[tokio::test]
async fn animal_sequence_is_unique() {
    // PURPOSE: Prevent a local fork at one event-sequence position for the same animal.
    // ASSERT: A second event with the same `(animal_id,event_sequence)` and a different hash is rejected.
    // FAILURE MEANS: PostgreSQL could contain two competing histories for one canonical sequence number.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "A").await;
    let _ = insert_valid_event(&db.pool, ANIMAL_A, 1).await;

    let capture = Uuid::new_v4();
    insert_capture(&db.pool, capture, ANIMAL_A, "EXPIRED")
        .await
        .unwrap();
    let duplicate = insert_event(
        &db.pool,
        capture,
        [0x99; 32],
        ANIMAL_A,
        1,
        1,
        &fixture("origin.bin"),
        &OBSERVED_RFID,
        &STATION_PUBKEY,
        &SIGNATURE64,
    )
    .await;
    assert!(duplicate.is_err());
    db.cleanup().await;
}

#[tokio::test]
async fn only_one_active_capture_per_station() {
    // PURPOSE: Ensure one Station cannot receive concurrent immutable capture contexts.
    // ASSERT: A second PENDING/DISPATCHED capture conflicts; EXPIRED and CANCELLED releases allow a new active capture.
    // FAILURE MEANS: One physical Station could sign evidence against competing capture contexts.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "A").await;

    let first = Uuid::new_v4();
    insert_capture(&db.pool, first, ANIMAL_A, "PENDING")
        .await
        .unwrap();
    assert!(
        insert_capture(&db.pool, Uuid::new_v4(), ANIMAL_A, "DISPATCHED")
            .await
            .is_err()
    );

    sqlx::query("UPDATE captures SET status='EXPIRED' WHERE capture_id=$1")
        .bind(first)
        .execute(&db.pool)
        .await
        .unwrap();
    let second = Uuid::new_v4();
    insert_capture(&db.pool, second, ANIMAL_A, "PENDING")
        .await
        .unwrap();

    sqlx::query("UPDATE captures SET status='CANCELLED' WHERE capture_id=$1")
        .bind(second)
        .execute(&db.pool)
        .await
        .unwrap();
    insert_capture(&db.pool, Uuid::new_v4(), ANIMAL_A, "DISPATCHED")
        .await
        .unwrap();
    db.cleanup().await;
}

#[tokio::test]
async fn capture_context_is_immutable_and_lifecycle_is_monotonic() {
    // PURPOSE: Enforce immutable Station capture context and monotonic capture lifecycle below the Rust repository layer.
    // ASSERT: Context rewrites, expiry extension, skipped transitions, and terminal-state regression fail; valid dispatch/accept transitions succeed.
    // FAILURE MEANS: Direct SQL or an application bug could make signed physical evidence refer to a different capture context or revive stale work.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "A").await;
    insert_animal(&db.pool, ANIMAL_B, "B").await;
    let capture = Uuid::new_v4();
    insert_capture(&db.pool, capture, ANIMAL_A, "PENDING")
        .await
        .unwrap();

    for sql in [
        "UPDATE captures SET station_id=decode(repeat('91',32),'hex') WHERE capture_id=$1",
        "UPDATE captures SET action=2 WHERE capture_id=$1",
        "UPDATE captures SET animal_id=decode(repeat('22',32),'hex') WHERE capture_id=$1",
        "UPDATE captures SET event_sequence=2 WHERE capture_id=$1",
        "UPDATE captures SET identity_revision=2 WHERE capture_id=$1",
        "UPDATE captures SET expected_old_rfid_hash=decode(repeat('33',32),'hex') WHERE capture_id=$1",
        "UPDATE captures SET from_custodian=decode(repeat('44',32),'hex') WHERE capture_id=$1",
        "UPDATE captures SET to_custodian=decode(repeat('55',32),'hex') WHERE capture_id=$1",
        "UPDATE captures SET previous_event_hash=decode(repeat('66',32),'hex') WHERE capture_id=$1",
        "UPDATE captures SET created_at=created_at-interval '1 second' WHERE capture_id=$1",
        "UPDATE captures SET expires_at=expires_at+interval '1 second' WHERE capture_id=$1",
        "UPDATE captures SET status='EVIDENCE_ACCEPTED' WHERE capture_id=$1",
    ] {
        assert!(
            sqlx::query(sql)
                .bind(capture)
                .execute(&db.pool)
                .await
                .is_err(),
            "unexpectedly allowed: {sql}"
        );
    }

    sqlx::query("UPDATE captures SET status='DISPATCHED' WHERE capture_id=$1")
        .bind(capture)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE captures SET expires_at=expires_at-interval '1 second' WHERE capture_id=$1",
    )
    .bind(capture)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE captures SET status='EVIDENCE_ACCEPTED' WHERE capture_id=$1")
        .bind(capture)
        .execute(&db.pool)
        .await
        .unwrap();

    assert!(
        sqlx::query("UPDATE captures SET status='DISPATCHED' WHERE capture_id=$1")
            .bind(capture)
            .execute(&db.pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query(
            "UPDATE captures SET expires_at=expires_at-interval '1 second' WHERE capture_id=$1"
        )
        .bind(capture)
        .execute(&db.pool)
        .await
        .is_err()
    );

    db.cleanup().await;
}

#[tokio::test]
async fn event_evidence_columns_are_immutable() {
    // PURPOSE: Enforce cryptographic evidence immutability below the Rust repository layer.
    // ASSERT: Every cryptographic/domain evidence column rejects mutation and the original row remains byte-identical.
    // FAILURE MEANS: Direct SQL or an application bug could rewrite already accepted physical evidence.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "A").await;
    insert_animal(&db.pool, ANIMAL_B, "B").await;
    let (_, event_hash) = insert_valid_event(&db.pool, ANIMAL_A, 1).await;

    for sql in [
        "UPDATE events SET event_hash=decode(repeat('91',32),'hex') WHERE event_hash=$1",
        "UPDATE events SET animal_id=decode(repeat('22',32),'hex') WHERE event_hash=$1",
        "UPDATE events SET event_sequence=2 WHERE event_hash=$1",
        "UPDATE events SET action=2 WHERE event_hash=$1",
        "UPDATE events SET event_bytes=decode(repeat('00',276),'hex') WHERE event_hash=$1",
        "UPDATE events SET observed_rfid=decode(repeat('aa',8),'hex') WHERE event_hash=$1",
        "UPDATE events SET station_pubkey=decode('02'||repeat('11',32),'hex') WHERE event_hash=$1",
        "UPDATE events SET station_signature=decode(repeat('bb',64),'hex') WHERE event_hash=$1",
    ] {
        assert!(
            sqlx::query(sql)
                .bind(event_hash.to_vec())
                .execute(&db.pool)
                .await
                .is_err(),
            "immutable evidence update unexpectedly succeeded: {sql}"
        );
    }

    let row = sqlx::query("SELECT event_hash,animal_id,event_sequence,action,event_bytes,observed_rfid,station_pubkey,station_signature FROM events WHERE event_hash=$1")
        .bind(event_hash.to_vec())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(row.get::<Vec<u8>, _>("event_hash").as_slice(), event_hash);
    assert_eq!(row.get::<Vec<u8>, _>("animal_id").as_slice(), ANIMAL_A);
    assert_eq!(row.get::<i64, _>("event_sequence"), 1);
    assert_eq!(row.get::<i16, _>("action"), 1);
    assert_eq!(row.get::<Vec<u8>, _>("event_bytes"), fixture("origin.bin"));
    assert_eq!(
        row.get::<Vec<u8>, _>("observed_rfid").as_slice(),
        OBSERVED_RFID
    );
    assert_eq!(
        row.get::<Vec<u8>, _>("station_pubkey").as_slice(),
        STATION_PUBKEY
    );
    assert_eq!(
        row.get::<Vec<u8>, _>("station_signature").as_slice(),
        SIGNATURE64
    );
    db.cleanup().await;
}

#[tokio::test]
async fn event_status_and_tx_signature_may_advance_without_mutating_evidence() {
    // PURPOSE: Permit transaction lifecycle metadata to advance without weakening evidence immutability.
    // ASSERT: SUBMITTED then FINALIZED updates succeed while event bytes and cryptographic columns remain unchanged.
    // FAILURE MEANS: Either legitimate confirmation is blocked or lifecycle code mutates evidence to make it fit.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "A").await;
    let (_, event_hash) = insert_valid_event(&db.pool, ANIMAL_A, 1).await;
    let before: Vec<u8> = sqlx::query_scalar("SELECT event_bytes FROM events WHERE event_hash=$1")
        .bind(event_hash.to_vec())
        .fetch_one(&db.pool)
        .await
        .unwrap();

    sqlx::query("UPDATE events SET status='SUBMITTED',tx_signature='tx-one' WHERE event_hash=$1")
        .bind(event_hash.to_vec())
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE events SET status='FINALIZED' WHERE event_hash=$1 AND tx_signature='tx-one'",
    )
    .bind(event_hash.to_vec())
    .execute(&db.pool)
    .await
    .unwrap();

    let row = sqlx::query("SELECT status,tx_signature,event_bytes FROM events WHERE event_hash=$1")
        .bind(event_hash.to_vec())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("status"), "FINALIZED");
    assert_eq!(
        row.get::<Option<String>, _>("tx_signature").as_deref(),
        Some("tx-one")
    );
    assert_eq!(row.get::<Vec<u8>, _>("event_bytes"), before);
    db.cleanup().await;
}

#[tokio::test]
async fn event_lifecycle_is_monotonic_and_transaction_signature_is_stable() {
    // PURPOSE: Enforce the confirmed -> submitted -> finalized lifecycle below the Rust route layer.
    // ARRANGE: Persist one accepted immutable event with no transaction signature.
    // ACTION: Attempt to skip SUBMITTED, change a stored signature, regress status, and delete history.
    // ASSERT: Only EVIDENCE_ACCEPTED -> SUBMITTED -> FINALIZED with one stable signature succeeds; deletion fails.
    // FAILURE MEANS: Direct SQL or a code defect could bypass RPC commitment staging or rewrite canonical history metadata.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "A").await;
    let (_, event_hash) = insert_valid_event(&db.pool, ANIMAL_A, 1).await;

    let skip = sqlx::query(
        "UPDATE events SET status='FINALIZED',tx_signature='tx-one' WHERE event_hash=$1",
    )
    .bind(event_hash.to_vec())
    .execute(&db.pool)
    .await;
    assert!(skip.is_err());

    sqlx::query("UPDATE events SET status='SUBMITTED',tx_signature='tx-one' WHERE event_hash=$1")
        .bind(event_hash.to_vec())
        .execute(&db.pool)
        .await
        .unwrap();

    let replace_signature =
        sqlx::query("UPDATE events SET tx_signature='tx-two' WHERE event_hash=$1")
            .bind(event_hash.to_vec())
            .execute(&db.pool)
            .await;
    assert!(replace_signature.is_err());

    sqlx::query("UPDATE events SET status='FINALIZED' WHERE event_hash=$1")
        .bind(event_hash.to_vec())
        .execute(&db.pool)
        .await
        .unwrap();

    let regress = sqlx::query("UPDATE events SET status='SUBMITTED' WHERE event_hash=$1")
        .bind(event_hash.to_vec())
        .execute(&db.pool)
        .await;
    assert!(regress.is_err());

    let delete = sqlx::query("DELETE FROM events WHERE event_hash=$1")
        .bind(event_hash.to_vec())
        .execute(&db.pool)
        .await;
    assert!(delete.is_err());

    let row = sqlx::query("SELECT status,tx_signature FROM events WHERE event_hash=$1")
        .bind(event_hash.to_vec())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("status"), "FINALIZED");
    assert_eq!(
        row.get::<Option<String>, _>("tx_signature").as_deref(),
        Some("tx-one")
    );
    db.cleanup().await;
}

#[tokio::test]
async fn submitted_and_finalized_rows_require_a_transaction_signature() {
    // PURPOSE: Prevent lifecycle rows from claiming Solana submission/finality without a transaction identifier.
    // ARRANGE: Persist one accepted event under an existing animal and capture.
    // ACTION: Attempt direct status updates to SUBMITTED and FINALIZED without setting tx_signature.
    // ASSERT: PostgreSQL rejects both inconsistent lifecycle states and leaves the event EVIDENCE_ACCEPTED.
    // FAILURE MEANS: Recovery code could observe a terminal-looking event that has no transaction to verify independently.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "A").await;
    let (_, event_hash) = insert_valid_event(&db.pool, ANIMAL_A, 1).await;

    for status in ["SUBMITTED", "FINALIZED"] {
        let result = sqlx::query("UPDATE events SET status=$2 WHERE event_hash=$1")
            .bind(event_hash.to_vec())
            .bind(status)
            .execute(&db.pool)
            .await;
        assert!(
            result.is_err(),
            "status {status} unexpectedly accepted without tx_signature"
        );
    }

    let status: String = sqlx::query_scalar("SELECT status FROM events WHERE event_hash=$1")
        .bind(event_hash.to_vec())
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(status, "EVIDENCE_ACCEPTED");
    db.cleanup().await;
}

#[tokio::test]
async fn events_and_captures_require_existing_animal_row() {
    // PURPOSE: Prevent capture/evidence rows from referring to an AnimalID that was never registered off-chain.
    // ASSERT: Both foreign-key paths reject an unknown animal and accept it only after the animal row exists.
    // FAILURE MEANS: API persistence could contain operational evidence with no resolvable identity record.
    let db = TestDb::new().await;
    let capture = Uuid::new_v4();
    assert!(
        insert_capture(&db.pool, capture, ANIMAL_A, "EXPIRED")
            .await
            .is_err()
    );

    insert_animal(&db.pool, ANIMAL_B, "B").await;
    let valid_capture = Uuid::new_v4();
    insert_capture(&db.pool, valid_capture, ANIMAL_B, "EXPIRED")
        .await
        .unwrap();
    let orphan_event = insert_event(
        &db.pool,
        valid_capture,
        [0x88; 32],
        ANIMAL_A,
        1,
        1,
        &fixture("origin.bin"),
        &OBSERVED_RFID,
        &STATION_PUBKEY,
        &SIGNATURE64,
    )
    .await;
    assert!(orphan_event.is_err());

    insert_animal(&db.pool, ANIMAL_A, "A").await;
    insert_capture(&db.pool, capture, ANIMAL_A, "EXPIRED")
        .await
        .unwrap();
    insert_event(
        &db.pool,
        capture,
        [0x89; 32],
        ANIMAL_A,
        1,
        1,
        &fixture("origin.bin"),
        &OBSERVED_RFID,
        &STATION_PUBKEY,
        &SIGNATURE64,
    )
    .await
    .unwrap();
    db.cleanup().await;
}

#[tokio::test]
async fn finalized_transaction_signature_cannot_be_attached_to_two_events() {
    // PURPOSE: Bind one Solana transaction signature to at most one immutable Lastro event row.
    // ASSERT: Reusing the same non-null transaction signature on a second distinct event violates the unique constraint.
    // FAILURE MEANS: The local projection could claim one transaction finalized two different events.
    let db = TestDb::new().await;
    insert_animal(&db.pool, ANIMAL_A, "A").await;
    insert_animal(&db.pool, ANIMAL_B, "B").await;
    let (_, first_hash) = insert_valid_event(&db.pool, ANIMAL_A, 1).await;
    let (_, second_hash) = insert_valid_event(&db.pool, ANIMAL_B, 1).await;

    sqlx::query("UPDATE events SET tx_signature='same-finalized-signature',status='SUBMITTED' WHERE event_hash=$1")
        .bind(first_hash.to_vec())
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE events SET status='FINALIZED' WHERE event_hash=$1")
        .bind(first_hash.to_vec())
        .execute(&db.pool)
        .await
        .unwrap();
    let duplicate = sqlx::query("UPDATE events SET tx_signature='same-finalized-signature',status='SUBMITTED' WHERE event_hash=$1")
        .bind(second_hash.to_vec())
        .execute(&db.pool)
        .await;
    assert!(duplicate.is_err());

    let first_signature: Option<String> =
        sqlx::query_scalar("SELECT tx_signature FROM events WHERE event_hash=$1")
            .bind(first_hash.to_vec())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    let second_signature: Option<String> =
        sqlx::query_scalar("SELECT tx_signature FROM events WHERE event_hash=$1")
            .bind(second_hash.to_vec())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(first_signature.as_deref(), Some("same-finalized-signature"));
    assert_eq!(second_signature, None);
    db.cleanup().await;
}
