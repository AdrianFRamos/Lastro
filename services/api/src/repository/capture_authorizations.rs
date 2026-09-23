//! Short-lived wallet authorization challenges for public capture creation.

use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Clone, Debug)]
pub struct CaptureAuthorizationRecord {
    pub challenge_id: Uuid,
    pub deployment_id: [u8; 32],
    pub action: u8,
    pub animal_id: [u8; 32],
    pub next_custodian: Option<[u8; 32]>,
    pub supersede_capture_id: Option<Uuid>,
    pub required_signer: [u8; 32],
    pub message_bytes: Vec<u8>,
    pub expires_at: OffsetDateTime,
    pub used_at: Option<OffsetDateTime>,
}

#[derive(Clone, Debug)]
pub struct NewCaptureAuthorization<'a> {
    pub challenge_id: Uuid,
    pub deployment_id: [u8; 32],
    pub action: u8,
    pub animal_id: [u8; 32],
    pub next_custodian: Option<[u8; 32]>,
    pub supersede_capture_id: Option<Uuid>,
    pub required_signer: [u8; 32],
    pub message_bytes: &'a [u8],
    pub expires_at: OffsetDateTime,
}

/// Cheap read-only gate before canonical RPC work; the insert transaction is the
/// authoritative, serialized check when multiple API replicas race this preflight.
pub async fn preflight_rate(pool: &PgPool, animal_id: [u8; 32]) -> Result<(), ApiError> {
    let global: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM capture_authorization_challenges WHERE created_at > now() - interval '1 minute'",
    )
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    let animal: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM capture_authorization_challenges WHERE animal_id=$1 AND created_at > now() - interval '1 minute'",
    )
    .bind(animal_id.to_vec())
    .fetch_one(pool)
    .await
    .map_err(db_error)?;
    if global >= 240 || animal >= 8 {
        return Err(ApiError::RateLimited(
            "capture authorization challenge budget exhausted; retry after one minute".into(),
        ));
    }
    Ok(())
}

pub async fn insert(
    pool: &PgPool,
    value: &NewCaptureAuthorization<'_>,
) -> Result<CaptureAuthorizationRecord, ApiError> {
    // A transaction-scoped lock makes the global and per-signer budgets atomic across
    // API replicas. The index lets both checks scan only the current minute.
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("SELECT pg_advisory_xact_lock(5494761203310419791)")
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    // Retain a short audit window while amortizing cleanup without a background worker.
    sqlx::query(
        "DELETE FROM capture_authorization_challenges WHERE challenge_id IN (SELECT challenge_id FROM capture_authorization_challenges WHERE created_at < now() - interval '1 day' ORDER BY created_at LIMIT 128)",
    )
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    let global: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM capture_authorization_challenges WHERE created_at > now() - interval '1 minute'",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    let signer: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM capture_authorization_challenges WHERE required_signer=$1 AND created_at > now() - interval '1 minute'",
    )
    .bind(value.required_signer.to_vec())
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    let animal: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM capture_authorization_challenges WHERE animal_id=$1 AND created_at > now() - interval '1 minute'",
    )
    .bind(value.animal_id.to_vec())
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    if global >= 240 || signer >= 8 || animal >= 8 {
        return Err(ApiError::RateLimited(
            "capture authorization challenge budget exhausted; retry after one minute".into(),
        ));
    }
    let row = sqlx::query(
        r#"INSERT INTO capture_authorization_challenges(
             challenge_id,deployment_id,action,animal_id,next_custodian,
             required_signer,message_bytes,expires_at,supersede_capture_id)
           VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)
           RETURNING *"#,
    )
    .bind(value.challenge_id)
    .bind(value.deployment_id.to_vec())
    .bind(i16::from(value.action))
    .bind(value.animal_id.to_vec())
    .bind(value.next_custodian.map(|v| v.to_vec()))
    .bind(value.required_signer.to_vec())
    .bind(value.message_bytes)
    .bind(value.expires_at)
    .bind(value.supersede_capture_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    decode(&row)
}

pub async fn find(
    pool: &PgPool,
    challenge_id: Uuid,
) -> Result<Option<CaptureAuthorizationRecord>, ApiError> {
    let row = sqlx::query("SELECT * FROM capture_authorization_challenges WHERE challenge_id=$1")
        .bind(challenge_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    row.as_ref().map(decode).transpose()
}

/// Atomically consume a challenge after its intent and signature have already been verified.
/// Returns false when another request consumed it first or it expired concurrently.
pub async fn consume_if_active(pool: &PgPool, challenge_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query(
        "UPDATE capture_authorization_challenges SET used_at=now() WHERE challenge_id=$1 AND used_at IS NULL AND expires_at>now()",
    )
    .bind(challenge_id)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(result.rows_affected() == 1)
}

pub async fn find_for_update(
    tx: &mut Transaction<'_, Postgres>,
    challenge_id: Uuid,
) -> Result<Option<CaptureAuthorizationRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT * FROM capture_authorization_challenges WHERE challenge_id=$1 FOR UPDATE",
    )
    .bind(challenge_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode).transpose()
}

pub async fn consume_if_active_tx(
    tx: &mut Transaction<'_, Postgres>,
    challenge_id: Uuid,
) -> Result<bool, ApiError> {
    let result = sqlx::query(
        "UPDATE capture_authorization_challenges SET used_at=now() WHERE challenge_id=$1 AND used_at IS NULL AND expires_at>now()",
    )
    .bind(challenge_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    Ok(result.rows_affected() == 1)
}

fn decode(row: &PgRow) -> Result<CaptureAuthorizationRecord, ApiError> {
    let action: i16 = row.try_get("action").map_err(db_error)?;
    let next: Option<Vec<u8>> = row.try_get("next_custodian").map_err(db_error)?;
    Ok(CaptureAuthorizationRecord {
        challenge_id: row.try_get("challenge_id").map_err(db_error)?,
        deployment_id: fixed(row, "deployment_id")?,
        action: u8::try_from(action).map_err(|_| ApiError::Internal)?,
        animal_id: fixed(row, "animal_id")?,
        next_custodian: next
            .map(|value| value.try_into().map_err(|_| ApiError::Internal))
            .transpose()?,
        supersede_capture_id: row.try_get("supersede_capture_id").map_err(db_error)?,
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

fn db_error(_error: sqlx::Error) -> ApiError {
    ApiError::Unavailable("postgres query failed".into())
}
