//! SQL access for the off-chain animal projection.
//! Canonical current state may change only after finalized Solana state has been checked.

use lastro_protocol::{Action, StationEvent};
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};

use crate::error::ApiError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnimalRecord {
    pub animal_id: [u8; 32],
    pub visual_recovery_id: String,
    pub current_rfid_hash: Option<[u8; 32]>,
    pub current_custodian: Option<[u8; 32]>,
    pub identity_revision: u32,
    pub event_sequence: u64,
    pub last_event_hash: Option<[u8; 32]>,
}

pub async fn insert_registration(
    pool: &PgPool,
    animal_id: [u8; 32],
    visual_recovery_id: &str,
) -> Result<AnimalRecord, ApiError> {
    let row =
        sqlx::query("INSERT INTO animals(animal_id, visual_recovery_id) VALUES($1,$2) RETURNING *")
            .bind(animal_id.to_vec())
            .bind(visual_recovery_id)
            .fetch_one(pool)
            .await
            .map_err(map_write_error)?;
    decode_animal(&row)
}

/// Keep anonymous demo registration within a deployment-wide persistence budget.
/// The transaction lock serializes the budget check across API replicas.
pub async fn insert_public_registration(
    pool: &PgPool,
    animal_id: [u8; 32],
    visual_recovery_id: &str,
) -> Result<AnimalRecord, ApiError> {
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("SELECT pg_advisory_xact_lock(5494761203310419792)")
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    let recent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM animals WHERE event_sequence=0 AND created_at > now() - interval '1 minute'",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    let unoriginated: i64 =
        sqlx::query_scalar("SELECT count(*) FROM animals WHERE event_sequence=0")
            .fetch_one(&mut *tx)
            .await
            .map_err(db_error)?;
    if recent >= 60 || unoriginated >= 5000 {
        return Err(ApiError::RateLimited(
            "public animal registration budget exhausted".into(),
        ));
    }
    let row =
        sqlx::query("INSERT INTO animals(animal_id, visual_recovery_id) VALUES($1,$2) RETURNING *")
            .bind(animal_id.to_vec())
            .bind(visual_recovery_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_write_error)?;
    tx.commit().await.map_err(db_error)?;
    decode_animal(&row)
}

pub async fn find_by_animal_id(
    pool: &PgPool,
    animal_id: [u8; 32],
) -> Result<Option<AnimalRecord>, ApiError> {
    fetch_optional(
        pool,
        "SELECT * FROM animals WHERE animal_id = $1",
        animal_id.to_vec(),
    )
    .await
}

pub async fn find_by_visual_recovery_id(
    pool: &PgPool,
    visual_recovery_id: &str,
) -> Result<Option<AnimalRecord>, ApiError> {
    let row = sqlx::query("SELECT * FROM animals WHERE visual_recovery_id = $1")
        .bind(visual_recovery_id)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    row.as_ref().map(decode_animal).transpose()
}

pub async fn find_by_current_rfid_hash(
    pool: &PgPool,
    rfid_hash: [u8; 32],
) -> Result<Option<AnimalRecord>, ApiError> {
    fetch_optional(
        pool,
        "SELECT * FROM animals WHERE current_rfid_hash = $1",
        rfid_hash.to_vec(),
    )
    .await
}

