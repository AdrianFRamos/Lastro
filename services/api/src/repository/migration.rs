//! Controlled v1 -> v2 migration bookkeeping.
//!
//! Planning never promotes data. Promotion requires a v2 AssetState whose
//! sequence, custodian and last event hash exactly match the v1 projection.

use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;

use crate::error::ApiError;

#[derive(Clone, Debug)]
pub struct MigrationRunRecord {
    pub run_id: Uuid,
    pub deployment_id: [u8; 32],
    pub requested_by_party_id: Option<[u8; 32]>,
    pub status: String,
    pub source_count: u64,
    pub eligible_count: u64,
    pub promoted_count: u64,
    pub rejected_count: u64,
}

#[derive(Clone, Debug)]
pub struct MigrationCandidateRecord {
    pub source_animal_id: [u8; 32],
    pub target_asset_id: Option<[u8; 32]>,
    pub status: String,
    pub reason: String,
    pub source_event_sequence: u64,
    pub source_last_event_hash: Option<[u8; 32]>,
}

pub async fn find_run(
    pool: &PgPool,
    deployment_id: [u8; 32],
    run_id: Uuid,
) -> Result<Option<MigrationRunRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT run_id,deployment_id,requested_by_party_id,status,source_count,eligible_count,promoted_count,rejected_count FROM v2_migration_runs WHERE deployment_id=$1 AND run_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_run).transpose()
}

pub async fn list_candidates(
    pool: &PgPool,
    run_id: Uuid,
) -> Result<Vec<MigrationCandidateRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT source_animal_id,target_asset_id,status,reason,source_event_sequence,source_last_event_hash FROM v2_migration_candidates WHERE run_id=$1 ORDER BY source_animal_id LIMIT 10000",
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    rows.iter().map(decode_candidate).collect()
}

pub async fn plan_run_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    run_id: Uuid,
    requested_by_party_id: Option<[u8; 32]>,
) -> Result<MigrationRunRecord, ApiError> {
    sqlx::query(
        "INSERT INTO v2_migration_runs(run_id,deployment_id,requested_by_party_id,source_name,status) VALUES($1,$2,$3,'V1_ANIMALS','PLANNED') RETURNING run_id,deployment_id,requested_by_party_id,status,source_count,eligible_count,promoted_count,rejected_count",
    )
    .bind(run_id)
    .bind(deployment_id.to_vec())
    .bind(requested_by_party_id.map(|value| value.to_vec()))
    .fetch_one(&mut **tx)
    .await
    .map_err(map_constraint_error)?;

    let sources = sqlx::query(
        "SELECT animal_id,event_sequence,last_event_hash FROM animals ORDER BY animal_id LIMIT 10000",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(db_error)?;
    for source in &sources {
        let animal_id: Vec<u8> = source.try_get("animal_id").map_err(db_error)?;
        let sequence: i64 = source.try_get("event_sequence").map_err(db_error)?;
        let last_event_hash: Option<Vec<u8>> =
            source.try_get("last_event_hash").map_err(db_error)?;
        sqlx::query(
            "INSERT INTO v2_migration_candidates(run_id,source_animal_id,target_asset_id,status,reason,source_event_sequence,source_last_event_hash) VALUES($1,$2,$2,'PENDING_REVIEW','requires finalized v2 AssetState proof before promotion',$3,$4)",
        )
        .bind(run_id)
        .bind(animal_id)
        .bind(sequence)
        .bind(last_event_hash)
        .execute(&mut **tx)
        .await
        .map_err(map_constraint_error)?;
    }
    let row = sqlx::query(
        "UPDATE v2_migration_runs SET status='REVIEW_REQUIRED',source_count=$2 WHERE run_id=$1 RETURNING run_id,deployment_id,requested_by_party_id,status,source_count,eligible_count,promoted_count,rejected_count",
    )
    .bind(run_id)
    .bind(i64::try_from(sources.len()).map_err(|_| ApiError::Internal)?)
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;
    decode_run(&row)
}

pub async fn promote_candidate_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    run_id: Uuid,
    source_animal_id: [u8; 32],
) -> Result<MigrationCandidateRecord, ApiError> {
    let candidate = sqlx::query(
        "SELECT source_animal_id,target_asset_id,status,reason,source_event_sequence,source_last_event_hash FROM v2_migration_candidates c JOIN v2_migration_runs r ON r.run_id=c.run_id WHERE r.deployment_id=$1 AND c.run_id=$2 AND c.source_animal_id=$3 FOR UPDATE",
    )
    .bind(deployment_id.to_vec())
    .bind(run_id)
    .bind(source_animal_id.to_vec())
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::NotFound("migration candidate not found".into()))?;
    let current = decode_candidate(&candidate)?;
    if current.status == "PROMOTED" {
        return Ok(current);
    }
    if current.target_asset_id != Some(source_animal_id) {
        return Err(ApiError::Conflict(
            "migration candidate target mapping is not deterministic".into(),
        ));
    }
    let asset = sqlx::query(
        "SELECT event_sequence,last_event_hash,custodian FROM v2_assets WHERE deployment_id=$1 AND asset_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(source_animal_id.to_vec())
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::Conflict("canonical v2 asset is not available for promotion".into()))?;
    let v2_sequence: i64 = asset.try_get("event_sequence").map_err(db_error)?;
    let v2_hash: Option<Vec<u8>> = asset.try_get("last_event_hash").map_err(db_error)?;
    let v2_custodian: Option<Vec<u8>> = asset.try_get("custodian").map_err(db_error)?;
    let source = sqlx::query(
        "SELECT event_sequence,last_event_hash,current_custodian FROM animals WHERE animal_id=$1",
    )
    .bind(source_animal_id.to_vec())
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;
    let source_sequence: i64 = source.try_get("event_sequence").map_err(db_error)?;
    let source_hash: Option<Vec<u8>> = source.try_get("last_event_hash").map_err(db_error)?;
    let source_custodian: Option<Vec<u8>> =
        source.try_get("current_custodian").map_err(db_error)?;
    if v2_sequence != source_sequence || v2_hash != source_hash || v2_custodian != source_custodian
    {
        let row = sqlx::query(
            "UPDATE v2_migration_candidates SET status='QUARANTINED',reason='v1 projection differs from canonical v2 AssetState',reviewed_at=now() WHERE run_id=$1 AND source_animal_id=$2 RETURNING source_animal_id,target_asset_id,status,reason,source_event_sequence,source_last_event_hash",
        )
        .bind(run_id)
        .bind(source_animal_id.to_vec())
        .fetch_one(&mut **tx)
        .await
        .map_err(db_error)?;
        return decode_candidate(&row);
    }
    let row = sqlx::query(
        "UPDATE v2_migration_candidates SET status='PROMOTED',reason='v1 projection matched finalized v2 AssetState',reviewed_at=now(),promoted_at=now() WHERE run_id=$1 AND source_animal_id=$2 RETURNING source_animal_id,target_asset_id,status,reason,source_event_sequence,source_last_event_hash",
    )
    .bind(run_id)
    .bind(source_animal_id.to_vec())
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;
    sqlx::query(
        "UPDATE v2_migration_runs SET promoted_count=(SELECT count(*) FROM v2_migration_candidates WHERE run_id=$1 AND status='PROMOTED'),rejected_count=(SELECT count(*) FROM v2_migration_candidates WHERE run_id=$1 AND status IN ('REJECTED','QUARANTINED')) WHERE run_id=$1",
    )
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    decode_candidate(&row)
}

