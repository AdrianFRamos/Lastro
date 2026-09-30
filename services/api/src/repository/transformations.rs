//! Public projection for transformation manifests and lineage edges.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use lastro_protocol::v2::{constants::HASH_DOMAIN_TRANSFORMATION, hash::domain_hash};
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};

use crate::error::ApiError;

#[derive(Clone, Debug)]
pub struct TransformationRecord {
    pub transformation_id: [u8; 32],
    pub deployment_id: [u8; 32],
    pub facility_id: [u8; 32],
    pub transformation_type: u16,
    pub input_root: [u8; 32],
    pub output_root: [u8; 32],
    pub input_count: u32,
    pub output_count: u32,
    pub input_weight_grams: u64,
    pub output_weight_grams: u64,
    pub byproduct_weight_grams: u64,
    pub loss_weight_grams: u64,
    pub tolerance_basis_points: u16,
    pub manifest_nonce: u64,
    pub manifest_hash: [u8; 32],
    pub manifest_bytes: [u8; 188],
    pub status: String,
    pub sequence: u64,
    pub expires_at: i64,
    pub tx_signature: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LineageEdgeRecord {
    pub transformation_id: [u8; 32],
    pub parent_asset_id: [u8; 32],
    pub child_asset_id: [u8; 32],
    pub role: u16,
    pub position: u32,
    pub quantity: u64,
    pub weight_grams: u64,
}

pub async fn find_transformation(
    pool: &PgPool,
    deployment_id: [u8; 32],
    transformation_id: [u8; 32],
) -> Result<Option<TransformationRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT * FROM v2_transformations WHERE deployment_id=$1 AND transformation_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(transformation_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_transformation).transpose()
}

pub async fn list_lineage(
    pool: &PgPool,
    deployment_id: [u8; 32],
    asset_id: [u8; 32],
) -> Result<Vec<LineageEdgeRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT transformation_id,parent_asset_id,child_asset_id,role,position,quantity,weight_grams FROM v2_lineage_edges WHERE deployment_id=$1 AND (parent_asset_id=$2 OR child_asset_id=$2) ORDER BY created_at ASC, transformation_id ASC, position ASC LIMIT 257",
    )
    .bind(deployment_id.to_vec())
    .bind(asset_id.to_vec())
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    if rows.len() > 256 {
        return Err(ApiError::Conflict(
            "lineage exceeds the public response limit".into(),
        ));
    }
    rows.iter().map(decode_edge).collect()
}

pub struct NewLineageEdge {
    pub parent_asset_id: [u8; 32],
    pub child_asset_id: [u8; 32],
    pub role: u8,
    pub position: u32,
    pub quantity: u64,
    pub weight_grams: u64,
}