pub async fn apply_confirmed_state(
    tx: &mut Transaction<'_, Postgres>,
    event: &StationEvent,
) -> Result<AnimalRecord, ApiError> {
    let event_hash = event.event_hash();
    let row = match event.action {
        Action::Origin => {
            sqlx::query(
                r#"UPDATE animals
                   SET current_rfid_hash=$2,current_custodian=$3,identity_revision=$4,
                       event_sequence=$5,last_event_hash=$6,updated_at=now()
                   WHERE animal_id=$1 AND event_sequence=0 AND identity_revision=0
                     AND current_rfid_hash IS NULL AND current_custodian IS NULL AND last_event_hash IS NULL
                   RETURNING *"#,
            )
            .bind(event.animal_id.to_vec())
            .bind(event.new_rfid_hash.to_vec())
            .bind(event.to_custodian.to_vec())
            .bind(i64::from(event.identity_revision))
            .bind(i64::try_from(event.event_sequence).map_err(|_| ApiError::Internal)?)
            .bind(event_hash.to_vec())
            .fetch_optional(&mut **tx)
            .await
            .map_err(map_write_error)?
        }
        Action::Transfer | Action::Reidentify => {
            let expected_revision = match event.action {
                Action::Transfer => event.identity_revision,
                Action::Reidentify => event.identity_revision.checked_sub(1).ok_or(ApiError::Internal)?,
                Action::Origin => unreachable!(),
            };
            let expected_sequence = event.event_sequence.checked_sub(1).ok_or(ApiError::Internal)?;
            sqlx::query(
                r#"UPDATE animals
                   SET current_rfid_hash=$2,current_custodian=$3,identity_revision=$4,
                       event_sequence=$5,last_event_hash=$6,updated_at=now()
                   WHERE animal_id=$1 AND current_rfid_hash=$7 AND current_custodian=$8
                     AND identity_revision=$9 AND event_sequence=$10 AND last_event_hash=$11
                   RETURNING *"#,
            )
            .bind(event.animal_id.to_vec())
            .bind(event.new_rfid_hash.to_vec())
            .bind(event.to_custodian.to_vec())
            .bind(i64::from(event.identity_revision))
            .bind(i64::try_from(event.event_sequence).map_err(|_| ApiError::Internal)?)
            .bind(event_hash.to_vec())
            .bind(event.old_rfid_hash.to_vec())
            .bind(event.from_custodian.to_vec())
            .bind(i64::from(expected_revision))
            .bind(i64::try_from(expected_sequence).map_err(|_| ApiError::Internal)?)
            .bind(event.previous_event_hash.to_vec())
            .fetch_optional(&mut **tx)
            .await
            .map_err(map_write_error)?
        }
    };
    if let Some(row) = row.as_ref() {
        return decode_animal(row);
    }

    // Confirmation retries are idempotent. If another request committed the same
    // canonical terminal state first, return that row instead of attempting a second
    // state transition. Any different local state remains a hard conflict.
    let current = sqlx::query("SELECT * FROM animals WHERE animal_id=$1 FOR UPDATE")
        .bind(event.animal_id.to_vec())
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?
        .as_ref()
        .map(decode_animal)
        .transpose()?
        .ok_or_else(|| {
            ApiError::Conflict("local projection is missing for confirmed event".into())
        })?;
    if matches_terminal(&current, event) {
        Ok(current)
    } else {
        Err(ApiError::Conflict(
            "local projection predecessor does not match confirmed event".into(),
        ))
    }
}

pub fn matches_terminal(record: &AnimalRecord, event: &StationEvent) -> bool {
    record.animal_id == event.animal_id
        && record.current_rfid_hash == Some(event.new_rfid_hash)
        && record.current_custodian == Some(event.to_custodian)
        && record.identity_revision == event.identity_revision
        && record.event_sequence == event.event_sequence
        && record.last_event_hash == Some(event.event_hash())
}

async fn fetch_optional(
    pool: &PgPool,
    sql: &'static str,
    value: Vec<u8>,
) -> Result<Option<AnimalRecord>, ApiError> {
    let row = sqlx::query(sql)
        .bind(value)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    row.as_ref().map(decode_animal).transpose()
}

fn decode_animal(row: &PgRow) -> Result<AnimalRecord, ApiError> {
    let revision: i32 = row.try_get("identity_revision").map_err(db_error)?;
    let sequence: i64 = row.try_get("event_sequence").map_err(db_error)?;
    Ok(AnimalRecord {
        animal_id: fixed(row, "animal_id")?,
        visual_recovery_id: row.try_get("visual_recovery_id").map_err(db_error)?,
        current_rfid_hash: optional_fixed(row, "current_rfid_hash")?,
        current_custodian: optional_fixed(row, "current_custodian")?,
        identity_revision: u32::try_from(revision).map_err(|_| ApiError::Internal)?,
        event_sequence: u64::try_from(sequence).map_err(|_| ApiError::Internal)?,
        last_event_hash: optional_fixed(row, "last_event_hash")?,
    })
}

fn fixed<const N: usize>(row: &PgRow, column: &str) -> Result<[u8; N], ApiError> {
    let value: Vec<u8> = row.try_get(column).map_err(db_error)?;
    value.try_into().map_err(|_| ApiError::Internal)
}

fn optional_fixed<const N: usize>(row: &PgRow, column: &str) -> Result<Option<[u8; N]>, ApiError> {
    let value: Option<Vec<u8>> = row.try_get(column).map_err(db_error)?;
    value
        .map(|bytes| bytes.try_into().map_err(|_| ApiError::Internal))
        .transpose()
}

fn map_write_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(db) = &error {
        if db.code().as_deref() == Some("23505") {
            return ApiError::Conflict("animal identifier already exists".into());
        }
    }
    db_error(error)
}

fn db_error(_error: sqlx::Error) -> ApiError {
    ApiError::Unavailable("postgres query failed".into())
}
