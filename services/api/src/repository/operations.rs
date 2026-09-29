//! Operational v2 projections: parties, facilities, lots and custody.
//!
//! These rows are not canonical by themselves. Handlers append an audit record and
//! later on-chain adapters are expected to bind state transitions to domain events.

use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{error::ApiError, model::LotAssetInput};

#[derive(Clone, Debug)]
pub struct PartyRecord {
    pub party_id: [u8; 32],
    pub deployment_id: [u8; 32],
    pub legal_name: String,
    pub tax_id_hash: Option<[u8; 32]>,
    pub wallet: [u8; 32],
    pub role: u16,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct FacilityRecord {
    pub facility_id: [u8; 32],
    pub deployment_id: [u8; 32],
    pub owner_party_id: [u8; 32],
    pub facility_type: u16,
    pub display_name: String,
    pub credential_hash: [u8; 32],
    pub valid_from: i64,
    pub valid_until: i64,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct LotRecord {
    pub lot_id: [u8; 32],
    pub deployment_id: [u8; 32],
    pub facility_id: [u8; 32],
    pub owner_party_id: [u8; 32],
    pub external_reference: Option<String>,
    pub head_count: u32,
    pub live_weight_grams: u64,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct LotAssetRecord {
    pub asset_id: [u8; 32],
    pub quantity: u64,
    pub weight_grams: u64,
    pub role: String,
}

#[derive(Clone, Debug)]
pub struct CustodyTransferRecord {
    pub transfer_id: Uuid,
    pub deployment_id: [u8; 32],
    pub asset_id: [u8; 32],
    pub from_party_id: Option<[u8; 32]>,
    pub to_party_id: [u8; 32],
    pub from_facility_id: Option<[u8; 32]>,
    pub to_facility_id: [u8; 32],
    pub reason: String,
    pub status: String,
    pub event_id: Option<[u8; 32]>,
    pub tx_signature: Option<String>,
    pub created_by_party_id: Option<[u8; 32]>,
    pub accepted_by_party_id: Option<[u8; 32]>,
}

#[derive(Clone, Debug)]
pub struct AuthorityGrantRecord {
    pub grant_id: Uuid,
    pub deployment_id: [u8; 32],
    pub party_id: [u8; 32],
    pub facility_id: Option<[u8; 32]>,
    pub capability: String,
    pub status: String,
    pub granted_by_party_id: Option<[u8; 32]>,
    pub valid_from: i64,
    pub valid_until: i64,
    pub reason: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AuditRecord {
    pub audit_id: i64,
    pub deployment_id: [u8; 32],
    pub actor_party_id: Option<[u8; 32]>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<[u8; 32]>,
    pub payload: serde_json::Value,
    pub created_at: OffsetDateTime,
}

pub async fn find_party(
    pool: &PgPool,
    deployment_id: [u8; 32],
    party_id: [u8; 32],
) -> Result<Option<PartyRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT party_id,deployment_id,legal_name,tax_id_hash,wallet,role,status FROM v2_parties WHERE deployment_id=$1 AND party_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(party_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_party).transpose()
}

pub async fn find_facility(
    pool: &PgPool,
    deployment_id: [u8; 32],
    facility_id: [u8; 32],
) -> Result<Option<FacilityRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT facility_id,deployment_id,owner_party_id,facility_type,display_name,credential_hash,valid_from,valid_until,status FROM v2_facilities WHERE deployment_id=$1 AND facility_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(facility_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_facility).transpose()
}

pub async fn find_lot(
    pool: &PgPool,
    deployment_id: [u8; 32],
    lot_id: [u8; 32],
) -> Result<Option<LotRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT lot_id,deployment_id,facility_id,owner_party_id,external_reference,head_count,live_weight_grams,status FROM v2_lots WHERE deployment_id=$1 AND lot_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(lot_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_lot).transpose()
}

pub async fn list_lot_assets(
    pool: &PgPool,
    deployment_id: [u8; 32],
    lot_id: [u8; 32],
) -> Result<Vec<LotAssetRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT asset_id,quantity,weight_grams,role FROM v2_lot_assets WHERE deployment_id=$1 AND lot_id=$2 ORDER BY asset_id",
    )
    .bind(deployment_id.to_vec())
    .bind(lot_id.to_vec())
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    rows.iter().map(decode_lot_asset).collect()
}

