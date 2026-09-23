//! Append-only cryptographic event persistence.

use lastro_protocol::StationEvent;
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;

use crate::{crypto::VerifiedEvidence, error::ApiError};

#[derive(Clone, Debug)]
pub struct EventRecord {
    pub capture_id: Uuid,
    pub event_hash: [u8; 32],
    pub event: StationEvent,
    pub event_bytes: [u8; 276],
    pub observed_rfid: [u8; 8],
    pub station_pubkey: [u8; 33],
    pub station_signature: [u8; 64],
    pub tx_signature: Option<String>,
    pub status: String,
}

/// Returns true when a new row was inserted and false for an exact idempotent duplicate.
pub async fn insert_or_match_exact(
    tx: &mut Transaction<'_, Postgres>,
    capture_id: Uuid,
    evidence: &VerifiedEvidence,
) -> Result<bool, ApiError> {
    let result = sqlx::query(
        r#"INSERT INTO events(capture_id,event_hash,animal_id,event_sequence,action,event_bytes,
             observed_rfid,station_pubkey,station_signature,status)
           VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'EVIDENCE_ACCEPTED')
           ON CONFLICT DO NOTHING"#,
    )
    .bind(capture_id)
    .bind(evidence.event_hash.to_vec())
    .bind(evidence.event.animal_id.to_vec())
    .bind(i64::try_from(evidence.event.event_sequence).map_err(|_| ApiError::Internal)?)
    .bind(i16::from(evidence.event.action as u8))
    .bind(evidence.event_bytes.to_vec())
    .bind(evidence.observed_rfid.to_vec())
    .bind(evidence.station_pubkey33.to_vec())
    .bind(evidence.station_signature64.to_vec())
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    if result.rows_affected() == 1 {
        return Ok(true);
    }

    let rows = sqlx::query(
        "SELECT * FROM events WHERE capture_id=$1 OR event_hash=$2 OR (animal_id=$3 AND event_sequence=$4)",
    )
    .bind(capture_id)
    .bind(evidence.event_hash.to_vec())
    .bind(evidence.event.animal_id.to_vec())
    .bind(i64::try_from(evidence.event.event_sequence).map_err(|_| ApiError::Internal)?)
    .fetch_all(&mut **tx)
    .await
    .map_err(db_error)?;
    if rows.len() != 1 {
        return Err(ApiError::Conflict(
            "event uniqueness collision is not an exact duplicate".into(),
        ));
    }
    let existing = decode_event(&rows[0])?;
    let exact = existing.capture_id == capture_id
        && existing.event_hash == evidence.event_hash
        && existing.event_bytes == evidence.event_bytes
        && existing.observed_rfid == evidence.observed_rfid
        && existing.station_pubkey == evidence.station_pubkey33
        && existing.station_signature == evidence.station_signature64;
    if exact {
        Ok(false)
    } else {
        Err(ApiError::Conflict(
            "event duplicate contains divergent evidence".into(),
        ))
    }
}

pub async fn find_by_hash(
    pool: &PgPool,
    event_hash: [u8; 32],
) -> Result<Option<EventRecord>, ApiError> {
    let row = sqlx::query("SELECT * FROM events WHERE event_hash=$1")
        .bind(event_hash.to_vec())
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    row.as_ref().map(decode_event).transpose()
}

pub async fn find_by_capture_id(
    pool: &PgPool,
    capture_id: Uuid,
) -> Result<Option<EventRecord>, ApiError> {
    let row = sqlx::query("SELECT * FROM events WHERE capture_id=$1")
        .bind(capture_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    row.as_ref().map(decode_event).transpose()
}

/// Returns the single accepted/submitted transition that still blocks a new canonical predecessor.
pub async fn find_unfinalized_for_animal(
    pool: &PgPool,
    animal_id: [u8; 32],
) -> Result<Option<EventRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT * FROM events WHERE animal_id=$1 AND status IN ('EVIDENCE_ACCEPTED','SUBMITTED') ORDER BY event_sequence ASC LIMIT 1",
    )
    .bind(animal_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_event).transpose()
}

pub async fn mark_rejected_tx(
    tx: &mut Transaction<'_, Postgres>,
    event_hash: [u8; 32],
) -> Result<(), ApiError> {
    let result = sqlx::query(
        "UPDATE events SET status='REJECTED' WHERE event_hash=$1 AND status='EVIDENCE_ACCEPTED' AND tx_signature IS NULL",
    )
    .bind(event_hash.to_vec())
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    if result.rows_affected() == 1 {
        Ok(())
    } else {
        Err(ApiError::Conflict(
            "accepted event cannot be superseded after transaction submission".into(),
        ))
    }
}

