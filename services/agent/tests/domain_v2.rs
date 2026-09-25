//! Durable Agent contracts for v2 domain observations.

use std::{fs, path::PathBuf};

use lastro_agent::spool::{
    Spool,
    model::{DomainOutboxRow, DomainOutboxState},
};
use uuid::Uuid;

fn database_url() -> (String, PathBuf) {
    let path = std::env::temp_dir().join(format!("lastro-agent-domain-{}.sqlite", Uuid::new_v4()));
    (format!("sqlite://{}", path.display()), path)
}

fn cleanup(path: &PathBuf) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}-wal", path.display()));
    let _ = fs::remove_file(format!("{}-shm", path.display()));
}

fn row() -> DomainOutboxRow {
    DomainOutboxRow {
        event_hash: [0x11; 32],
        envelope_bytes: [0x22; 220],
        station_pubkey: [0x33; 33],
        station_signature: [0x44; 64],
        state: DomainOutboxState::Local,
        attempts: 0,
    }
}

#[tokio::test]
async fn domain_outbox_is_idempotent_and_monotonic() {
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let expected = row();

    spool.persist_domain_local(&expected).await.unwrap();
    spool.persist_domain_local(&expected).await.unwrap();

    let pending = spool.pending_domain().await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].envelope_bytes, expected.envelope_bytes);
    assert_eq!(pending[0].state, DomainOutboxState::Local);

    spool
        .advance_domain(expected.event_hash, DomainOutboxState::Server)
        .await
        .unwrap();
    assert_eq!(
        spool.pending_domain().await.unwrap()[0].state,
        DomainOutboxState::Server
    );
    assert!(
        spool
            .advance_domain(expected.event_hash, DomainOutboxState::Local)
            .await
            .is_err()
    );

    spool
        .quarantine_domain(expected.event_hash, "terminal test response")
        .await
        .unwrap();
    assert!(spool.pending_domain().await.unwrap().is_empty());

    drop(spool);
    cleanup(&path);
}

#[tokio::test]
async fn divergent_domain_duplicate_is_rejected() {
    let (url, path) = database_url();
    let spool = Spool::connect_and_migrate(&url).await.unwrap();
    let expected = row();
    spool.persist_domain_local(&expected).await.unwrap();

    let mut divergent = expected.clone();
    divergent.envelope_bytes[0] ^= 1;
    assert!(spool.persist_domain_local(&divergent).await.is_err());

    drop(spool);
    cleanup(&path);
}