pub async fn find_transfer(
    pool: &PgPool,
    deployment_id: [u8; 32],
    transfer_id: Uuid,
) -> Result<Option<CustodyTransferRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT transfer_id,deployment_id,asset_id,from_party_id,to_party_id,from_facility_id,to_facility_id,reason,status,event_id,tx_signature,created_by_party_id,accepted_by_party_id FROM v2_custody_transfers WHERE deployment_id=$1 AND transfer_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(transfer_id)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_transfer).transpose()
}

pub async fn insert_party_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    party_id: [u8; 32],
    legal_name: &str,
    tax_id_hash: Option<[u8; 32]>,
    wallet: [u8; 32],
    role: u16,
) -> Result<PartyRecord, ApiError> {
    let row = sqlx::query(
        "INSERT INTO v2_parties(party_id,deployment_id,legal_name,tax_id_hash,wallet,role,status) VALUES($1,$2,$3,$4,$5,$6,'ACTIVE') RETURNING party_id,deployment_id,legal_name,tax_id_hash,wallet,role,status",
    )
    .bind(party_id.to_vec())
    .bind(deployment_id.to_vec())
    .bind(legal_name)
    .bind(tax_id_hash.map(|value| value.to_vec()))
    .bind(wallet.to_vec())
    .bind(i16::try_from(role).map_err(|_| ApiError::Validation("role is out of range".into()))?)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_constraint_error)?;
    decode_party(&row)
}

pub async fn insert_facility_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    facility_id: [u8; 32],
    owner_party_id: [u8; 32],
    facility_type: u16,
    display_name: &str,
    credential_hash: [u8; 32],
    valid_from: i64,
    valid_until: i64,
) -> Result<FacilityRecord, ApiError> {
    ensure_active_party_tx(tx, deployment_id, owner_party_id).await?;
    let row = sqlx::query(
        "INSERT INTO v2_facilities(facility_id,deployment_id,owner_party_id,facility_type,display_name,credential_hash,valid_from,valid_until,status) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'ACTIVE') RETURNING facility_id,deployment_id,owner_party_id,facility_type,display_name,credential_hash,valid_from,valid_until,status",
    )
    .bind(facility_id.to_vec())
    .bind(deployment_id.to_vec())
    .bind(owner_party_id.to_vec())
    .bind(i16::try_from(facility_type).map_err(|_| ApiError::Validation("facilityType is out of range".into()))?)
    .bind(display_name)
    .bind(credential_hash.to_vec())
    .bind(valid_from)
    .bind(valid_until)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_constraint_error)?;
    decode_facility(&row)
}