/// Inserts a validated manifest (status OPEN) and its append-only lineage edges.
/// Inputs link `input -> transformation`; outputs link `transformation -> output`.
pub async fn insert_transformation_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    manifest: &lastro_protocol::v2::TransformationManifest,
    manifest_bytes: &[u8; 188],
    manifest_hash: [u8; 32],
    edges: &[NewLineageEdge],
) -> Result<TransformationRecord, ApiError> {
    let facility_active = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM v2_facilities WHERE deployment_id=$1 AND facility_id=$2 AND status='ACTIVE')",
    )
    .bind(deployment_id.to_vec())
    .bind(manifest.facility_id.to_vec())
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;
    if !facility_active {
        return Err(ApiError::Conflict(
            "facility is not active in this deployment".into(),
        ));
    }
    let int = |value: u64| {
        i64::try_from(value).map_err(|_| ApiError::Validation("value exceeds i64".into()))
    };
    let inserted = sqlx::query(
        "INSERT INTO v2_transformations(transformation_id,deployment_id,facility_id,transformation_type,input_root,output_root,input_count,output_count,input_weight_grams,output_weight_grams,byproduct_weight_grams,loss_weight_grams,tolerance_basis_points,manifest_nonce,manifest_hash,manifest_bytes,status,sequence,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,'OPEN',0,$17) ON CONFLICT (transformation_id) DO NOTHING",
    )
    .bind(manifest.transformation_id.to_vec())
    .bind(deployment_id.to_vec())
    .bind(manifest.facility_id.to_vec())
    .bind(i16::try_from(manifest.transformation_type).map_err(|_| ApiError::Validation("transformationType is too large".into()))?)
    .bind(manifest.input_root.to_vec())
    .bind(manifest.output_root.to_vec())
    .bind(i32::try_from(manifest.input_count).map_err(|_| ApiError::Internal)?)
    .bind(i32::try_from(manifest.output_count).map_err(|_| ApiError::Internal)?)
    .bind(int(manifest.mass.input_weight_grams)?)
    .bind(int(manifest.mass.output_weight_grams)?)
    .bind(int(manifest.mass.byproduct_weight_grams)?)
    .bind(int(manifest.mass.loss_weight_grams)?)
    .bind(i32::from(manifest.mass.tolerance_basis_points))
    .bind(int(manifest.manifest_nonce)?)
    .bind(manifest_hash.to_vec())
    .bind(manifest_bytes.to_vec())
    .bind(manifest.expires_at)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;

    if inserted.rows_affected() == 1 {
        for edge in edges {
            sqlx::query(
                "INSERT INTO v2_lineage_edges(deployment_id,transformation_id,parent_asset_id,child_asset_id,role,position,quantity,weight_grams) VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
            )
            .bind(deployment_id.to_vec())
            .bind(manifest.transformation_id.to_vec())
            .bind(edge.parent_asset_id.to_vec())
            .bind(edge.child_asset_id.to_vec())
            .bind(i16::from(edge.role))
            .bind(i32::try_from(edge.position).map_err(|_| ApiError::Validation("position exceeds i32".into()))?)
            .bind(int(edge.quantity)?)
            .bind(int(edge.weight_grams)?)
            .execute(&mut **tx)
            .await
            .map_err(db_error)?;
        }
    }

    let row = sqlx::query(
        "SELECT * FROM v2_transformations WHERE deployment_id=$1 AND transformation_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(manifest.transformation_id.to_vec())
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::Conflict("transformation belongs to another deployment".into()))?;
    let record = decode_transformation(&row)?;
    // Idempotent only for the identical manifest; a divergent retry is a conflict.
    if record.manifest_hash != manifest_hash {
        return Err(ApiError::Conflict(
            "transformation id already registered with a different manifest".into(),
        ));
    }
    Ok(record)
}

/// Owner wallet of an ACTIVE facility, used to rebuild the facility-signed finalize tx.
pub async fn facility_owner_wallet(
    pool: &PgPool,
    deployment_id: [u8; 32],
    facility_id: [u8; 32],
) -> Result<Option<[u8; 32]>, ApiError> {
    let wallet: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT p.wallet FROM v2_facilities f JOIN v2_parties p ON p.party_id=f.owner_party_id WHERE f.deployment_id=$1 AND f.facility_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(facility_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    wallet
        .map(|value| value.try_into().map_err(|_| ApiError::Internal))
        .transpose()
}

/// Mirrors the finalized on-chain status. Evidence columns are immutable (DB trigger).
pub async fn update_status(
    pool: &PgPool,
    deployment_id: [u8; 32],
    transformation_id: [u8; 32],
    status: &str,
    sequence: u64,
    tx_signature: Option<&str>,
) -> Result<TransformationRecord, ApiError> {
    let row = sqlx::query(
        "UPDATE v2_transformations SET status=$3,sequence=$4,tx_signature=COALESCE($5,tx_signature) WHERE deployment_id=$1 AND transformation_id=$2 AND status NOT IN ('FINALIZED','ABORTED','EXPIRED') RETURNING *",
    )
    .bind(deployment_id.to_vec())
    .bind(transformation_id.to_vec())
    .bind(status)
    .bind(i64::try_from(sequence).map_err(|_| ApiError::Internal)?)
    .bind(tx_signature)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::Conflict("transformation is already terminal".into()))?;
    decode_transformation(&row)
}

