//! PostgreSQL boundary.
//!
//! Startup is allowed only after a connection succeeds and every SQLx migration
//! applies. The migration contains the length/uniqueness/immutability constraints
//! that act as a second line of defense behind domain validation.

use sqlx::{postgres::PgPoolOptions, PgPool};
use crate::error::ApiError;

pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, ApiError> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .map_err(|_| ApiError::Unavailable("postgres connection failed".into()))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|_| ApiError::Internal)?;

    Ok(pool)
}
