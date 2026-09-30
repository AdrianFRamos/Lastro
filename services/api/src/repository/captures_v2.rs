//! Persistence for v2 physical captures and their wallet authorization challenges.

use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureStatus {
    Pending,
    Dispatched,
    EvidenceAccepted,
    Expired,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct CaptureRecord {
    pub capture_id: Uuid,
    pub deployment_id: [u8; 32],
    pub station_id: [u8; 32],
    pub asset_id: [u8; 32],
    pub event_type: u16,
    pub event_id: [u8; 32],
    pub state_version: u64,
    pub previous_event_hash: [u8; 32],
    pub expected_rfid_hash: [u8; 32],
    pub required_signer: [u8; 32],
    pub observed_at: i64,
    pub expires_at: i64,
    pub status: CaptureStatus,
    pub event_hash: Option<[u8; 32]>,
}

#[derive(Clone, Debug)]
pub struct ChallengeRecord {
    pub challenge_id: Uuid,
    pub deployment_id: [u8; 32],
    pub asset_id: [u8; 32],
    pub event_type: u16,
    pub required_signer: [u8; 32],
    pub message_bytes: Vec<u8>,
    pub expires_at: OffsetDateTime,
    pub used_at: Option<OffsetDateTime>,
}

const GLOBAL_CHALLENGES_PER_MINUTE: i64 = 240;
const SIGNER_OR_ASSET_CHALLENGES_PER_MINUTE: i64 = 8;

/// Cheap read-only gate before canonical RPC work; `insert_challenge` is authoritative.
pub async fn preflight_rate(pool: &PgPool, asset_id: [u8; 32]) -> Result<(), ApiError> {
    let (global, asset): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE asset_id=$1) FROM v2_capture_challenges WHERE created_at > now() - interval '1 minute'",
    )
    .bind(asset_id.to_vec())
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    if global >= GLOBAL_CHALLENGES_PER_MINUTE || asset >= SIGNER_OR_ASSET_CHALLENGES_PER_MINUTE {
        return Err(budget_exhausted());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_challenge(
    pool: &PgPool,
    challenge_id: Uuid,
    deployment_id: [u8; 32],
    asset_id: [u8; 32],
    event_type: u16,
    required_signer: [u8; 32],
    message_bytes: &[u8],
    expires_at: OffsetDateTime,
) -> Result<ChallengeRecord, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    // One transaction-scoped lock keeps every budget atomic across API replicas.
    sqlx::query("SELECT pg_advisory_xact_lock(5494761203310419792)")
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    sqlx::query(
        "DELETE FROM v2_capture_challenges WHERE challenge_id IN (SELECT challenge_id FROM v2_capture_challenges WHERE created_at < now() - interval '1 day' ORDER BY created_at LIMIT 128)",
    )
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    let (global, signer, asset): (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE required_signer=$1), count(*) FILTER (WHERE asset_id=$2) FROM v2_capture_challenges WHERE created_at > now() - interval '1 minute'",
    )
    .bind(required_signer.to_vec())
    .bind(asset_id.to_vec())
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    if global >= GLOBAL_CHALLENGES_PER_MINUTE
        || signer >= SIGNER_OR_ASSET_CHALLENGES_PER_MINUTE
        || asset >= SIGNER_OR_ASSET_CHALLENGES_PER_MINUTE
    {
        return Err(budget_exhausted());
    }
    let row = sqlx::query(
        "INSERT INTO v2_capture_challenges(challenge_id,deployment_id,asset_id,event_type,required_signer,message_bytes,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING *",
    )
    .bind(challenge_id)
    .bind(deployment_id.to_vec())
    .bind(asset_id.to_vec())
    .bind(i16::try_from(event_type).map_err(|_| ApiError::Internal)?)
    .bind(required_signer.to_vec())
    .bind(message_bytes)
    .bind(expires_at)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    decode_challenge(&row)
}