pub async fn insert_lot_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    lot_id: [u8; 32],
    facility_id: [u8; 32],
    owner_party_id: [u8; 32],
    external_reference: Option<&str>,
    head_count: u32,
    live_weight_grams: u64,
    assets: &[LotAssetInput],
) -> Result<LotRecord, ApiError> {
    if assets.is_empty() || assets.len() > 512 {
        return Err(ApiError::Validation(
            "lot must contain between 1 and 512 assets".into(),
        ));
    }
    if assets
        .iter()
        .map(|asset| asset.asset_id.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len()
        != assets.len()
    {
        return Err(ApiError::Validation("lot assets must be unique".into()));
    }
    ensure_active_party_tx(tx, deployment_id, owner_party_id).await?;
    ensure_active_facility_tx(tx, deployment_id, facility_id).await?;

    let row = sqlx::query(
        "INSERT INTO v2_lots(lot_id,deployment_id,facility_id,owner_party_id,external_reference,head_count,live_weight_grams,status) VALUES($1,$2,$3,$4,$5,$6,$7,'ACTIVE') RETURNING lot_id,deployment_id,facility_id,owner_party_id,external_reference,head_count,live_weight_grams,status",
    )
    .bind(lot_id.to_vec())
    .bind(deployment_id.to_vec())
    .bind(facility_id.to_vec())
    .bind(owner_party_id.to_vec())
    .bind(external_reference)
    .bind(i32::try_from(head_count).map_err(|_| ApiError::Validation("headCount is out of range".into()))?)
    .bind(i64::try_from(live_weight_grams).map_err(|_| ApiError::Validation("liveWeightGrams is out of range".into()))?)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_constraint_error)?;

    for asset in assets {
        let asset_id = crate::model::parse_hex32("assetId", &asset.asset_id)?;
        let role = normalize_asset_role(&asset.role)?;
        let quantity = i64::try_from(asset.quantity)
            .map_err(|_| ApiError::Validation("asset quantity is out of range".into()))?;
        let weight = i64::try_from(asset.weight_grams)
            .map_err(|_| ApiError::Validation("asset weight is out of range".into()))?;
        if quantity == 0 || weight == 0 {
            return Err(ApiError::Validation(
                "asset quantity and weight must be positive".into(),
            ));
        }
        let row = sqlx::query(
            "SELECT asset_type,available_weight_grams FROM v2_assets WHERE deployment_id=$1 AND asset_id=$2 FOR UPDATE",
        )
        .bind(deployment_id.to_vec())
        .bind(asset_id.to_vec())
        .fetch_optional(&mut **tx)
        .await
        .map_err(db_error)?
        .ok_or_else(|| ApiError::NotFound("asset not found in this deployment".into()))?;
        let asset_type: i16 = row.try_get("asset_type").map_err(db_error)?;
        let available: i64 = row.try_get("available_weight_grams").map_err(db_error)?;
        if !role_matches_asset_type(&role, asset_type) {
            return Err(ApiError::Conflict(
                "lot asset role does not match canonical asset type".into(),
            ));
        }
        if weight > available {
            return Err(ApiError::Conflict(
                "lot asset weight exceeds canonical available weight".into(),
            ));
        }
        sqlx::query(
            "INSERT INTO v2_lot_assets(deployment_id,lot_id,asset_id,quantity,weight_grams,role) VALUES($1,$2,$3,$4,$5,$6)",
        )
        .bind(deployment_id.to_vec())
        .bind(lot_id.to_vec())
        .bind(asset_id.to_vec())
        .bind(quantity)
        .bind(weight)
        .bind(role)
        .execute(&mut **tx)
        .await
        .map_err(map_constraint_error)?;
    }
    decode_lot(&row)
}

pub async fn insert_transfer_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    transfer_id: Uuid,
    asset_id: [u8; 32],
    from_party_id: Option<[u8; 32]>,
    to_party_id: [u8; 32],
    from_facility_id: Option<[u8; 32]>,
    to_facility_id: [u8; 32],
    reason: &str,
    created_by_party_id: Option<[u8; 32]>,
) -> Result<CustodyTransferRecord, ApiError> {
    ensure_active_party_tx(tx, deployment_id, to_party_id).await?;
    ensure_active_facility_tx(tx, deployment_id, to_facility_id).await?;
    if let Some(party_id) = from_party_id {
        ensure_active_party_tx(tx, deployment_id, party_id).await?;
    }
    if let Some(facility_id) = from_facility_id {
        ensure_active_facility_tx(tx, deployment_id, facility_id).await?;
    }
    if let Some(party_id) = created_by_party_id {
        ensure_active_party_tx(tx, deployment_id, party_id).await?;
    }
    let asset_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM v2_assets WHERE deployment_id=$1 AND asset_id=$2)",
    )
    .bind(deployment_id.to_vec())
    .bind(asset_id.to_vec())
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;
    if !asset_exists {
        return Err(ApiError::NotFound("asset not found in this deployment".into()));
    }
    let row = sqlx::query(
        "INSERT INTO v2_custody_transfers(transfer_id,deployment_id,asset_id,from_party_id,to_party_id,from_facility_id,to_facility_id,reason,status,created_by_party_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'PROPOSED',$9) RETURNING transfer_id,deployment_id,asset_id,from_party_id,to_party_id,from_facility_id,to_facility_id,reason,status,event_id,tx_signature,created_by_party_id,accepted_by_party_id",
    )
    .bind(transfer_id)
    .bind(deployment_id.to_vec())
    .bind(asset_id.to_vec())
    .bind(from_party_id.map(|value| value.to_vec()))
    .bind(to_party_id.to_vec())
    .bind(from_facility_id.map(|value| value.to_vec()))
    .bind(to_facility_id.to_vec())
    .bind(reason)
    .bind(created_by_party_id.map(|value| value.to_vec()))
    .fetch_one(&mut **tx)
    .await
    .map_err(map_constraint_error)?;
    decode_transfer(&row)
}