fn decode_transformation(row: &PgRow) -> Result<TransformationRecord, ApiError> {
    let manifest_bytes: [u8; 188] = fixed(row, "manifest_bytes")?;
    let manifest_hash: [u8; 32] = fixed(row, "manifest_hash")?;
    let tolerance_basis_points = u16::try_from(unsigned_i32(row, "tolerance_basis_points")?)
        .map_err(|_| ApiError::Internal)?;
    if domain_hash(HASH_DOMAIN_TRANSFORMATION, &manifest_bytes) != manifest_hash {
        return Err(ApiError::Internal);
    }
    Ok(TransformationRecord {
        transformation_id: fixed(row, "transformation_id")?,
        deployment_id: fixed(row, "deployment_id")?,
        facility_id: fixed(row, "facility_id")?,
        transformation_type: unsigned_i16(row, "transformation_type")?,
        input_root: fixed(row, "input_root")?,
        output_root: fixed(row, "output_root")?,
        input_count: unsigned_i32(row, "input_count")?,
        output_count: unsigned_i32(row, "output_count")?,
        input_weight_grams: unsigned_i64(row, "input_weight_grams")?,
        output_weight_grams: unsigned_i64(row, "output_weight_grams")?,
        byproduct_weight_grams: unsigned_i64(row, "byproduct_weight_grams")?,
        loss_weight_grams: unsigned_i64(row, "loss_weight_grams")?,
        tolerance_basis_points,
        manifest_nonce: unsigned_i64(row, "manifest_nonce")?,
        manifest_hash,
        manifest_bytes,
        status: row.try_get("status").map_err(db_error)?,
        sequence: unsigned_i64(row, "sequence")?,
        expires_at: row.try_get("expires_at").map_err(db_error)?,
        tx_signature: row.try_get("tx_signature").map_err(db_error)?,
    })
}

fn decode_edge(row: &PgRow) -> Result<LineageEdgeRecord, ApiError> {
    Ok(LineageEdgeRecord {
        transformation_id: fixed(row, "transformation_id")?,
        parent_asset_id: fixed(row, "parent_asset_id")?,
        child_asset_id: fixed(row, "child_asset_id")?,
        role: unsigned_i16(row, "role")?,
        position: unsigned_i32(row, "position")?,
        quantity: unsigned_i64(row, "quantity")?,
        weight_grams: unsigned_i64(row, "weight_grams")?,
    })
}

fn fixed<const N: usize>(row: &PgRow, column: &str) -> Result<[u8; N], ApiError> {
    let value: Vec<u8> = row.try_get(column).map_err(db_error)?;
    value.try_into().map_err(|_| ApiError::Internal)
}

fn unsigned_i16(row: &PgRow, column: &str) -> Result<u16, ApiError> {
    let value: i16 = row.try_get(column).map_err(db_error)?;
    u16::try_from(value).map_err(|_| ApiError::Internal)
}

fn unsigned_i32(row: &PgRow, column: &str) -> Result<u32, ApiError> {
    let value: i32 = row.try_get(column).map_err(db_error)?;
    u32::try_from(value).map_err(|_| ApiError::Internal)
}

fn unsigned_i64(row: &PgRow, column: &str) -> Result<u64, ApiError> {
    let value: i64 = row.try_get(column).map_err(db_error)?;
    u64::try_from(value).map_err(|_| ApiError::Internal)
}

pub fn manifest_base64(record: &TransformationRecord) -> String {
    BASE64.encode(record.manifest_bytes)
}

fn db_error(_error: sqlx::Error) -> ApiError {
    ApiError::Unavailable("postgres query failed".into())
}
