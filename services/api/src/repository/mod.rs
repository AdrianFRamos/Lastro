//! PostgreSQL persistence. Repository code never decides canonical Solana state.
//! Projection updates after confirmation run in one SQL transaction.

pub mod captures_v2;
pub mod domain_v2;
pub mod facility_boundaries;
pub mod operations;
pub mod orgs;
pub mod processing;
pub mod reconciliation;
pub mod sessions;
pub mod sponsor;
pub mod transformations;

use crate::error::ApiError;

/// Every PostgreSQL failure answers 503 without detail. The log records only the error class
/// and, for database errors, the SQLSTATE and constraint name — never row values, which can
/// carry personal data (e-mails) or evidence bytes.
pub(crate) fn db_error(error: sqlx::Error) -> ApiError {
    match &error {
        sqlx::Error::Database(db) => tracing::error!(
            code = db.code().as_deref().unwrap_or("unknown"),
            constraint = db.constraint().unwrap_or(""),
            "postgres query failed"
        ),
        other => tracing::error!(kind = %error_kind(other), "postgres query failed"),
    }
    ApiError::Unavailable("postgres query failed".into())
}

fn error_kind(error: &sqlx::Error) -> &'static str {
    match error {
        sqlx::Error::PoolTimedOut => "pool timed out",
        sqlx::Error::PoolClosed => "pool closed",
        sqlx::Error::Io(_) => "io",
        sqlx::Error::Tls(_) => "tls",
        sqlx::Error::Protocol(_) => "protocol",
        sqlx::Error::RowNotFound => "row not found",
        sqlx::Error::ColumnDecode { .. } | sqlx::Error::Decode(_) => "decode",
        _ => "other",
    }
}
