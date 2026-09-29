//! Persistence for v2 domain-event evidence and public timeline reads.

use lastro_protocol::v2::DomainEventEnvelope;
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};

use crate::{
    domain::v2::VerifiedDomainObservation, error::ApiError, solana::rpc::CanonicalV2AssetState,
};

#[derive(Clone, Debug)]
pub struct DomainAnchorRecord {
    pub envelope: DomainEventEnvelope,
    pub envelope_bytes: [u8; 220],
    pub station_pubkey33: [u8; 33],
    pub station_signature64: [u8; 64],
    pub event_hash: [u8; 32],
    pub status: String,
    pub tx_signature: Option<String>,
}

/// Inserts evidence once. Any event_id/event_hash/subject-version collision must be exact.
pub async fn insert_or_match_exact(
    tx: &mut Transaction<'_, Postgres>,
    observation: &VerifiedDomainObservation,
) -> Result<bool, ApiError> {
    let e = &observation.envelope;
    let result = sqlx::query(
        r#"INSERT INTO v2_event_anchors(
            event_id,event_hash,deployment_id,subject_id,source_id,event_type,state_version,
            observed_at,expires_at,expected_previous_hash,payload_hash,envelope_bytes,
            station_pubkey,station_signature,status
        ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,'EVIDENCE_ACCEPTED')
        ON CONFLICT DO NOTHING"#,
    )
    .bind(e.event_id.to_vec())
    .bind(observation.event_hash.to_vec())
    .bind(e.deployment_id.to_vec())
    .bind(e.subject_id.to_vec())
    .bind(e.source_id.to_vec())
    .bind(i16::try_from(e.event_type).map_err(|_| ApiError::Internal)?)
    .bind(i64::try_from(e.state_version).map_err(|_| ApiError::Internal)?)
    .bind(e.observed_at)
    .bind(e.expires_at)
    .bind(e.expected_previous_hash.to_vec())
    .bind(e.payload_hash.to_vec())
    .bind(observation.envelope_bytes.to_vec())
    .bind(observation.station_pubkey33.to_vec())
    .bind(observation.station_signature64.to_vec())
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    if result.rows_affected() == 1 {
        return Ok(true);
    }

    let rows = sqlx::query(
        "SELECT * FROM v2_event_anchors WHERE event_id=$1 OR event_hash=$2 OR (subject_id=$3 AND state_version=$4)",
    )
    .bind(e.event_id.to_vec())
    .bind(observation.event_hash.to_vec())
    .bind(e.subject_id.to_vec())
    .bind(i64::try_from(e.state_version).map_err(|_| ApiError::Internal)?)
    .fetch_all(&mut **tx)
    .await
    .map_err(db_error)?;
    if rows.len() != 1 {
        return Err(ApiError::Conflict(
            "v2 event uniqueness collision is not an exact duplicate".into(),
        ));
    }
    let existing = decode(&rows[0])?;
    let exact = existing.event_hash == observation.event_hash
        && existing.envelope_bytes == observation.envelope_bytes
        && existing.station_pubkey33 == observation.station_pubkey33
        && existing.station_signature64 == observation.station_signature64;
    if exact {
        Ok(false)
    } else {
        Err(ApiError::Conflict(
            "v2 event duplicate contains divergent evidence".into(),
        ))
    }
}

pub async fn find_by_event_hash(
    pool: &PgPool,
    event_hash: [u8; 32],
) -> Result<Option<DomainAnchorRecord>, ApiError> {
    let row = sqlx::query("SELECT * FROM v2_event_anchors WHERE event_hash=$1")
        .bind(event_hash.to_vec())
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;
    row.as_ref().map(decode).transpose()
}

pub async fn mark_submitted(
    pool: &PgPool,
    event_hash: [u8; 32],
    tx_signature: &str,
) -> Result<(), ApiError> {
    let updated = sqlx::query(
        r#"UPDATE v2_event_anchors
              SET status='SUBMITTED', tx_signature=$2
            WHERE event_hash=$1
              AND status='EVIDENCE_ACCEPTED'
              AND (tx_signature IS NULL OR tx_signature=$2)"#,
    )
    .bind(event_hash.to_vec())
    .bind(tx_signature)
    .execute(pool)
    .await
    .map_err(db_error)?
    .rows_affected();
    if updated == 1 {
        return Ok(());
    }
    let current = find_by_event_hash(pool, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("v2 event not found".into()))?;
    if current.status == "SUBMITTED" && current.tx_signature.as_deref() == Some(tx_signature) {
        return Ok(());
    }
    Err(ApiError::Conflict(
        "v2 event is not eligible for this transaction submission".into(),
    ))
}

pub async fn mark_finalized(
    pool: &PgPool,
    event_hash: [u8; 32],
    tx_signature: &str,
) -> Result<(), ApiError> {
    let updated = sqlx::query(
        r#"UPDATE v2_event_anchors
              SET status='FINALIZED', tx_signature=$2
            WHERE event_hash=$1
              AND status IN ('SUBMITTED','FINALIZED')
              AND tx_signature=$2"#,
    )
    .bind(event_hash.to_vec())
    .bind(tx_signature)
    .execute(pool)
    .await
    .map_err(db_error)?
    .rows_affected();
    if updated == 1 {
        return Ok(());
    }
    Err(ApiError::Conflict(
        "v2 event finalization transaction does not match the accepted transaction".into(),
    ))
}

