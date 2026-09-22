//! SQL access for immutable capture contexts and lifecycle transitions.

use sqlx::{postgres::PgRow, PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureState { Pending, Dispatched, EvidenceAccepted, Expired, Cancelled }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureRecord {
    pub capture_id: Uuid,
    pub station_id: [u8; 32],
    pub action: u8,
    pub animal_id: [u8; 32],
    pub event_sequence: u64,
    pub identity_revision: u32,
    pub expected_old_rfid_hash: [u8; 32],
    pub from_custodian: [u8; 32],
    pub to_custodian: [u8; 32],
    pub previous_event_hash: [u8; 32],
    pub status: CaptureState,
}

#[derive(Clone, Debug)]
pub struct NewCapture {
    pub station_id: [u8; 32],
    pub action: u8,
    pub animal_id: [u8; 32],
    pub event_sequence: u64,
    pub identity_revision: u32,
    pub expected_old_rfid_hash: [u8; 32],
    pub from_custodian: [u8; 32],
    pub to_custodian: [u8; 32],
    pub previous_event_hash: [u8; 32],
}

pub async fn insert_pending(pool: &PgPool, value: &NewCapture) -> Result<CaptureRecord, ApiError> {
    let capture_id = Uuid::new_v4();
    let row = sqlx::query(
        r#"INSERT INTO captures(
             capture_id,station_id,action,animal_id,event_sequence,identity_revision,
             expected_old_rfid_hash,from_custodian,to_custodian,previous_event_hash,status,expires_at)
           VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'PENDING',now()+interval '5 minutes')
           RETURNING *"#,
    )
    .bind(capture_id)
    .bind(value.station_id.to_vec())
    .bind(i16::from(value.action))
    .bind(value.animal_id.to_vec())
    .bind(i64::try_from(value.event_sequence).map_err(|_| ApiError::Validation("event sequence is too large".into()))?)
    .bind(i32::try_from(value.identity_revision).map_err(|_| ApiError::Validation("identity revision is too large".into()))?)
    .bind(value.expected_old_rfid_hash.to_vec())
    .bind(value.from_custodian.to_vec())
    .bind(value.to_custodian.to_vec())
    .bind(value.previous_event_hash.to_vec())
    .fetch_one(pool)
    .await
    .map_err(map_insert_error)?;
    decode_capture(&row)
}

pub async fn claim_next_for_station(
    pool: &PgPool,
    station_id: [u8; 32],
) -> Result<Option<CaptureRecord>, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("UPDATE captures SET status='EXPIRED' WHERE station_id=$1 AND status IN ('PENDING','DISPATCHED') AND expires_at <= now()")
        .bind(station_id.to_vec()).execute(&mut *tx).await.map_err(db_error)?;
    let row = sqlx::query(
        "SELECT * FROM captures WHERE station_id=$1 AND status IN ('DISPATCHED','PENDING') AND expires_at>now() ORDER BY CASE status WHEN 'DISPATCHED' THEN 0 ELSE 1 END,created_at,capture_id FOR UPDATE SKIP LOCKED LIMIT 1",
    )
    .bind(station_id.to_vec())
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?;
    let Some(row) = row else {
        tx.commit().await.map_err(db_error)?;
        return Ok(None);
    };
    let selected = decode_capture(&row)?;
    if selected.status == CaptureState::Dispatched {
        tx.commit().await.map_err(db_error)?;
        return Ok(Some(selected));
    }

    let claimed = sqlx::query("UPDATE captures SET status='DISPATCHED' WHERE capture_id=$1 AND status='PENDING' RETURNING *")
        .bind(selected.capture_id).fetch_one(&mut *tx).await.map_err(db_error)?;
    let result = decode_capture(&claimed)?;
    tx.commit().await.map_err(db_error)?;
    Ok(Some(result))
}