pub async fn require_active_event_tx(
    tx: &mut Transaction<'_, Postgres>,
    event_hash: [u8; 32],
) -> Result<(), ApiError> {
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM events WHERE event_hash=$1 FOR UPDATE")
            .bind(event_hash.to_vec())
            .fetch_optional(&mut **tx)
            .await
            .map_err(db_error)?;
    if matches!(status.as_deref(), Some("EVIDENCE_ACCEPTED" | "SUBMITTED")) {
        Ok(())
    } else {
        Err(ApiError::Conflict(
            "previously accepted evidence changed while authorizing this capture".into(),
        ))
    }
}

pub async fn mark_submitted(
    pool: &PgPool,
    event_hash: [u8; 32],
    tx_signature: &str,
) -> Result<(), ApiError> {
    let result = sqlx::query(
        "UPDATE events SET tx_signature=$2,status='SUBMITTED' WHERE event_hash=$1 AND status IN ('EVIDENCE_ACCEPTED','SUBMITTED') AND (tx_signature IS NULL OR tx_signature=$2)",
    )
    .bind(event_hash.to_vec()).bind(tx_signature).execute(pool).await.map_err(db_error)?;
    if result.rows_affected() == 1 {
        Ok(())
    } else {
        Err(ApiError::Conflict(
            "event cannot be marked submitted".into(),
        ))
    }
}

pub async fn mark_finalized(
    tx: &mut Transaction<'_, Postgres>,
    event_hash: [u8; 32],
    tx_signature: &str,
) -> Result<(), ApiError> {
    let result = sqlx::query(
        "UPDATE events SET tx_signature=$2,status='FINALIZED' WHERE event_hash=$1 AND status IN ('SUBMITTED','FINALIZED') AND (tx_signature IS NULL OR tx_signature=$2)",
    )
    .bind(event_hash.to_vec()).bind(tx_signature).execute(&mut **tx).await.map_err(db_error)?;
    if result.rows_affected() == 1 {
        Ok(())
    } else {
        Err(ApiError::Conflict(
            "event cannot be finalized with this transaction signature".into(),
        ))
    }
}

pub async fn list_finalized_for_animal(
    pool: &PgPool,
    animal_id: [u8; 32],
) -> Result<Vec<EventRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT * FROM events WHERE animal_id=$1 AND status='FINALIZED' ORDER BY event_sequence ASC LIMIT 129",
    )
    .bind(animal_id.to_vec())
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    rows.iter().map(decode_event).collect()
}

pub async fn list_for_animal(
    pool: &PgPool,
    animal_id: [u8; 32],
) -> Result<Vec<EventRecord>, ApiError> {
    let rows = sqlx::query("SELECT * FROM events WHERE animal_id=$1 ORDER BY event_sequence ASC")
        .bind(animal_id.to_vec())
        .fetch_all(pool)
        .await
        .map_err(db_error)?;
    rows.iter().map(decode_event).collect()
}

fn decode_event(row: &PgRow) -> Result<EventRecord, ApiError> {
    let event_bytes: [u8; 276] = fixed(row, "event_bytes")?;
    let event = StationEvent::decode(&event_bytes).map_err(|_| ApiError::Internal)?;
    Ok(EventRecord {
        capture_id: row.try_get("capture_id").map_err(db_error)?,
        event_hash: fixed(row, "event_hash")?,
        event,
        event_bytes,
        observed_rfid: fixed(row, "observed_rfid")?,
        station_pubkey: fixed(row, "station_pubkey")?,
        station_signature: fixed(row, "station_signature")?,
        tx_signature: row.try_get("tx_signature").map_err(db_error)?,
        status: row.try_get("status").map_err(db_error)?,
    })
}

fn fixed<const N: usize>(row: &PgRow, column: &str) -> Result<[u8; N], ApiError> {
    let value: Vec<u8> = row.try_get(column).map_err(db_error)?;
    value.try_into().map_err(|_| ApiError::Internal)
}

fn db_error(_error: sqlx::Error) -> ApiError {
    ApiError::Unavailable("postgres query failed".into())
}
