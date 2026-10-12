//! Handling sessions (SPEC v2.1, S3/S4a/S8): session projection, frozen manifest, append-only
//! Station evidence and the batches the chain anchored. Never decides canonical state.

use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};

use crate::{error::ApiError, repository::db_error};

#[derive(Clone, Debug)]
pub struct SessionRecord {
    pub session_id: [u8; 32],
    pub deployment_id: [u8; 32],
    pub custodian: [u8; 32],
    pub station_id: [u8; 32],
    pub facility_id: [u8; 32],
    pub max_events: u32,
    pub valid_from: i64,
    pub expires_at: i64,
    pub revoked_at: Option<i64>,
    pub status: String,
    pub open_tx_signature: Option<String>,
}

impl SessionRecord {
    /// Latest instant an admitted observation may carry.
    pub fn observation_deadline(&self) -> i64 {
        self.revoked_at
            .map_or(self.expires_at, |revoked| revoked.min(self.expires_at))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManifestEntry {
    pub asset_id: [u8; 32],
    pub rfid_hash: [u8; 32],
    pub state_version: u64,
    pub last_event_hash: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct SessionObservation {
    pub event_hash: [u8; 32],
    pub session_id: [u8; 32],
    pub asset_id: [u8; 32],
    pub envelope: [u8; 220],
    pub station_pubkey: [u8; 33],
    pub station_signature: [u8; 64],
    pub observed_rfid: [u8; 8],
    pub observed_at: i64,
    pub batch_index: Option<u32>,
    pub leaf_index: Option<u32>,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct BatchRecord {
    pub session_id: [u8; 32],
    pub batch_index: u32,
    pub merkle_root: [u8; 32],
    pub leaf_count: u32,
    pub first_observed_at: i64,
    pub last_observed_at: i64,
    pub status: String,
    pub tx_signature: Option<String>,
}

const SESSION_COLUMNS: &str = "session_id,deployment_id,custodian,station_id,facility_id,max_events,valid_from,expires_at,revoked_at,status,open_tx_signature";
const OBSERVATION_COLUMNS: &str = "event_hash,session_id,asset_id,envelope,station_pubkey,station_signature,observed_rfid,observed_at,batch_index,leaf_index,status";
const BATCH_COLUMNS: &str = "session_id,batch_index,merkle_root,leaf_count,first_observed_at,last_observed_at,status,tx_signature";

pub async fn insert_prepared(pool: &PgPool, session: &SessionRecord) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO v2_sessions(session_id,deployment_id,custodian,station_id,facility_id,max_events,valid_from,expires_at,status) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'PREPARED')",
    )
    .bind(session.session_id.to_vec())
    .bind(session.deployment_id.to_vec())
    .bind(session.custodian.to_vec())
    .bind(session.station_id.to_vec())
    .bind(session.facility_id.to_vec())
    .bind(i32::try_from(session.max_events).map_err(|_| ApiError::Internal)?)
    .bind(session.valid_from)
    .bind(session.expires_at)
    .execute(pool)
    .await
    .map_err(|error| match error {
        // The id is derived from custodian, Station and terms: same terms in the same second.
        sqlx::Error::Database(db) if db.is_unique_violation() => ApiError::Conflict(
            "an identical session was prepared this second; retry in a moment".into(),
        ),
        other => db_error(other),
    })?;
    Ok(())
}

pub async fn find(
    pool: &PgPool,
    deployment_id: [u8; 32],
    session_id: [u8; 32],
) -> Result<Option<SessionRecord>, ApiError> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {SESSION_COLUMNS} FROM v2_sessions WHERE deployment_id=$1 AND session_id=$2"
    )))
    .bind(deployment_id.to_vec())
    .bind(session_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_session).transpose()
}

