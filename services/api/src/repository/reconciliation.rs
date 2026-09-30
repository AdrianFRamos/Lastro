//! Durable reconciliation queue shared by HTTP confirmation and the background worker.
//!
//! The queue is deliberately a projection concern: it never makes a PostgreSQL row
//! canonical by itself. A claimed job must still pass the exact Solana transaction and
//! canonical-account checks before it can be marked DONE.

use sqlx::{PgPool, Row, postgres::PgRow};

use crate::error::ApiError;

const LEASE_SECONDS: i64 = 30;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconciliationJob {
    pub job_id: i64,
    pub kind: String,
    pub target_hash: [u8; 32],
    pub tx_signature: String,
    pub attempts: i32,
}

/// Seed jobs from submitted v2 events. The insert is idempotent and never changes a
/// transaction signature already associated with a target hash.
pub async fn seed_pending(pool: &PgPool) -> Result<u64, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let domain = sqlx::query(
        r#"INSERT INTO reconciliation_jobs(kind,target_hash,tx_signature)
           SELECT 'DOMAIN_EVENT',event_hash,tx_signature
             FROM v2_event_anchors
            WHERE status='SUBMITTED' AND tx_signature IS NOT NULL
           ON CONFLICT (kind,target_hash) DO NOTHING"#,
    )
    .execute(&mut *tx)
    .await
    .map_err(db_error)?
    .rows_affected();
    tx.commit().await.map_err(db_error)?;
    Ok(domain)
}

/// Claim a bounded batch. Expired leases are made retryable before the SKIP LOCKED
/// query, so a crashed worker becomes recoverable without operator intervention.
pub async fn claim_batch(
    pool: &PgPool,
    worker_id: &str,
    limit: i64,
) -> Result<Vec<ReconciliationJob>, ApiError> {
    if worker_id.trim().is_empty() || limit <= 0 || limit > 256 {
        return Err(ApiError::Validation(
            "invalid reconciliation claim parameters".into(),
        ));
    }
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query(
        r#"UPDATE reconciliation_jobs
              SET status='RETRY', locked_by=NULL, locked_until=NULL,
                  last_error=COALESCE(last_error, 'worker lease expired')
            WHERE status='CLAIMED' AND locked_until IS NOT NULL AND locked_until < now()"#,
    )
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    let rows = sqlx::query(
        r#"WITH candidates AS (
               SELECT job_id
                 FROM reconciliation_jobs
                WHERE status IN ('PENDING','RETRY') AND available_at <= now()
                ORDER BY job_id
                FOR UPDATE SKIP LOCKED
                LIMIT $1
           )
           UPDATE reconciliation_jobs AS jobs
              SET status='CLAIMED', locked_by=$2,
                  locked_until=now() + $3 * interval '1 second',
                  attempts=jobs.attempts + 1
             FROM candidates
            WHERE jobs.job_id=candidates.job_id
        RETURNING jobs.*"#,
    )
    .bind(limit)
    .bind(worker_id)
    .bind(LEASE_SECONDS)
    .fetch_all(&mut *tx)
    .await
    .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    rows.iter().map(decode).collect()
}

pub async fn mark_done(
    pool: &PgPool,
    job: &ReconciliationJob,
    worker_id: &str,
) -> Result<(), ApiError> {
    let result = sqlx::query(
        r#"UPDATE reconciliation_jobs
              SET status='DONE', locked_by=NULL, locked_until=NULL,
                  last_error=NULL, completed_at=now()
            WHERE job_id=$1 AND status='CLAIMED' AND locked_by=$2"#,
    )
    .bind(job.job_id)
    .bind(worker_id)
    .execute(pool)
    .await
    .map_err(db_error)?;
    if result.rows_affected() == 1 {
        Ok(())
    } else {
        Err(ApiError::Conflict(
            "reconciliation job lease is no longer owned by this worker".into(),
        ))
    }
}