pub async fn accept_transfer_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    transfer_id: Uuid,
    accepted_by_party_id: [u8; 32],
) -> Result<CustodyTransferRecord, ApiError> {
    ensure_active_party_tx(tx, deployment_id, accepted_by_party_id).await?;
    let row = sqlx::query(
        "UPDATE v2_custody_transfers SET status='ACCEPTED',accepted_by_party_id=$3,accepted_at=now() WHERE deployment_id=$1 AND transfer_id=$2 AND to_party_id=$3 AND status='PROPOSED' RETURNING transfer_id,deployment_id,asset_id,from_party_id,to_party_id,from_facility_id,to_facility_id,reason,status,event_id,tx_signature,created_by_party_id,accepted_by_party_id",
    )
    .bind(deployment_id.to_vec())
    .bind(transfer_id)
    .bind(accepted_by_party_id.to_vec())
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or_else(|| {
        ApiError::Conflict("custody transfer is not pending for this recipient".into())
    })?;
    decode_transfer(&row)
}

pub async fn append_audit_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    actor_party_id: Option<[u8; 32]>,
    actor_wallet: Option<[u8; 32]>,
    action: &str,
    resource_type: &str,
    resource_id: Option<[u8; 32]>,
    payload: serde_json::Value,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO v2_audit_log(deployment_id,actor_party_id,actor_wallet,action,resource_type,resource_id,payload) VALUES($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(deployment_id.to_vec())
    .bind(actor_party_id.map(|value| value.to_vec()))
    .bind(actor_wallet.map(|value| value.to_vec()))
    .bind(action)
    .bind(resource_type)
    .bind(resource_id.map(|value| value.to_vec()))
    .bind(payload)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    Ok(())
}

pub async fn insert_authority_grant_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    grant_id: Uuid,
    party_id: [u8; 32],
    facility_id: Option<[u8; 32]>,
    capability: &str,
    granted_by_party_id: Option<[u8; 32]>,
    valid_from: i64,
    valid_until: i64,
    reason: Option<&str>,
) -> Result<AuthorityGrantRecord, ApiError> {
    if !is_valid_capability(capability) || valid_from < 0 || valid_until <= valid_from {
        return Err(ApiError::Validation("authority grant is invalid".into()));
    }
    ensure_active_party_tx(tx, deployment_id, party_id).await?;
    if let Some(facility_id) = facility_id {
        ensure_active_facility_tx(tx, deployment_id, facility_id).await?;
    }
    if let Some(actor) = granted_by_party_id {
        ensure_active_party_tx(tx, deployment_id, actor).await?;
    }
    let row = sqlx::query(
        "INSERT INTO v2_authority_grants(grant_id,deployment_id,party_id,facility_id,capability,status,granted_by_party_id,valid_from,valid_until,reason) VALUES($1,$2,$3,$4,$5,'ACTIVE',$6,$7,$8,$9) RETURNING grant_id,deployment_id,party_id,facility_id,capability,status,granted_by_party_id,valid_from,valid_until,reason",
    )
    .bind(grant_id)
    .bind(deployment_id.to_vec())
    .bind(party_id.to_vec())
    .bind(facility_id.map(|value| value.to_vec()))
    .bind(capability)
    .bind(granted_by_party_id.map(|value| value.to_vec()))
    .bind(valid_from)
    .bind(valid_until)
    .bind(reason)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_constraint_error)?;
    decode_grant(&row)
}