pub async fn challenge_for_update(
    tx: &mut Transaction<'_, Postgres>,
    challenge_id: Uuid,
) -> Result<Option<ChallengeRecord>, ApiError> {
    let row = sqlx::query("SELECT * FROM v2_capture_challenges WHERE challenge_id=$1 FOR UPDATE")
        .bind(challenge_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?;
    row.as_ref().map(decode_challenge).transpose()
}

pub async fn consume_challenge_tx(
    tx: &mut Transaction<'_, Postgres>,
    challenge_id: Uuid,
) -> Result<bool, ApiError> {
    let result = sqlx::query(
        "UPDATE v2_capture_challenges SET used_at=now() WHERE challenge_id=$1 AND used_at IS NULL AND expires_at>now()",
    )
    .bind(challenge_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    Ok(result.rows_affected() == 1)
}

async fn expire_stale_tx(tx: &mut Transaction<'_, Postgres>) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE v2_captures SET status='EXPIRED' WHERE status IN ('PENDING','DISPATCHED') AND expires_at <= extract(epoch FROM now())::bigint",
    )
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    Ok(())
}

/// Inserts a PENDING capture. A Station owns at most one active capture at a time.
pub async fn insert_capture_tx(
    tx: &mut Transaction<'_, Postgres>,
    value: &CaptureRecord,
) -> Result<CaptureRecord, ApiError> {
    expire_stale_tx(tx).await?;
    let row = sqlx::query(
        r#"INSERT INTO v2_captures(
            capture_id,deployment_id,station_id,asset_id,event_type,event_id,state_version,
            previous_event_hash,expected_rfid_hash,required_signer,observed_at,expires_at,status
        ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,'PENDING') RETURNING *"#,
    )
    .bind(value.capture_id)
    .bind(value.deployment_id.to_vec())
    .bind(value.station_id.to_vec())
    .bind(value.asset_id.to_vec())
    .bind(i16::try_from(value.event_type).map_err(|_| ApiError::Internal)?)
    .bind(value.event_id.to_vec())
    .bind(i64::try_from(value.state_version).map_err(|_| ApiError::Internal)?)
    .bind(value.previous_event_hash.to_vec())
    .bind(value.expected_rfid_hash.to_vec())
    .bind(value.required_signer.to_vec())
    .bind(value.observed_at)
    .bind(value.expires_at)
    .fetch_one(&mut **tx)
    .await
    .map_err(|error| match &error {
        sqlx::Error::Database(db) if db.is_unique_violation() => ApiError::Conflict(
            "the Station is busy with another capture; retry after it completes or expires".into(),
        ),
        _ => db_error(error),
    })?;
    decode_capture(&row)
}

/// The asset's live capture, used for idempotent retries: not expired/cancelled, and not a
/// capture whose event already reached a terminal state (FINALIZED or REJECTED).
pub async fn find_open_for_asset(
    pool: &PgPool,
    asset_id: [u8; 32],
) -> Result<Option<CaptureRecord>, ApiError> {
    let row = sqlx::query(
        r#"SELECT c.* FROM v2_captures c
             LEFT JOIN v2_event_anchors a ON a.event_hash = c.event_hash
            WHERE c.asset_id=$1
              AND c.status IN ('PENDING','DISPATCHED','EVIDENCE_ACCEPTED')
              AND (c.status='EVIDENCE_ACCEPTED' OR c.expires_at > extract(epoch FROM now())::bigint)
              AND (a.status IS NULL OR a.status IN ('EVIDENCE_ACCEPTED','SUBMITTED'))
            ORDER BY c.created_at DESC LIMIT 1"#,
    )
    .bind(asset_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_capture).transpose()
}

pub async fn cancel_tx(
    tx: &mut Transaction<'_, Postgres>,
    capture_id: Uuid,
) -> Result<(), ApiError> {
    sqlx::query("UPDATE v2_captures SET status='CANCELLED' WHERE capture_id=$1 AND status IN ('PENDING','DISPATCHED','EVIDENCE_ACCEPTED')")
        .bind(capture_id)
        .execute(&mut **tx)
        .await
        .map_err(db_error)?;
    Ok(())
}