/// PREPARED -> OPEN once the grant is finalized, freezing the manifest: every active asset the
/// custodian holds with a bound RFID, at its projected head.
pub async fn open_with_manifest(
    pool: &PgPool,
    session_id: [u8; 32],
    tx_signature: &str,
    max_assets: u32,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    let updated = sqlx::query(
        "UPDATE v2_sessions SET status='OPEN',open_tx_signature=$2 WHERE session_id=$1 AND status='PREPARED'",
    )
    .bind(session_id.to_vec())
    .bind(tx_signature)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    if updated.rows_affected() == 1 {
        sqlx::query(
            "INSERT INTO v2_session_manifest(session_id,asset_id,rfid_hash,state_version,last_event_hash) \
             SELECT s.session_id,a.asset_id,a.current_rfid_hash,a.state_version,COALESCE(a.last_event_hash,'\\x0000000000000000000000000000000000000000000000000000000000000000'::bytea) \
             FROM v2_sessions s JOIN v2_assets a ON a.deployment_id=s.deployment_id AND a.custodian=s.custodian \
             WHERE s.session_id=$1 AND a.status=1 AND a.current_rfid_hash IS NOT NULL AND a.state_version>=1 \
             ORDER BY a.asset_id LIMIT $2",
        )
        .bind(session_id.to_vec())
        .bind(i64::from(max_assets))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    }
    tx.commit().await.map_err(db_error)?;
    Ok(())
}

pub async fn mark_revoked(
    pool: &PgPool,
    session_id: [u8; 32],
    revoked_at: i64,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE v2_sessions SET status='REVOKED',revoked_at=$2 WHERE session_id=$1 AND status='OPEN'",
    )
    .bind(session_id.to_vec())
    .bind(revoked_at)
    .execute(pool)
    .await
    .map_err(db_error)?;
    Ok(())
}

pub async fn manifest(pool: &PgPool, session_id: [u8; 32]) -> Result<Vec<ManifestEntry>, ApiError> {
    let rows = sqlx::query(
        "SELECT asset_id,rfid_hash,state_version,last_event_hash FROM v2_session_manifest WHERE session_id=$1 ORDER BY asset_id",
    )
    .bind(session_id.to_vec())
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    rows.iter().map(decode_manifest).collect()
}

pub async fn manifest_entry(
    pool: &PgPool,
    session_id: [u8; 32],
    asset_id: [u8; 32],
) -> Result<Option<ManifestEntry>, ApiError> {
    let row = sqlx::query(
        "SELECT asset_id,rfid_hash,state_version,last_event_hash FROM v2_session_manifest WHERE session_id=$1 AND asset_id=$2",
    )
    .bind(session_id.to_vec())
    .bind(asset_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_manifest).transpose()
}

/// Inserts verified evidence. Returns `true` when inserted, `false` when the identical
/// evidence already exists; a different observation of the same asset is a conflict.
pub async fn insert_observation(
    pool: &PgPool,
    observation: &SessionObservation,
) -> Result<bool, ApiError> {
    let inserted = sqlx::query(
        "INSERT INTO v2_session_observations(event_hash,session_id,asset_id,envelope,station_pubkey,station_signature,observed_rfid,observed_at,status) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'PENDING') ON CONFLICT DO NOTHING",
    )
    .bind(observation.event_hash.to_vec())
    .bind(observation.session_id.to_vec())
    .bind(observation.asset_id.to_vec())
    .bind(observation.envelope.to_vec())
    .bind(observation.station_pubkey.to_vec())
    .bind(observation.station_signature.to_vec())
    .bind(observation.observed_rfid.to_vec())
    .bind(observation.observed_at)
    .execute(pool)
    .await
    .map_err(db_error)?
    .rows_affected()
        == 1;
    if inserted {
        return Ok(true);
    }
    let existing = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {OBSERVATION_COLUMNS} FROM v2_session_observations WHERE session_id=$1 AND asset_id=$2"
    )))
    .bind(observation.session_id.to_vec())
    .bind(observation.asset_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    match existing.as_ref().map(decode_observation).transpose()? {
        Some(stored)
            if stored.event_hash == observation.event_hash
                && stored.envelope == observation.envelope
                && stored.station_signature == observation.station_signature =>
        {
            Ok(false)
        }
        _ => Err(ApiError::Conflict(
            "this asset already has a different observation in this session".into(),
        )),
    }
}

pub async fn find_batch(
    pool: &PgPool,
    session_id: [u8; 32],
    batch_index: u32,
) -> Result<Option<BatchRecord>, ApiError> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {BATCH_COLUMNS} FROM v2_session_batches WHERE session_id=$1 AND batch_index=$2"
    )))
    .bind(session_id.to_vec())
    .bind(i32::try_from(batch_index).map_err(|_| ApiError::Internal)?)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_batch).transpose()
}