pub async fn revoke_authority_grant_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    grant_id: Uuid,
) -> Result<AuthorityGrantRecord, ApiError> {
    let row = sqlx::query(
        "UPDATE v2_authority_grants SET status='REVOKED',updated_at=now() WHERE deployment_id=$1 AND grant_id=$2 AND status <> 'REVOKED' RETURNING grant_id,deployment_id,party_id,facility_id,capability,status,granted_by_party_id,valid_from,valid_until,reason",
    )
    .bind(deployment_id.to_vec())
    .bind(grant_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::Conflict("authority grant is already revoked or missing".into()))?;
    decode_grant(&row)
}

pub async fn set_party_status_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    party_id: [u8; 32],
    status: &str,
) -> Result<PartyRecord, ApiError> {
    if !matches!(status, "ACTIVE" | "SUSPENDED" | "REVOKED") {
        return Err(ApiError::Validation("party status is invalid".into()));
    }
    let row = sqlx::query(
        "UPDATE v2_parties SET status=$3,updated_at=now() WHERE deployment_id=$1 AND party_id=$2 AND NOT (status='REVOKED' AND $3 <> 'REVOKED') RETURNING party_id,deployment_id,legal_name,tax_id_hash,wallet,role,status",
    )
    .bind(deployment_id.to_vec())
    .bind(party_id.to_vec())
    .bind(status)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::Conflict("party is missing or already terminally revoked".into()))?;
    decode_party(&row)
}

pub async fn set_facility_status_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    facility_id: [u8; 32],
    status: &str,
) -> Result<FacilityRecord, ApiError> {
    if !matches!(status, "ACTIVE" | "SUSPENDED" | "REVOKED" | "EXPIRED") {
        return Err(ApiError::Validation("facility status is invalid".into()));
    }
    let row = sqlx::query(
        "UPDATE v2_facilities SET status=$3,updated_at=now() WHERE deployment_id=$1 AND facility_id=$2 AND NOT (status='REVOKED' AND $3 <> 'REVOKED') RETURNING facility_id,deployment_id,owner_party_id,facility_type,display_name,credential_hash,valid_from,valid_until,status",
    )
    .bind(deployment_id.to_vec())
    .bind(facility_id.to_vec())
    .bind(status)
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::Conflict("facility is missing or already terminally revoked".into()))?;
    decode_facility(&row)
}

pub async fn list_audit(
    pool: &PgPool,
    deployment_id: [u8; 32],
    limit: u32,
    offset: u32,
) -> Result<Vec<AuditRecord>, ApiError> {
    if limit == 0 || limit > 1000 {
        return Err(ApiError::Validation("audit limit must be between 1 and 1000".into()));
    }
    let rows = sqlx::query(
        "SELECT audit_id,deployment_id,actor_party_id,action,resource_type,resource_id,payload,created_at FROM v2_audit_log WHERE deployment_id=$1 ORDER BY audit_id DESC LIMIT $2 OFFSET $3",
    )
    .bind(deployment_id.to_vec())
    .bind(i64::from(limit))
    .bind(i64::from(offset))
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    rows.iter().map(decode_audit).collect()
}

