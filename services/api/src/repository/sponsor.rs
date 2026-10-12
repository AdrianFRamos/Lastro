//! Spending ledger of the fee sponsor. A row is reserved before broadcast (under a
//! deployment-wide advisory lock, so concurrent requests cannot both pass the budget) and
//! released only if the broadcast fails.

use sqlx::PgPool;

use crate::{error::ApiError, repository::db_error, sponsor::SponsorLimits};

/// Reserves `lamports` for `tx_signature` if neither the signer's nor the deployment's rolling
/// 24 h budget would be exceeded.
pub async fn reserve(
    pool: &PgPool,
    deployment_id: [u8; 32],
    signer: [u8; 32],
    purpose: &str,
    tx_signature: &str,
    lamports: u64,
    limits: &SponsorLimits,
) -> Result<(), ApiError> {
    let amount = i64::try_from(lamports).map_err(|_| ApiError::Internal)?;
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('lastro-sponsor-ledger', 0))")
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    let (signer_spent, total_spent): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(lamports) FILTER (WHERE signer=$2),0)::BIGINT, COALESCE(SUM(lamports),0)::BIGINT FROM v2_sponsor_ledger WHERE deployment_id=$1 AND created_at > now() - interval '24 hours'",
    )
    .bind(deployment_id.to_vec())
    .bind(signer.to_vec())
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    let within = |spent: i64, limit: u64| {
        u64::try_from(spent)
            .ok()
            .and_then(|spent| spent.checked_add(lamports))
            .is_some_and(|after| after <= limit)
    };
    if !within(signer_spent, limits.daily_lamports_per_signer) {
        return Err(ApiError::Conflict(
            "the daily sponsorship budget of this wallet is exhausted".into(),
        ));
    }
    if !within(total_spent, limits.daily_lamports_total) {
        return Err(ApiError::Conflict(
            "the deployment's daily sponsorship budget is exhausted".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO v2_sponsor_ledger(tx_signature,deployment_id,signer,purpose,lamports) VALUES($1,$2,$3,$4,$5)",
    )
    .bind(tx_signature)
    .bind(deployment_id.to_vec())
    .bind(signer.to_vec())
    .bind(purpose)
    .bind(amount)
    .execute(&mut *tx)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            ApiError::Conflict("this transaction was already sponsored".into())
        }
        _ => ApiError::Unavailable("postgres query failed".into()),
    })?;
    tx.commit().await.map_err(db_error)?;
    Ok(())
}

/// Releases a reservation whose transaction never reached the network.
pub async fn release(pool: &PgPool, tx_signature: &str) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM v2_sponsor_ledger WHERE tx_signature=$1")
        .bind(tx_signature)
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

/// A wallet the platform knows: an ACTIVE party or an organization wallet of this deployment.
/// The sponsor pays only for these (and the deployment authority), so throwaway keys cannot
/// drain its budget.
pub async fn is_known_signer(
    pool: &PgPool,
    deployment_id: [u8; 32],
    wallet: [u8; 32],
) -> Result<bool, ApiError> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM v2_parties WHERE deployment_id=$1 AND wallet=$2 AND status='ACTIVE') OR EXISTS(SELECT 1 FROM v2_organizations WHERE deployment_id=$1 AND wallet=$2)",
    )
    .bind(deployment_id.to_vec())
    .bind(wallet.to_vec())
    .fetch_one(pool)
    .await
    .map_err(db_error)
}