/// Observations of one batch in leaf order.
pub async fn batch_leaves(
    pool: &PgPool,
    session_id: [u8; 32],
    batch_index: u32,
) -> Result<Vec<SessionObservation>, ApiError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {OBSERVATION_COLUMNS} FROM v2_session_observations WHERE session_id=$1 AND batch_index=$2 ORDER BY leaf_index"
    )))
    .bind(session_id.to_vec())
    .bind(i32::try_from(batch_index).map_err(|_| ApiError::Internal)?)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    rows.iter().map(decode_observation).collect()
}

/// Oldest pending observations, in the order they will become leaves.
pub async fn pending_observations(
    pool: &PgPool,
    session_id: [u8; 32],
    limit: u32,
) -> Result<Vec<SessionObservation>, ApiError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {OBSERVATION_COLUMNS} FROM v2_session_observations WHERE session_id=$1 AND status='PENDING' ORDER BY observed_at,event_hash LIMIT $2"
    )))
    .bind(session_id.to_vec())
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    rows.iter().map(decode_observation).collect()
}

/// Records a prepared batch and assigns its leaves, all or nothing.
pub async fn prepare_batch(
    pool: &PgPool,
    batch: &BatchRecord,
    leaves: &[[u8; 32]],
) -> Result<(), ApiError> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await.map_err(db_error)?;
    sqlx::query(
        "INSERT INTO v2_session_batches(session_id,batch_index,merkle_root,leaf_count,first_observed_at,last_observed_at,status) VALUES($1,$2,$3,$4,$5,$6,'PREPARED')",
    )
    .bind(batch.session_id.to_vec())
    .bind(i32::try_from(batch.batch_index).map_err(|_| ApiError::Internal)?)
    .bind(batch.merkle_root.to_vec())
    .bind(i32::try_from(batch.leaf_count).map_err(|_| ApiError::Internal)?)
    .bind(batch.first_observed_at)
    .bind(batch.last_observed_at)
    .execute(&mut *tx)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            ApiError::Conflict("this batch was already prepared".into())
        }
        _ => ApiError::Unavailable("postgres query failed".into()),
    })?;
    for (index, event_hash) in leaves.iter().enumerate() {
        let assigned = sqlx::query(
            "UPDATE v2_session_observations SET status='BATCHED',batch_index=$2,leaf_index=$3 WHERE event_hash=$1 AND status='PENDING'",
        )
        .bind(event_hash.to_vec())
        .bind(i32::try_from(batch.batch_index).map_err(|_| ApiError::Internal)?)
        .bind(i32::try_from(index).map_err(|_| ApiError::Internal)?)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        if assigned.rows_affected() != 1 {
            return Err(ApiError::Conflict(
                "an observation was batched concurrently; prepare again".into(),
            ));
        }
    }
    tx.commit().await.map_err(db_error)?;
    Ok(())
}

pub async fn mark_batch_anchored(
    pool: &PgPool,
    session_id: [u8; 32],
    batch_index: u32,
    tx_signature: &str,
) -> Result<(), ApiError> {
    let index = i32::try_from(batch_index).map_err(|_| ApiError::Internal)?;
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query(
        "UPDATE v2_session_batches SET status='ANCHORED',tx_signature=$3 WHERE session_id=$1 AND batch_index=$2 AND status='PREPARED'",
    )
    .bind(session_id.to_vec())
    .bind(index)
    .bind(tx_signature)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    sqlx::query(
        "UPDATE v2_session_observations SET status='ANCHORED' WHERE session_id=$1 AND batch_index=$2",
    )
    .bind(session_id.to_vec())
    .bind(index)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(())
}

/// Anchored presence proofs of one asset, oldest first (bounded like the event history).
pub async fn anchored_for_asset(
    pool: &PgPool,
    deployment_id: [u8; 32],
    asset_id: [u8; 32],
    limit: u32,
) -> Result<Vec<SessionObservation>, ApiError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {} FROM v2_session_observations o JOIN v2_sessions s ON s.session_id=o.session_id WHERE s.deployment_id=$1 AND o.asset_id=$2 AND o.status='ANCHORED' ORDER BY o.observed_at,o.event_hash LIMIT $3",
        OBSERVATION_COLUMNS
            .split(',')
            .map(|column| format!("o.{column}"))
            .collect::<Vec<_>>()
            .join(",")
    )))
    .bind(deployment_id.to_vec())
    .bind(asset_id.to_vec())
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    rows.iter().map(decode_observation).collect()
}

