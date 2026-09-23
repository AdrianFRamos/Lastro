pub mod model;

use std::{str::FromStr, time::Duration};

use sqlx::{
    Row, SqlitePool,
    sqlite::{
        SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow, SqliteSynchronous,
    },
};
use uuid::Uuid;

use crate::error::AgentError;
use model::{OutboxRow, OutboxState};

/// Durable outbox. Evidence bytes are immutable; delivery state may advance to SERVER/FINALIZED or terminal QUARANTINED.
pub struct Spool {
    pool: SqlitePool,
}

impl Spool {
    pub async fn connect_and_migrate(url: &str) -> Result<Self, AgentError> {
        let options = SqliteConnectOptions::from_str(url)
            .map_err(|error| AgentError::Spool(format!("invalid SQLite URL: {error}")))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Full)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await
            .map_err(spool_error)?;
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|error| AgentError::Spool(format!("SQLite migration failed: {error}")))?;
        Ok(Self { pool })
    }

    /// Insert evidence transactionally before any network send. Same event/capture with identical
    /// bytes is idempotent; any divergent duplicate is a hard conflict.
    pub async fn persist_local(&self, row: &OutboxRow) -> Result<(), AgentError> {
        if row.state != OutboxState::Local || row.attempts != 0 {
            return Err(AgentError::Spool(
                "new outbox evidence must start in LOCAL with zero attempts".into(),
            ));
        }

        let mut tx = self.pool.begin().await.map_err(spool_error)?;
        let result = sqlx::query(
            r#"INSERT INTO outbox (
                event_hash, capture_id, event_bytes, observed_rfid, station_pubkey,
                station_signature, state, attempts, last_error, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, 'LOCAL', 0, NULL,
                strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            ON CONFLICT DO NOTHING"#,
        )
        .bind(row.event_hash.to_vec())
        .bind(row.capture_id.to_string())
        .bind(row.event_bytes.to_vec())
        .bind(row.observed_rfid.to_vec())
        .bind(row.station_pubkey.to_vec())
        .bind(row.station_signature.to_vec())
        .execute(&mut *tx)
        .await
        .map_err(spool_error)?;

        if result.rows_affected() == 0 {
            let matches = sqlx::query(
                "SELECT * FROM outbox WHERE event_hash = ? OR capture_id = ? ORDER BY capture_id",
            )
            .bind(row.event_hash.to_vec())
            .bind(row.capture_id.to_string())
            .fetch_all(&mut *tx)
            .await
            .map_err(spool_error)?;

            if matches.len() != 1 {
                return Err(AgentError::Spool(
                    "duplicate capture/event identity resolves to divergent outbox rows".into(),
                ));
            }
            let existing = decode_row(&matches[0])?;
            if !same_evidence(&existing, row) {
                return Err(AgentError::Spool(
                    "duplicate capture/event identity contains divergent evidence".into(),
                ));
            }
        }

        tx.commit().await.map_err(spool_error)?;
        Ok(())
    }

    pub async fn pending(&self) -> Result<Vec<OutboxRow>, AgentError> {
        let rows = sqlx::query(
            "SELECT * FROM outbox WHERE state IN ('LOCAL','SERVER') ORDER BY created_at ASC, capture_id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(spool_error)?;
        rows.iter().map(decode_row).collect()
    }

    pub async fn advance(&self, capture_id: Uuid, to: OutboxState) -> Result<(), AgentError> {
        let mut tx = self.pool.begin().await.map_err(spool_error)?;
        let current: Option<String> =
            sqlx::query_scalar("SELECT state FROM outbox WHERE capture_id = ?")
                .bind(capture_id.to_string())
                .fetch_optional(&mut *tx)
                .await
                .map_err(spool_error)?;
        let current = current
            .as_deref()
            .map(parse_state)
            .transpose()?
            .ok_or_else(|| {
                AgentError::Spool(format!("outbox capture {capture_id} does not exist"))
            })?;

        if current == to {
            tx.commit().await.map_err(spool_error)?;
            return Ok(());
        }
        let valid = matches!(
            (current, to),
            (OutboxState::Local, OutboxState::Server)
                | (OutboxState::Server, OutboxState::Finalized)
        );
        if !valid {
            return Err(AgentError::Spool(format!(
                "invalid outbox state transition {} -> {}",
                state_text(current),
                state_text(to)
            )));
        }

        sqlx::query(
            "UPDATE outbox SET state = ?, attempts = attempts + 1, last_error = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE capture_id = ?",
        )
        .bind(state_text(to))
        .bind(capture_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(spool_error)?;
        tx.commit().await.map_err(spool_error)?;
        Ok(())
    }

    pub async fn quarantine(&self, capture_id: Uuid, error: &str) -> Result<(), AgentError> {
        let result = sqlx::query(
            "UPDATE outbox SET state = 'QUARANTINED', attempts = attempts + 1, last_error = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE capture_id = ? AND state IN ('LOCAL','SERVER')",
        )
        .bind(error)
        .bind(capture_id.to_string())
        .execute(&self.pool)
        .await
        .map_err(spool_error)?;
        if result.rows_affected() != 1 {
            return Err(AgentError::Spool(format!(
                "cannot quarantine non-pending capture {capture_id}"
            )));
        }
        Ok(())
    }

    pub async fn record_failure(&self, capture_id: Uuid, error: &str) -> Result<(), AgentError> {
        let result = sqlx::query(
            "UPDATE outbox SET attempts = attempts + 1, last_error = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE capture_id = ? AND state IN ('LOCAL','SERVER')",
        )
        .bind(error)
        .bind(capture_id.to_string())
        .execute(&self.pool)
        .await
        .map_err(spool_error)?;
        if result.rows_affected() != 1 {
            return Err(AgentError::Spool(format!(
                "cannot record failure for non-pending capture {capture_id}"
            )));
        }
        Ok(())
    }
}

fn decode_row(row: &SqliteRow) -> Result<OutboxRow, AgentError> {
    let capture: String = row.try_get("capture_id").map_err(spool_error)?;
    let attempts: i64 = row.try_get("attempts").map_err(spool_error)?;
    let state: String = row.try_get("state").map_err(spool_error)?;
    Ok(OutboxRow {
        capture_id: Uuid::parse_str(&capture)
            .map_err(|error| AgentError::Spool(format!("invalid capture_id in SQLite: {error}")))?,
        event_hash: fixed_blob(row, "event_hash")?,
        event_bytes: fixed_blob(row, "event_bytes")?,
        observed_rfid: fixed_blob(row, "observed_rfid")?,
        station_pubkey: fixed_blob(row, "station_pubkey")?,
        station_signature: fixed_blob(row, "station_signature")?,
        state: parse_state(&state)?,
        attempts: u32::try_from(attempts)
            .map_err(|_| AgentError::Spool("invalid attempts value in SQLite".into()))?,
    })
}

fn fixed_blob<const N: usize>(row: &SqliteRow, column: &str) -> Result<[u8; N], AgentError> {
    let value: Vec<u8> = row.try_get(column).map_err(spool_error)?;
    value.try_into().map_err(|value: Vec<u8>| {
        AgentError::Spool(format!(
            "SQLite column {column} must contain {N} bytes, got {}",
            value.len()
        ))
    })
}

fn same_evidence(left: &OutboxRow, right: &OutboxRow) -> bool {
    left.capture_id == right.capture_id
        && left.event_hash == right.event_hash
        && left.event_bytes == right.event_bytes
        && left.observed_rfid == right.observed_rfid
        && left.station_pubkey == right.station_pubkey
        && left.station_signature == right.station_signature
}

fn parse_state(value: &str) -> Result<OutboxState, AgentError> {
    match value {
        "LOCAL" => Ok(OutboxState::Local),
        "SERVER" => Ok(OutboxState::Server),
        "FINALIZED" => Ok(OutboxState::Finalized),
        "QUARANTINED" => Ok(OutboxState::Quarantined),
        other => Err(AgentError::Spool(format!("invalid outbox state {other}"))),
    }
}

const fn state_text(state: OutboxState) -> &'static str {
    match state {
        OutboxState::Local => "LOCAL",
        OutboxState::Server => "SERVER",
        OutboxState::Finalized => "FINALIZED",
        OutboxState::Quarantined => "QUARANTINED",
    }
}

fn spool_error(error: sqlx::Error) -> AgentError {
    AgentError::Spool(error.to_string())
}
