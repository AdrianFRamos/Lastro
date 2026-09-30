//! Background reconciliation between finalized Solana state and PostgreSQL projections.
//!
//! The worker is deliberately conservative: dependency failures are retried, while
//! cryptographic, authorization, transaction-shape, and projection conflicts are
//! quarantined for explicit operator review. It never writes a canonical projection
//! before the same finalized checks used by the synchronous event endpoint pass.

use std::time::Duration;

use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::{
    error::ApiError,
    repository::reconciliation::{self, ReconciliationJob},
    routes::domain_v2,
    state::AppState,
};

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const BATCH_SIZE: i64 = 16;
const MAX_ATTEMPTS: i32 = 12;

/// Start one process-local worker. The durable lease in PostgreSQL makes a second
/// API replica safe: workers claim disjoint jobs with `FOR UPDATE SKIP LOCKED`.
pub fn spawn(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let worker_id = format!("api-reconciler-{}", std::process::id());
        let mut ticker = tokio::time::interval(POLL_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;
            if let Err(error) = run_once(&state, &worker_id).await {
                warn!(worker = %worker_id, error = %error, "reconciliation cycle failed");
            }
        }
    })
}

pub async fn run_once(state: &AppState, worker_id: &str) -> Result<usize, ApiError> {
    reconciliation::seed_pending(&state.db).await?;
    if let Err(error) = domain_v2::settle_unreported_evidence(state).await {
        warn!(worker = %worker_id, error = %error, "settling unreported evidence failed");
    }
    let jobs = reconciliation::claim_batch(&state.db, worker_id, BATCH_SIZE).await?;
    let mut processed = 0usize;

    for job in jobs {
        if process_job(state, worker_id, &job).await {
            processed = processed.saturating_add(1);
        }
    }

    if processed > 0 {
        info!(worker = %worker_id, processed, "reconciliation cycle completed");
    }
    Ok(processed)
}

async fn process_job(state: &AppState, worker_id: &str, job: &ReconciliationJob) -> bool {
    let result = match job.kind.as_str() {
        "DOMAIN_EVENT" => {
            domain_v2::reconcile_finalized_event(state, job.target_hash, &job.tx_signature).await
        }
        _ => Err(ApiError::Internal),
    };

    match result {
        Ok(()) => {
            if let Err(error) = reconciliation::mark_done(&state.db, job, worker_id).await {
                error!(job_id = job.job_id, error = %error, "could not mark reconciliation job done");
                false
            } else {
                true
            }
        }
        Err(error) if reconciliation::is_retryable(&error) && job.attempts < MAX_ATTEMPTS => {
            let delay = reconciliation::retry_delay_seconds(job.attempts);
            if let Err(update_error) =
                reconciliation::mark_retry(&state.db, job, worker_id, &error.to_string(), delay)
                    .await
            {
                error!(job_id = job.job_id, error = %update_error, "could not schedule reconciliation retry");
                false
            } else {
                warn!(job_id = job.job_id, attempts = job.attempts, delay, error = %error, "reconciliation scheduled for retry");
                false
            }
        }
        Err(error) => {
            if let Err(update_error) =
                reconciliation::quarantine(&state.db, job, worker_id, &error.to_string()).await
            {
                error!(job_id = job.job_id, error = %update_error, "could not quarantine reconciliation job");
            } else {
                warn!(job_id = job.job_id, kind = %job.kind, error = %error, "reconciliation job quarantined");
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_id_is_non_empty_and_process_scoped() {
        let id = format!("api-reconciler-{}", std::process::id());
        assert!(id.starts_with("api-reconciler-"));
        assert!(id.len() > "api-reconciler-".len());
    }

    #[test]
    fn retry_policy_has_a_terminal_attempt_limit() {
        assert!(reconciliation::is_retryable(&ApiError::Unavailable(
            "rpc".into()
        )));
        const { assert!(MAX_ATTEMPTS > 0) };
    }
}