fn decode_run(row: &PgRow) -> Result<MigrationRunRecord, ApiError> {
    Ok(MigrationRunRecord {
        run_id: row.try_get("run_id").map_err(db_error)?,
        deployment_id: fixed(row, "deployment_id")?,
        requested_by_party_id: optional_fixed(row, "requested_by_party_id")?,
        status: row.try_get("status").map_err(db_error)?,
        source_count: unsigned_i64(row, "source_count")?,
        eligible_count: unsigned_i64(row, "eligible_count")?,
        promoted_count: unsigned_i64(row, "promoted_count")?,
        rejected_count: unsigned_i64(row, "rejected_count")?,
    })
}

fn decode_candidate(row: &PgRow) -> Result<MigrationCandidateRecord, ApiError> {
    Ok(MigrationCandidateRecord {
        source_animal_id: fixed(row, "source_animal_id")?,
        target_asset_id: optional_fixed(row, "target_asset_id")?,
        status: row.try_get("status").map_err(db_error)?,
        reason: row.try_get("reason").map_err(db_error)?,
        source_event_sequence: unsigned_i64(row, "source_event_sequence")?,
        source_last_event_hash: optional_fixed(row, "source_last_event_hash")?,
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
fn unsigned_i64(row: &PgRow, column: &str) -> Result<u64, ApiError> {
    let value: i64 = row.try_get(column).map_err(db_error)?;
    u64::try_from(value).map_err(|_| ApiError::Internal)
}
fn map_constraint_error(error: sqlx::Error) -> ApiError {
    match error {
        sqlx::Error::Database(database) if database.code().as_deref() == Some("23505") => {
            ApiError::Conflict("migration run or candidate already exists".into())
        }
        _ => db_error(error),
    }
}
fn db_error(_: sqlx::Error) -> ApiError {
    ApiError::Unavailable("postgres query failed".into())
}