pub async fn mark_retry(
    pool: &PgPool,
    job: &ReconciliationJob,
    worker_id: &str,
    error: &str,
    delay_seconds: i64,
) -> Result<(), ApiError> {
    let delay_seconds = delay_seconds.clamp(1, 3600);
    let result = sqlx::query(
        r#"UPDATE reconciliation_jobs
              SET status='RETRY', locked_by=NULL, locked_until=NULL,
                  available_at=now() + $4 * interval '1 second', last_error=$3
            WHERE job_id=$1 AND status='CLAIMED' AND locked_by=$2"#,
    )
    .bind(job.job_id)
    .bind(worker_id)
    .bind(truncate_error(error))
    .bind(delay_seconds)
    .execute(pool)
    .await
    .map_err(db_error)?;
    if result.rows_affected() == 1 {
        Ok(())
    } else {
        Err(ApiError::Conflict(
            "reconciliation retry lease is no longer owned by this worker".into(),
        ))
    }
}

pub async fn quarantine(
    pool: &PgPool,
    job: &ReconciliationJob,
    worker_id: &str,
    error: &str,
) -> Result<(), ApiError> {
    let result = sqlx::query(
        r#"UPDATE reconciliation_jobs
              SET status='QUARANTINED', locked_by=NULL, locked_until=NULL,
                  last_error=$3
            WHERE job_id=$1 AND status='CLAIMED' AND locked_by=$2"#,
    )
    .bind(job.job_id)
    .bind(worker_id)
    .bind(truncate_error(error))
    .execute(pool)
    .await
    .map_err(db_error)?;
    if result.rows_affected() == 1 {
        Ok(())
    } else {
        Err(ApiError::Conflict(
            "reconciliation quarantine lease is no longer owned by this worker".into(),
        ))
    }
}

/// Explicit operator action: requeue a quarantined target without changing its identity.
pub async fn requeue_quarantined(
    pool: &PgPool,
    kind: &str,
    target_hash: [u8; 32],
) -> Result<(), ApiError> {
    if kind != "DOMAIN_EVENT" {
        return Err(ApiError::Validation(
            "invalid reconciliation job kind".into(),
        ));
    }
    let result = sqlx::query(
        r#"UPDATE reconciliation_jobs
              SET status='PENDING', available_at=now(), locked_by=NULL,
                  locked_until=NULL, last_error=NULL, completed_at=NULL
            WHERE kind=$1 AND target_hash=$2 AND status='QUARANTINED'"#,
    )
    .bind(kind)
    .bind(target_hash.to_vec())
    .execute(pool)
    .await
    .map_err(db_error)?;
    if result.rows_affected() == 1 {
        Ok(())
    } else {
        Err(ApiError::NotFound(
            "quarantined reconciliation job not found".into(),
        ))
    }
}

/// Used by tests and operational diagnostics to make a retry decision without exposing
/// database details to callers.
pub fn is_retryable(error: &ApiError) -> bool {
    matches!(error, ApiError::Unavailable(_) | ApiError::RateLimited(_))
}

pub fn retry_delay_seconds(attempts: i32) -> i64 {
    let exponent = attempts.clamp(0, 10) as u32;
    2_i64.saturating_pow(exponent).clamp(2, 900)
}

fn decode(row: &PgRow) -> Result<ReconciliationJob, ApiError> {
    let target_hash: Vec<u8> = row.try_get("target_hash").map_err(db_error)?;
    Ok(ReconciliationJob {
        job_id: row.try_get("job_id").map_err(db_error)?,
        kind: row.try_get("kind").map_err(db_error)?,
        target_hash: target_hash.try_into().map_err(|_| ApiError::Internal)?,
        tx_signature: row.try_get("tx_signature").map_err(db_error)?,
        attempts: row.try_get("attempts").map_err(db_error)?,
    })
}

fn truncate_error(error: &str) -> String {
    error.chars().take(1024).collect()
}

fn db_error(_error: sqlx::Error) -> ApiError {
    ApiError::Unavailable("postgres query failed".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_dependency_failures_but_quarantines_conflicts() {
        assert!(is_retryable(&ApiError::Unavailable("rpc".into())));
        assert!(is_retryable(&ApiError::RateLimited("busy".into())));
        assert!(!is_retryable(&ApiError::Conflict("tampered".into())));
        assert!(!is_retryable(&ApiError::Validation("bad".into())));
    }

    #[test]
    fn retry_backoff_is_bounded() {
        assert_eq!(retry_delay_seconds(0), 2);
        assert_eq!(retry_delay_seconds(5), 32);
        assert_eq!(retry_delay_seconds(99), 900);
    }
}