pub async fn find(pool: &PgPool, capture_id: Uuid) -> Result<Option<CaptureRecord>, ApiError> {
    let row = sqlx::query("SELECT * FROM v2_captures WHERE capture_id=$1")
        .bind(capture_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    row.as_ref().map(decode_capture).transpose()
}

/// Returns the Station's DISPATCHED capture again (Agent restart) or claims the oldest PENDING.
pub async fn claim_next_for_station(
    pool: &PgPool,
    station_id: [u8; 32],
) -> Result<Option<CaptureRecord>, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    expire_stale_tx(&mut tx).await?;
    let row = sqlx::query(
        "SELECT * FROM v2_captures WHERE station_id=$1 AND status IN ('DISPATCHED','PENDING') ORDER BY CASE status WHEN 'DISPATCHED' THEN 0 ELSE 1 END, created_at, capture_id FOR UPDATE SKIP LOCKED LIMIT 1",
    )
    .bind(station_id.to_vec())
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?;
    let Some(row) = row else {
        tx.commit().await.map_err(db_error)?;
        return Ok(None);
    };
    let mut selected = decode_capture(&row)?;
    if selected.status == CaptureStatus::Pending {
        let claimed = sqlx::query(
            "UPDATE v2_captures SET status='DISPATCHED' WHERE capture_id=$1 AND status='PENDING' RETURNING *",
        )
        .bind(selected.capture_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
        selected = decode_capture(&claimed)?;
    }
    tx.commit().await.map_err(db_error)?;
    Ok(Some(selected))
}

pub async fn for_evidence_tx(
    tx: &mut Transaction<'_, Postgres>,
    capture_id: Uuid,
) -> Result<Option<CaptureRecord>, ApiError> {
    expire_stale_tx(tx).await?;
    let row = sqlx::query("SELECT * FROM v2_captures WHERE capture_id=$1 FOR UPDATE")
        .bind(capture_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?;
    row.as_ref().map(decode_capture).transpose()
}

pub async fn mark_evidence_accepted_tx(
    tx: &mut Transaction<'_, Postgres>,
    capture_id: Uuid,
    event_hash: [u8; 32],
) -> Result<(), ApiError> {
    let result = sqlx::query(
        "UPDATE v2_captures SET status='EVIDENCE_ACCEPTED', event_hash=$2 WHERE capture_id=$1 AND status='DISPATCHED'",
    )
    .bind(capture_id)
    .bind(event_hash.to_vec())
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    if result.rows_affected() == 1 {
        Ok(())
    } else {
        Err(ApiError::Conflict(
            "capture is not active for evidence admission".into(),
        ))
    }
}

fn decode_capture(row: &PgRow) -> Result<CaptureRecord, ApiError> {
    let status: String = row.try_get("status").map_err(db_error)?;
    let event_type: i16 = row.try_get("event_type").map_err(db_error)?;
    let version: i64 = row.try_get("state_version").map_err(db_error)?;
    let event_hash: Option<Vec<u8>> = row.try_get("event_hash").map_err(db_error)?;
    Ok(CaptureRecord {
        capture_id: row.try_get("capture_id").map_err(db_error)?,
        deployment_id: fixed(row, "deployment_id")?,
        station_id: fixed(row, "station_id")?,
        asset_id: fixed(row, "asset_id")?,
        event_type: u16::try_from(event_type).map_err(|_| ApiError::Internal)?,
        event_id: fixed(row, "event_id")?,
        state_version: u64::try_from(version).map_err(|_| ApiError::Internal)?,
        previous_event_hash: fixed(row, "previous_event_hash")?,
        expected_rfid_hash: fixed(row, "expected_rfid_hash")?,
        required_signer: fixed(row, "required_signer")?,
        observed_at: row.try_get("observed_at").map_err(db_error)?,
        expires_at: row.try_get("expires_at").map_err(db_error)?,
        status: match status.as_str() {
            "PENDING" => CaptureStatus::Pending,
            "DISPATCHED" => CaptureStatus::Dispatched,
            "EVIDENCE_ACCEPTED" => CaptureStatus::EvidenceAccepted,
            "EXPIRED" => CaptureStatus::Expired,
            "CANCELLED" => CaptureStatus::Cancelled,
            _ => return Err(ApiError::Internal),
        },
        event_hash: event_hash
            .map(|value| value.try_into().map_err(|_| ApiError::Internal))
            .transpose()?,
    })
}

fn decode_challenge(row: &PgRow) -> Result<ChallengeRecord, ApiError> {
    let event_type: i16 = row.try_get("event_type").map_err(db_error)?;
    Ok(ChallengeRecord {
        challenge_id: row.try_get("challenge_id").map_err(db_error)?,
        deployment_id: fixed(row, "deployment_id")?,
        asset_id: fixed(row, "asset_id")?,
        event_type: u16::try_from(event_type).map_err(|_| ApiError::Internal)?,
        required_signer: fixed(row, "required_signer")?,
        message_bytes: row.try_get("message_bytes").map_err(db_error)?,
        expires_at: row.try_get("expires_at").map_err(db_error)?,
        used_at: row.try_get("used_at").map_err(db_error)?,
    })
}

fn fixed<const N: usize>(row: &PgRow, column: &str) -> Result<[u8; N], ApiError> {
    let value: Vec<u8> = row.try_get(column).map_err(db_error)?;
    value.try_into().map_err(|_| ApiError::Internal)
}

fn budget_exhausted() -> ApiError {
    ApiError::RateLimited(
        "capture authorization challenge budget exhausted; retry after one minute".into(),
    )
}

fn db_error(_error: sqlx::Error) -> ApiError {
    ApiError::Unavailable("postgres query failed".into())
}