fn decode_session(row: &PgRow) -> Result<SessionRecord, ApiError> {
    Ok(SessionRecord {
        session_id: fixed(row, "session_id")?,
        deployment_id: fixed(row, "deployment_id")?,
        custodian: fixed(row, "custodian")?,
        station_id: fixed(row, "station_id")?,
        facility_id: fixed(row, "facility_id")?,
        max_events: unsigned(row, "max_events")?,
        valid_from: row.try_get("valid_from").map_err(db_error)?,
        expires_at: row.try_get("expires_at").map_err(db_error)?,
        revoked_at: row.try_get("revoked_at").map_err(db_error)?,
        status: row.try_get("status").map_err(db_error)?,
        open_tx_signature: row.try_get("open_tx_signature").map_err(db_error)?,
    })
}

fn decode_manifest(row: &PgRow) -> Result<ManifestEntry, ApiError> {
    let version: i64 = row.try_get("state_version").map_err(db_error)?;
    Ok(ManifestEntry {
        asset_id: fixed(row, "asset_id")?,
        rfid_hash: fixed(row, "rfid_hash")?,
        state_version: u64::try_from(version).map_err(|_| ApiError::Internal)?,
        last_event_hash: fixed(row, "last_event_hash")?,
    })
}

fn decode_observation(row: &PgRow) -> Result<SessionObservation, ApiError> {
    let optional = |column: &str| -> Result<Option<u32>, ApiError> {
        let value: Option<i32> = row.try_get(column).map_err(db_error)?;
        value
            .map(|value| u32::try_from(value).map_err(|_| ApiError::Internal))
            .transpose()
    };
    Ok(SessionObservation {
        event_hash: fixed(row, "event_hash")?,
        session_id: fixed(row, "session_id")?,
        asset_id: fixed(row, "asset_id")?,
        envelope: fixed(row, "envelope")?,
        station_pubkey: fixed(row, "station_pubkey")?,
        station_signature: fixed(row, "station_signature")?,
        observed_rfid: fixed(row, "observed_rfid")?,
        observed_at: row.try_get("observed_at").map_err(db_error)?,
        batch_index: optional("batch_index")?,
        leaf_index: optional("leaf_index")?,
        status: row.try_get("status").map_err(db_error)?,
    })
}

fn decode_batch(row: &PgRow) -> Result<BatchRecord, ApiError> {
    Ok(BatchRecord {
        session_id: fixed(row, "session_id")?,
        batch_index: unsigned(row, "batch_index")?,
        merkle_root: fixed(row, "merkle_root")?,
        leaf_count: unsigned(row, "leaf_count")?,
        first_observed_at: row.try_get("first_observed_at").map_err(db_error)?,
        last_observed_at: row.try_get("last_observed_at").map_err(db_error)?,
        status: row.try_get("status").map_err(db_error)?,
        tx_signature: row.try_get("tx_signature").map_err(db_error)?,
    })
}

fn fixed<const N: usize>(row: &PgRow, column: &str) -> Result<[u8; N], ApiError> {
    let value: Vec<u8> = row.try_get(column).map_err(db_error)?;
    value.try_into().map_err(|_| ApiError::Internal)
}

fn unsigned(row: &PgRow, column: &str) -> Result<u32, ApiError> {
    let value: i32 = row.try_get(column).map_err(db_error)?;
    u32::try_from(value).map_err(|_| ApiError::Internal)
}

/// Sessions of `custodian` still alive: open and unexpired, or prepared recently enough that
/// `open_session` could still land (the program allows a 10-minute backdate).
pub async fn count_live(
    pool: &PgPool,
    deployment_id: [u8; 32],
    custodian: [u8; 32],
) -> Result<i64, ApiError> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    sqlx::query_scalar(
        "SELECT count(*) FROM v2_sessions WHERE deployment_id=$1 AND custodian=$2 AND ((status='OPEN' AND expires_at > $3) OR (status='PREPARED' AND valid_from > $3 - 600))",
    )
    .bind(deployment_id.to_vec())
    .bind(custodian.to_vec())
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(db_error)
}