pub async fn mark_finalized_tx(
    tx: &mut Transaction<'_, Postgres>,
    event_hash: [u8; 32],
    tx_signature: &str,
) -> Result<(), ApiError> {
    let updated = sqlx::query(
        r#"UPDATE v2_event_anchors
              SET status='FINALIZED', tx_signature=$2
            WHERE event_hash=$1
              AND status IN ('SUBMITTED','FINALIZED')
              AND tx_signature=$2"#,
    )
    .bind(event_hash.to_vec())
    .bind(tx_signature)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?
    .rows_affected();
    if updated == 1 {
        Ok(())
    } else {
        Err(ApiError::Conflict(
            "v2 event finalization transaction does not match the accepted transaction".into(),
        ))
    }
}

pub async fn upsert_asset_projection_tx(
    tx: &mut Transaction<'_, Postgres>,
    asset: &CanonicalV2AssetState,
) -> Result<(), ApiError> {
    let result = sqlx::query(
        r#"INSERT INTO v2_assets(
            asset_id,deployment_id,asset_type,status,custodian,parent_root,lineage_root,
            current_lot_id,available_weight_grams,event_sequence,state_version,last_event_hash
        ) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
        ON CONFLICT (asset_id) DO UPDATE SET
            asset_type=EXCLUDED.asset_type,
            status=EXCLUDED.status,
            custodian=EXCLUDED.custodian,
            parent_root=EXCLUDED.parent_root,
            lineage_root=EXCLUDED.lineage_root,
            current_lot_id=EXCLUDED.current_lot_id,
            available_weight_grams=EXCLUDED.available_weight_grams,
            event_sequence=EXCLUDED.event_sequence,
            state_version=EXCLUDED.state_version,
            last_event_hash=EXCLUDED.last_event_hash,
            updated_at=now()
        WHERE v2_assets.deployment_id=EXCLUDED.deployment_id"#,
    )
    .bind(asset.asset_id.to_vec())
    .bind(asset.deployment_id.to_vec())
    .bind(i16::from(asset.asset_type))
    .bind(i16::from(asset.status))
    .bind(asset.custodian.to_bytes().to_vec())
    .bind(asset.parent_root.to_vec())
    .bind(asset.lineage_root.to_vec())
    .bind(asset.current_lot_id.to_vec())
    .bind(i64::try_from(asset.available_weight_grams).map_err(|_| ApiError::Internal)?)
    .bind(i64::try_from(asset.event_sequence).map_err(|_| ApiError::Internal)?)
    .bind(i64::try_from(asset.state_version).map_err(|_| ApiError::Internal)?)
    .bind(asset.last_event_hash.to_vec())
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    if result.rows_affected() == 0 {
        return Err(ApiError::Conflict(
            "v2 AssetState belongs to a different deployment projection".into(),
        ));
    }
    Ok(())
}

pub async fn list_by_subject(
    pool: &PgPool,
    deployment_id: [u8; 32],
    subject_id: [u8; 32],
) -> Result<Vec<DomainAnchorRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT * FROM v2_event_anchors WHERE deployment_id=$1 AND subject_id=$2 ORDER BY state_version ASC, event_id ASC LIMIT 257",
    )
    .bind(deployment_id.to_vec())
    .bind(subject_id.to_vec())
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    if rows.len() > 256 {
        return Err(ApiError::Conflict(
            "v2 timeline exceeds the public response limit".into(),
        ));
    }
    rows.iter().map(decode).collect()
}

fn decode(row: &PgRow) -> Result<DomainAnchorRecord, ApiError> {
    let envelope_bytes: [u8; 220] = fixed(row, "envelope_bytes")?;
    let envelope = DomainEventEnvelope::decode(&envelope_bytes).map_err(|_| ApiError::Internal)?;
    let event_hash: [u8; 32] = fixed(row, "event_hash")?;
    if envelope.event_hash().map_err(|_| ApiError::Internal)? != event_hash {
        return Err(ApiError::Internal);
    }
    Ok(DomainAnchorRecord {
        envelope,
        envelope_bytes,
        station_pubkey33: fixed(row, "station_pubkey")?,
        station_signature64: fixed(row, "station_signature")?,
        event_hash,
        status: row.try_get("status").map_err(db_error)?,
        tx_signature: row.try_get("tx_signature").map_err(db_error)?,
    })
}

fn fixed<const N: usize>(row: &PgRow, column: &str) -> Result<[u8; N], ApiError> {
    let value: Vec<u8> = row.try_get(column).map_err(db_error)?;
    value.try_into().map_err(|_| ApiError::Internal)
}

fn db_error(_error: sqlx::Error) -> ApiError {
    ApiError::Unavailable("postgres query failed".into())
}
