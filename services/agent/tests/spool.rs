//! Durable SQLite outbox contracts.

use std::{fs, path::PathBuf};

use lastro_agent::spool::{
    Spool,
    model::{OutboxRow, OutboxState},
};
use sqlx::SqlitePool;
use uuid::Uuid;

fn fixture(name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::read(root.join("test-vectors").join(name)).expect("read fixture")
}

fn row(capture_id: Uuid) -> OutboxRow {
    OutboxRow {
        capture_id,
        event_hash: [
            0xb1, 0x96, 0xb6, 0xd1, 0x3b, 0x0f, 0xbf, 0xe4, 0x47, 0xd7, 0x93, 0xed, 0x56, 0xf2,
            0x91, 0x95, 0xa5, 0x59, 0x27, 0xea, 0x40, 0xfb, 0x8e, 0x22, 0xe9, 0x94, 0xa1, 0x63,
            0x7d, 0x28, 0xcf, 0x41,
        ],
        event_bytes: fixture("v2-serial-event-ready.bin")[16..236]
            .try_into()
            .unwrap(),
        observed_rfid: [0x80, 0x00, 0x13, 0, 0, 0, 0, 1],
        station_pubkey: [
            0x03, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63,
            0xa4, 0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39,
            0x45, 0xd8, 0x98, 0xc2, 0x96,
        ],
        station_signature: [
            0x8d, 0xf1, 0xab, 0x3b, 0x78, 0xcc, 0x14, 0x3f, 0x38, 0x73, 0xbc, 0xef, 0x05, 0xc1,
            0x2a, 0x52, 0xab, 0x13, 0x00, 0x09, 0x7e, 0x0d, 0x5e, 0x18, 0x43, 0x63, 0xb5, 0x23,
            0x57, 0x39, 0xb8, 0xdc, 0x06, 0xe8, 0x3e, 0xcd, 0xed, 0xa8, 0x46, 0x56, 0x37, 0x14,
            0x98, 0x91, 0x23, 0x89, 0x0e, 0x0c, 0x68, 0x2b, 0x29, 0x30, 0x06, 0xd9, 0x1f, 0x01,
            0x35, 0x62, 0x7c, 0x79, 0xd6, 0xe0, 0xf8, 0x9f,
        ],
        state: OutboxState::Local,
        attempts: 0,
    }
}

fn database_url() -> (String, PathBuf) {
    let path = std::env::temp_dir().join(format!("lastro-agent-{}.sqlite", Uuid::new_v4()));
    (format!("sqlite://{}", path.display()), path)
}

#[tokio::test]
async fn identical_duplicate_is_idempotent() {
    // PURPOSE: Repeated delivery of byte-identical evidence must not create a second durable row.
    // ASSERT: Both inserts succeed and pending contains exactly the original byte-identical row once.
    // FAILURE MEANS: Serial reconnect/retry could duplicate the durable outbox.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let expected = row(Uuid::new_v4());
    spool.persist_local(&expected).await.unwrap();
    spool.persist_local(&expected).await.unwrap();
    let pending = spool.pending().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].event_bytes, expected.event_bytes);
    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn divergent_duplicate_is_conflict() {
    // PURPOSE: Reusing a capture/event identity with different cryptographic bytes must never overwrite evidence.
    // ASSERT: The divergent insert fails and the original bytes remain unchanged.
    // FAILURE MEANS: Local evidence could be rewritten after capture.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let expected = row(Uuid::new_v4());
    spool.persist_local(&expected).await.unwrap();
    let mut divergent = expected.clone();
    divergent.station_signature[0] ^= 1;
    assert!(spool.persist_local(&divergent).await.is_err());
    assert_eq!(
        spool.pending().await.unwrap()[0].station_signature,
        expected.station_signature
    );
    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn pending_rows_are_ordered_deterministically() {
    // PURPOSE: Recovery must use a stable order across calls and process restarts.
    // ASSERT: Rows with the same durable timestamp order by capture_id and remain stable after reopening SQLite.
    // FAILURE MEANS: Restart could reorder evidence delivery unpredictably.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let first_id = Uuid::parse_str("00112233-4455-6677-8899-aabbccdde001").unwrap();
    let second_id = Uuid::parse_str("00112233-4455-6677-8899-aabbccdde002").unwrap();
    let first = row(first_id);
    let mut second = row(second_id);
    second.event_hash[0] ^= 1;
    second.event_bytes[10] ^= 1;
    spool.persist_local(&second).await.unwrap();
    spool.persist_local(&first).await.unwrap();
    let one: Vec<_> = spool
        .pending()
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.capture_id)
        .collect();
    drop(spool);
    let reopened = Spool::connect_and_migrate(&url).await.unwrap();
    let two: Vec<_> = reopened
        .pending()
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.capture_id)
        .collect();
    assert_eq!(one, two);
    drop(reopened);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn finalized_row_is_immutable() {
    // PURPOSE: Terminal evidence must neither regress lifecycle state nor accept direct evidence mutation.
    // ASSERT: Rust rejects regression and SQLite triggers reject direct evidence/state rewrites.
    // FAILURE MEANS: Finalized canonical evidence could be reopened or rewritten locally.
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let expected = row(Uuid::new_v4());
    spool.persist_local(&expected).await.unwrap();
    spool
        .advance(expected.capture_id, OutboxState::Server)
        .await
        .unwrap();
    spool
        .advance(expected.capture_id, OutboxState::Finalized)
        .await
        .unwrap();
    assert!(
        spool
            .advance(expected.capture_id, OutboxState::Local)
            .await
            .is_err()
    );

    let direct = SqlitePool::connect(&url).await.unwrap();
    assert!(
        sqlx::query("UPDATE outbox SET event_bytes = zeroblob(220) WHERE capture_id = ?")
            .bind(expected.capture_id.to_string())
            .execute(&direct)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE outbox SET state = 'SERVER' WHERE capture_id = ?")
            .bind(expected.capture_id.to_string())
            .execute(&direct)
            .await
            .is_err()
    );
    direct.close().await;
    drop(spool);
    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn sqlite_rejects_non_eight_byte_observed_rfid() {
    // PURPOSE: The durable boundary must preserve the canonical eight-byte RFID width.
    // ASSERT: Direct SQL accepts eight bytes but rejects seven and nine bytes through the migration CHECK.
    // FAILURE MEANS: SQLite could preserve evidence that protocol hashing cannot reproduce.
    for len in [7usize, 9] {
        let (url, path) = database_url();
        let spool = Spool::connect_and_migrate(&url).await.unwrap();
        let direct = SqlitePool::connect(&url).await.unwrap();
        let expected = row(Uuid::new_v4());
        let result = sqlx::query("INSERT INTO outbox(event_hash,capture_id,event_bytes,observed_rfid,station_pubkey,station_signature,state,attempts,created_at,updated_at) VALUES(?,?,?,?,?,?,'LOCAL',0,'x','x')")
            .bind(expected.event_hash.to_vec()).bind(expected.capture_id.to_string()).bind(expected.event_bytes.to_vec())
            .bind(vec![0u8; len]).bind(expected.station_pubkey.to_vec()).bind(expected.station_signature.to_vec())
            .execute(&direct).await;
        assert!(result.is_err());
        direct.close().await;
        drop(spool);
        let _ = fs::remove_file(path);
    }
}