fn is_valid_capability(value: &str) -> bool {
    matches!(
        value,
        "REGISTER_PARTY"
            | "REGISTER_FACILITY"
            | "REGISTER_LOT"
            | "ADMIT_PROCESSING"
            | "FINALIZE_PROCESSING"
            | "CREATE_SHIPMENT"
            | "ACCEPT_SHIPMENT"
            | "OPEN_RECALL"
            | "CLOSE_RECALL"
            | "READ_AUDIT"
    )
}

async fn ensure_active_party_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    party_id: [u8; 32],
) -> Result<(), ApiError> {
    let found = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM v2_parties WHERE deployment_id=$1 AND party_id=$2 AND status='ACTIVE')",
    )
    .bind(deployment_id.to_vec())
    .bind(party_id.to_vec())
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;
    if found {
        Ok(())
    } else {
        Err(ApiError::Conflict("party is not active in this deployment".into()))
    }
}

async fn ensure_active_facility_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    facility_id: [u8; 32],
) -> Result<(), ApiError> {
    let found = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM v2_facilities WHERE deployment_id=$1 AND facility_id=$2 AND status='ACTIVE')",
    )
    .bind(deployment_id.to_vec())
    .bind(facility_id.to_vec())
    .fetch_one(&mut **tx)
    .await
    .map_err(db_error)?;
    if found {
        Ok(())
    } else {
        Err(ApiError::Conflict("facility is not active in this deployment".into()))
    }
}

fn normalize_asset_role(value: &str) -> Result<&'static str, ApiError> {
    match value {
        "ANIMAL" => Ok("ANIMAL"),
        "CARCASS" => Ok("CARCASS"),
        "PRODUCT" => Ok("PRODUCT"),
        "BYPRODUCT" => Ok("BYPRODUCT"),
        _ => Err(ApiError::Validation(
            "asset role must be ANIMAL, CARCASS, PRODUCT or BYPRODUCT".into(),
        )),
    }
}

fn role_matches_asset_type(role: &str, asset_type: i16) -> bool {
    match role {
        "ANIMAL" => asset_type == 1,
        "CARCASS" => asset_type == 3,
        "PRODUCT" => matches!(asset_type, 4..=6),
        "BYPRODUCT" => asset_type == 7,
        _ => false,
    }
}

fn decode_party(row: &PgRow) -> Result<PartyRecord, ApiError> {
    Ok(PartyRecord {
        party_id: fixed(row, "party_id")?,
        deployment_id: fixed(row, "deployment_id")?,
        legal_name: row.try_get("legal_name").map_err(db_error)?,
        tax_id_hash: optional_fixed(row, "tax_id_hash")?,
        wallet: fixed(row, "wallet")?,
        role: unsigned_i16(row, "role")?,
        status: row.try_get("status").map_err(db_error)?,
    })
}

fn decode_facility(row: &PgRow) -> Result<FacilityRecord, ApiError> {
    Ok(FacilityRecord {
        facility_id: fixed(row, "facility_id")?,
        deployment_id: fixed(row, "deployment_id")?,
        owner_party_id: fixed(row, "owner_party_id")?,
        facility_type: unsigned_i16(row, "facility_type")?,
        display_name: row.try_get("display_name").map_err(db_error)?,
        credential_hash: fixed(row, "credential_hash")?,
        valid_from: row.try_get("valid_from").map_err(db_error)?,
        valid_until: row.try_get("valid_until").map_err(db_error)?,
        status: row.try_get("status").map_err(db_error)?,
    })
}

fn decode_lot(row: &PgRow) -> Result<LotRecord, ApiError> {
    Ok(LotRecord {
        lot_id: fixed(row, "lot_id")?,
        deployment_id: fixed(row, "deployment_id")?,
        facility_id: fixed(row, "facility_id")?,
        owner_party_id: fixed(row, "owner_party_id")?,
        external_reference: row.try_get("external_reference").map_err(db_error)?,
        head_count: unsigned_i32(row, "head_count")?,
        live_weight_grams: unsigned_i64(row, "live_weight_grams")?,
        status: row.try_get("status").map_err(db_error)?,
    })
}