pub async fn find_by_id(pool: &PgPool, capture_id: Uuid) -> Result<Option<CaptureRecord>, ApiError> {
    let row = sqlx::query("SELECT * FROM captures WHERE capture_id=$1")
        .bind(capture_id).fetch_optional(pool).await.map_err(db_error)?;
    row.as_ref().map(decode_capture).transpose()
}

pub async fn load_for_evidence(pool: &PgPool, capture_id: Uuid) -> Result<Option<CaptureRecord>, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("UPDATE captures SET status='EXPIRED' WHERE capture_id=$1 AND status IN ('PENDING','DISPATCHED') AND expires_at <= now()")
        .bind(capture_id).execute(&mut *tx).await.map_err(db_error)?;
    let row = sqlx::query("SELECT * FROM captures WHERE capture_id=$1 FOR UPDATE")
        .bind(capture_id).fetch_optional(&mut *tx).await.map_err(db_error)?;
    let decoded = row.as_ref().map(decode_capture).transpose()?;
    tx.commit().await.map_err(db_error)?;
    Ok(decoded)
}

pub async fn mark_evidence_accepted(
    tx: &mut Transaction<'_, Postgres>,
    capture_id: Uuid,
) -> Result<(), ApiError> {
    let result = sqlx::query("UPDATE captures SET status='EVIDENCE_ACCEPTED' WHERE capture_id=$1 AND status='DISPATCHED' AND expires_at>now()")
        .bind(capture_id).execute(&mut **tx).await.map_err(db_error)?;
    if result.rows_affected() == 1 {
        return Ok(());
    }
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM captures WHERE capture_id=$1")
        .bind(capture_id).fetch_optional(&mut **tx).await.map_err(db_error)?;
    if status.as_deref() == Some("EVIDENCE_ACCEPTED") {
        Ok(())
    } else {
        Err(ApiError::Conflict("capture is not active for evidence admission".into()))
    }
}

fn decode_capture(row: &PgRow) -> Result<CaptureRecord, ApiError> {
    let action: i16 = row.try_get("action").map_err(db_error)?;
    let sequence: i64 = row.try_get("event_sequence").map_err(db_error)?;
    let revision: i32 = row.try_get("identity_revision").map_err(db_error)?;
    let status: String = row.try_get("status").map_err(db_error)?;
    Ok(CaptureRecord {
        capture_id: row.try_get("capture_id").map_err(db_error)?,
        station_id: fixed(row, "station_id")?,
        action: u8::try_from(action).map_err(|_| ApiError::Internal)?,
        animal_id: fixed(row, "animal_id")?,
        event_sequence: u64::try_from(sequence).map_err(|_| ApiError::Internal)?,
        identity_revision: u32::try_from(revision).map_err(|_| ApiError::Internal)?,
        expected_old_rfid_hash: fixed(row, "expected_old_rfid_hash")?,
        from_custodian: fixed(row, "from_custodian")?,
        to_custodian: fixed(row, "to_custodian")?,
        previous_event_hash: fixed(row, "previous_event_hash")?,
        status: parse_status(&status)?,
    })
}

fn fixed<const N: usize>(row: &PgRow, column: &str) -> Result<[u8; N], ApiError> {
    let value: Vec<u8> = row.try_get(column).map_err(db_error)?;
    value.try_into().map_err(|_| ApiError::Internal)
}

fn parse_status(value: &str) -> Result<CaptureState, ApiError> {
    match value {
        "PENDING" => Ok(CaptureState::Pending),
        "DISPATCHED" => Ok(CaptureState::Dispatched),
        "EVIDENCE_ACCEPTED" => Ok(CaptureState::EvidenceAccepted),
        "EXPIRED" => Ok(CaptureState::Expired),
        "CANCELLED" => Ok(CaptureState::Cancelled),
        _ => Err(ApiError::Internal),
    }
}

fn map_insert_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(db) = &error {
        if db.code().as_deref() == Some("23505") {
            return ApiError::Conflict("another capture is already active for this Station".into());
        }
    }
    db_error(error)
}

fn db_error(_error: sqlx::Error) -> ApiError {
    ApiError::Unavailable("postgres query failed".into())
}