fn decode_lot_asset(row: &PgRow) -> Result<LotAssetRecord, ApiError> {
    Ok(LotAssetRecord {
        asset_id: fixed(row, "asset_id")?,
        quantity: unsigned_i64(row, "quantity")?,
        weight_grams: unsigned_i64(row, "weight_grams")?,
        role: row.try_get("role").map_err(db_error)?,
    })
}

fn decode_transfer(row: &PgRow) -> Result<CustodyTransferRecord, ApiError> {
    Ok(CustodyTransferRecord {
        transfer_id: row.try_get("transfer_id").map_err(db_error)?,
        deployment_id: fixed(row, "deployment_id")?,
        asset_id: fixed(row, "asset_id")?,
        from_party_id: optional_fixed(row, "from_party_id")?,
        to_party_id: fixed(row, "to_party_id")?,
        from_facility_id: optional_fixed(row, "from_facility_id")?,
        to_facility_id: fixed(row, "to_facility_id")?,
        reason: row.try_get("reason").map_err(db_error)?,
        status: row.try_get("status").map_err(db_error)?,
        event_id: optional_fixed(row, "event_id")?,
        tx_signature: row.try_get("tx_signature").map_err(db_error)?,
        created_by_party_id: optional_fixed(row, "created_by_party_id")?,
        accepted_by_party_id: optional_fixed(row, "accepted_by_party_id")?,
    })
}

fn decode_grant(row: &PgRow) -> Result<AuthorityGrantRecord, ApiError> {
    Ok(AuthorityGrantRecord {
        grant_id: row.try_get("grant_id").map_err(db_error)?,
        deployment_id: fixed(row, "deployment_id")?,
        party_id: fixed(row, "party_id")?,
        facility_id: optional_fixed(row, "facility_id")?,
        capability: row.try_get("capability").map_err(db_error)?,
        status: row.try_get("status").map_err(db_error)?,
        granted_by_party_id: optional_fixed(row, "granted_by_party_id")?,
        valid_from: row.try_get("valid_from").map_err(db_error)?,
        valid_until: row.try_get("valid_until").map_err(db_error)?,
        reason: row.try_get("reason").map_err(db_error)?,
    })
}

fn decode_audit(row: &PgRow) -> Result<AuditRecord, ApiError> {
    Ok(AuditRecord {
        audit_id: row.try_get("audit_id").map_err(db_error)?,
        deployment_id: fixed(row, "deployment_id")?,
        actor_party_id: optional_fixed(row, "actor_party_id")?,
        action: row.try_get("action").map_err(db_error)?,
        resource_type: row.try_get("resource_type").map_err(db_error)?,
        resource_id: optional_fixed(row, "resource_id")?,
        payload: row.try_get("payload").map_err(db_error)?,
        created_at: row.try_get("created_at").map_err(db_error)?,
    })
}

fn fixed<const N: usize>(row: &PgRow, column: &str) -> Result<[u8; N], ApiError> {
    let value: Vec<u8> = row.try_get(column).map_err(db_error)?;
    value.try_into().map_err(|_| ApiError::Internal)
}

fn optional_fixed<const N: usize>(
    row: &PgRow,
    column: &str,
) -> Result<Option<[u8; N]>, ApiError> {
    let value: Option<Vec<u8>> = row.try_get(column).map_err(db_error)?;
    value
        .map(|bytes| bytes.try_into().map_err(|_| ApiError::Internal))
        .transpose()
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

fn map_constraint_error(error: sqlx::Error) -> ApiError {
    match error {
        sqlx::Error::Database(database) if database.code().as_deref() == Some("23505") => {
            ApiError::Conflict("operational identity already exists".into())
        }
        sqlx::Error::Database(database) if database.code().as_deref() == Some("23503") => {
            ApiError::Conflict("operational reference does not exist".into())
        }
        _ => db_error(error),
    }
}

fn db_error(_error: sqlx::Error) -> ApiError {
    ApiError::Unavailable("postgres query failed".into())
}
