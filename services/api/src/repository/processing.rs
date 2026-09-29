//! Durable operational projections for slaughter, processing, shipments and recall.
//!
//! A processing operation becomes FINALIZED only after its referenced canonical
//! transformation is FINALIZED and every declared item is present in its lineage.

use std::collections::{HashMap, VecDeque};

use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow};
use time::OffsetDateTime;

use crate::{error::ApiError, model::{ProcessingItemInput, ShipmentItemInput}};

#[derive(Clone, Debug)]
pub struct ProcessingRecord {
    pub operation_id: [u8; 32],
    pub deployment_id: [u8; 32],
    pub facility_id: [u8; 32],
    pub lot_id: Option<[u8; 32]>,
    pub transformation_id: Option<[u8; 32]>,
    pub operator_party_id: [u8; 32],
    pub operation_kind: String,
    pub status: String,
    pub notes: Option<String>,
    pub tx_signature: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProcessingItemRecord {
    pub position: u32,
    pub asset_id: Option<[u8; 32]>,
    pub direction: String,
    pub quantity: u64,
    pub weight_grams: u64,
}

#[derive(Clone, Debug)]
pub struct ShipmentRecord {
    pub shipment_id: [u8; 32],
    pub deployment_id: [u8; 32],
    pub origin_facility_id: [u8; 32],
    pub destination_facility_id: [u8; 32],
    pub carrier_party_id: [u8; 32],
    pub created_by_party_id: [u8; 32],
    pub status: String,
    pub planned_departure: Option<OffsetDateTime>,
    pub departed_at: Option<OffsetDateTime>,
    pub delivered_at: Option<OffsetDateTime>,
    pub notes: Option<String>,
    pub tx_signature: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ShipmentItemRecord {
    pub position: u32,
    pub asset_id: [u8; 32],
    pub quantity: u64,
    pub weight_grams: u64,
}

#[derive(Clone, Debug)]
pub struct RecallRecord {
    pub recall_id: [u8; 32],
    pub deployment_id: [u8; 32],
    pub opened_by_party_id: [u8; 32],
    pub scope_type: String,
    pub scope_id: [u8; 32],
    pub reason: String,
    pub status: String,
    pub snapshot_root: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct RecallMemberRecord {
    pub asset_id: [u8; 32],
    pub traversal_depth: u32,
    pub relation: String,
}

pub async fn find_processing(
    pool: &PgPool,
    deployment_id: [u8; 32],
    operation_id: [u8; 32],
) -> Result<Option<ProcessingRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT operation_id,deployment_id,facility_id,lot_id,transformation_id,operator_party_id,operation_kind,status,notes,tx_signature FROM v2_processing_operations WHERE deployment_id=$1 AND operation_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(operation_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_processing).transpose()
}

pub async fn list_processing_items(
    pool: &PgPool,
    deployment_id: [u8; 32],
    operation_id: [u8; 32],
) -> Result<Vec<ProcessingItemRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT position,asset_id,direction,quantity,weight_grams FROM v2_processing_items WHERE deployment_id=$1 AND operation_id=$2 ORDER BY position",
    )
    .bind(deployment_id.to_vec())
    .bind(operation_id.to_vec())
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    rows.iter().map(decode_processing_item).collect()
}

pub async fn find_shipment(
    pool: &PgPool,
    deployment_id: [u8; 32],
    shipment_id: [u8; 32],
) -> Result<Option<ShipmentRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT shipment_id,deployment_id,origin_facility_id,destination_facility_id,carrier_party_id,created_by_party_id,status,planned_departure,departed_at,delivered_at,notes,tx_signature FROM v2_shipments WHERE deployment_id=$1 AND shipment_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(shipment_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_shipment).transpose()
}

pub async fn list_shipment_items(
    pool: &PgPool,
    deployment_id: [u8; 32],
    shipment_id: [u8; 32],
) -> Result<Vec<ShipmentItemRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT position,asset_id,quantity,weight_grams FROM v2_shipment_items WHERE deployment_id=$1 AND shipment_id=$2 ORDER BY position",
    )
    .bind(deployment_id.to_vec())
    .bind(shipment_id.to_vec())
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    rows.iter().map(decode_shipment_item).collect()
}

pub async fn find_recall(
    pool: &PgPool,
    deployment_id: [u8; 32],
    recall_id: [u8; 32],
) -> Result<Option<RecallRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT recall_id,deployment_id,opened_by_party_id,scope_type,scope_id,reason,status,snapshot_root FROM v2_recalls WHERE deployment_id=$1 AND recall_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(recall_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    row.as_ref().map(decode_recall).transpose()
}

pub async fn list_recall_members(
    pool: &PgPool,
    deployment_id: [u8; 32],
    recall_id: [u8; 32],
) -> Result<Vec<RecallMemberRecord>, ApiError> {
    let rows = sqlx::query(
        "SELECT asset_id,traversal_depth,relation FROM v2_recall_members WHERE deployment_id=$1 AND recall_id=$2 ORDER BY traversal_depth,asset_id",
    )
    .bind(deployment_id.to_vec())
    .bind(recall_id.to_vec())
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    rows.iter().map(decode_recall_member).collect()
}

pub async fn insert_processing_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    operation_id: [u8; 32],
    facility_id: [u8; 32],
    lot_id: Option<[u8; 32]>,
    transformation_id: Option<[u8; 32]>,
    operator_party_id: [u8; 32],
    operation_kind: &str,
    notes: Option<&str>,
    items: &[ProcessingItemInput],
) -> Result<ProcessingRecord, ApiError> {
    validate_processing_kind(operation_kind)?;
    if items.is_empty() || items.len() > 512 {
        return Err(ApiError::Validation(
            "processing operation must contain between 1 and 512 items".into(),
        ));
    }
    ensure_active_party_tx(tx, deployment_id, operator_party_id).await?;
    ensure_active_facility_tx(tx, deployment_id, facility_id).await?;
    if let Some(lot_id) = lot_id {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM v2_lots WHERE deployment_id=$1 AND lot_id=$2)",
        )
        .bind(deployment_id.to_vec())
        .bind(lot_id.to_vec())
        .fetch_one(&mut **tx)
        .await
        .map_err(db_error)?;
        if !exists {
            return Err(ApiError::NotFound("processing lot not found".into()));
        }
    }
    if let Some(transformation_id) = transformation_id {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM v2_transformations WHERE deployment_id=$1 AND transformation_id=$2)",
        )
        .bind(deployment_id.to_vec())
        .bind(transformation_id.to_vec())
        .fetch_one(&mut **tx)
        .await
        .map_err(db_error)?;
        if !exists {
            return Err(ApiError::NotFound("processing transformation not found".into()));
        }
    }

    let mut input_weight = 0u128;
    let mut output_weight = 0u128;
    let mut byproduct_weight = 0u128;
    let mut loss_weight = 0u128;
    let mut seen = std::collections::HashSet::new();
    let op_row = sqlx::query(
        "INSERT INTO v2_processing_operations(operation_id,deployment_id,facility_id,lot_id,transformation_id,operator_party_id,operation_kind,status,notes) VALUES($1,$2,$3,$4,$5,$6,$7,'REGISTERED',$8) RETURNING operation_id,deployment_id,facility_id,lot_id,transformation_id,operator_party_id,operation_kind,status,notes,tx_signature",
    )
    .bind(operation_id.to_vec())
    .bind(deployment_id.to_vec())
    .bind(facility_id.to_vec())
    .bind(lot_id.map(|value| value.to_vec()))
    .bind(transformation_id.map(|value| value.to_vec()))
    .bind(operator_party_id.to_vec())
    .bind(operation_kind)
    .bind(notes)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_constraint_error)?;

    for (position, item) in items.iter().enumerate() {
        let direction = normalize_direction(&item.direction)?;
        if item.quantity == 0 || item.weight_grams == 0 {
            return Err(ApiError::Validation("processing quantity and weight must be positive".into()));
        }
        let asset_id = match item.asset_id.as_deref() {
            Some(value) => {
                if !seen.insert(value) {
                    return Err(ApiError::Validation("processing assets must be unique".into()));
                }
                let asset_id = crate::model::parse_hex32("assetId", value)?;
                let asset = sqlx::query(
                    "SELECT asset_type,available_weight_grams FROM v2_assets WHERE deployment_id=$1 AND asset_id=$2 FOR UPDATE",
                )
                .bind(deployment_id.to_vec())
                .bind(asset_id.to_vec())
                .fetch_optional(&mut **tx)
                .await
                .map_err(db_error)?
                .ok_or_else(|| ApiError::NotFound("processing asset not found".into()))?;
                let asset_type: i16 = asset.try_get("asset_type").map_err(db_error)?;
                let available: i64 = asset.try_get("available_weight_grams").map_err(db_error)?;
                if i64::try_from(item.weight_grams).map_err(|_| ApiError::Validation("weight is out of range".into()))? > available {
                    return Err(ApiError::Conflict("processing weight exceeds available asset weight".into()));
                }
                if !direction_matches_kind(direction, operation_kind, asset_type) {
                    return Err(ApiError::Conflict("processing direction does not match asset type and operation kind".into()));
                }
                Some(asset_id)
            }
            None if direction == "LOSS" => None,
            None => return Err(ApiError::Validation("only LOSS may omit assetId".into())),
        };
        match direction {
            "INPUT" => input_weight += u128::from(item.weight_grams),
            "OUTPUT" => output_weight += u128::from(item.weight_grams),
            "BYPRODUCT" => byproduct_weight += u128::from(item.weight_grams),
            "LOSS" => loss_weight += u128::from(item.weight_grams),
            _ => unreachable!(),
        }
        sqlx::query(
            "INSERT INTO v2_processing_items(deployment_id,operation_id,position,asset_id,direction,quantity,weight_grams) VALUES($1,$2,$3,$4,$5,$6,$7)",
        )
        .bind(deployment_id.to_vec())
        .bind(operation_id.to_vec())
        .bind(i32::try_from(position).map_err(|_| ApiError::Internal)?)
        .bind(asset_id.map(|value| value.to_vec()))
        .bind(direction)
        .bind(i64::try_from(item.quantity).map_err(|_| ApiError::Validation("quantity is out of range".into()))?)
        .bind(i64::try_from(item.weight_grams).map_err(|_| ApiError::Validation("weight is out of range".into()))?)
        .execute(&mut **tx)
        .await
        .map_err(map_constraint_error)?;
    }
    if input_weight == 0 || output_weight + byproduct_weight + loss_weight != input_weight {
        return Err(ApiError::Conflict(
            "processing mass balance must be exact before chain admission".into(),
        ));
    }
    decode_processing(&op_row)
}

pub async fn mark_processing_ready_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    operation_id: [u8; 32],
) -> Result<ProcessingRecord, ApiError> {
    let row = sqlx::query(
        "UPDATE v2_processing_operations SET status='READY_FOR_CHAIN',updated_at=now() WHERE deployment_id=$1 AND operation_id=$2 AND status IN ('REGISTERED','IN_PROGRESS') AND transformation_id IS NOT NULL RETURNING operation_id,deployment_id,facility_id,lot_id,transformation_id,operator_party_id,operation_kind,status,notes,tx_signature",
    )
    .bind(deployment_id.to_vec())
    .bind(operation_id.to_vec())
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::Conflict("operation needs a linked transformation and active status".into()))?;
    decode_processing(&row)
}

pub async fn finalize_processing_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    operation_id: [u8; 32],
) -> Result<ProcessingRecord, ApiError> {
    let operation = sqlx::query(
        "SELECT operation_id,deployment_id,facility_id,lot_id,transformation_id,operator_party_id,operation_kind,status,notes,tx_signature FROM v2_processing_operations WHERE deployment_id=$1 AND operation_id=$2 FOR UPDATE",
    )
    .bind(deployment_id.to_vec())
    .bind(operation_id.to_vec())
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::NotFound("processing operation not found".into()))?;
    let current = decode_processing(&operation)?;
    if current.status == "FINALIZED" {
        return Ok(current);
    }
    if current.status != "READY_FOR_CHAIN" {
        return Err(ApiError::Conflict("operation is not ready for canonical finalization".into()));
    }
    let transformation_id = current.transformation_id.ok_or_else(|| ApiError::Conflict("operation has no transformation".into()))?;
    let transformation = sqlx::query(
        "SELECT status,tx_signature FROM v2_transformations WHERE deployment_id=$1 AND transformation_id=$2",
    )
    .bind(deployment_id.to_vec())
    .bind(transformation_id.to_vec())
    .fetch_optional(&mut **tx)
    .await
    .map_err(db_error)?
    .ok_or_else(|| ApiError::NotFound("linked transformation not found".into()))?;
    let status: String = transformation.try_get("status").map_err(db_error)?;
    let tx_signature: Option<String> = transformation.try_get("tx_signature").map_err(db_error)?;
    if status != "FINALIZED" || tx_signature.is_none() {
        return Err(ApiError::Conflict("linked transformation is not finalized on Solana".into()));
    }
    let items = sqlx::query(
        "SELECT asset_id,direction FROM v2_processing_items WHERE deployment_id=$1 AND operation_id=$2 ORDER BY position",
    )
    .bind(deployment_id.to_vec())
    .bind(operation_id.to_vec())
    .fetch_all(&mut **tx)
    .await
    .map_err(db_error)?;
    for item in items {
        let asset_id: Option<Vec<u8>> = item.try_get("asset_id").map_err(db_error)?;
        let direction: String = item.try_get("direction").map_err(db_error)?;
        if direction == "LOSS" {
            if asset_id.is_some() {
                return Err(ApiError::Internal);
            }
            continue;
        }
        let asset_id = asset_id.ok_or(ApiError::Internal)?;
        let role = match direction.as_str() {
            "INPUT" => 1i16,
            "OUTPUT" => 2i16,
            "BYPRODUCT" => 3i16,
            _ => return Err(ApiError::Internal),
        };
        let exists = if role == 1 {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM v2_lineage_edges WHERE deployment_id=$1 AND transformation_id=$2 AND parent_asset_id=$3 AND role=$4)",
            )
            .bind(deployment_id.to_vec()).bind(transformation_id.to_vec()).bind(&asset_id).bind(role)
            .fetch_one(&mut **tx).await.map_err(db_error)?
        } else {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM v2_lineage_edges WHERE deployment_id=$1 AND transformation_id=$2 AND child_asset_id=$3 AND role=$4)",
            )
            .bind(deployment_id.to_vec()).bind(transformation_id.to_vec()).bind(&asset_id).bind(role)
            .fetch_one(&mut **tx).await.map_err(db_error)?
        };
        if !exists {
            return Err(ApiError::Conflict("processing item is not present in canonical transformation lineage".into()));
        }
    }
    let row = sqlx::query(
        "UPDATE v2_processing_operations SET status='FINALIZED',tx_signature=$3,finalized_at=now(),updated_at=now() WHERE deployment_id=$1 AND operation_id=$2 RETURNING operation_id,deployment_id,facility_id,lot_id,transformation_id,operator_party_id,operation_kind,status,notes,tx_signature",
    )
    .bind(deployment_id.to_vec()).bind(operation_id.to_vec()).bind(tx_signature)
    .fetch_one(&mut **tx).await.map_err(db_error)?;
    decode_processing(&row)
}

pub async fn insert_shipment_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    shipment_id: [u8; 32],
    origin_facility_id: [u8; 32],
    destination_facility_id: [u8; 32],
    carrier_party_id: [u8; 32],
    created_by_party_id: [u8; 32],
    planned_departure: Option<OffsetDateTime>,
    notes: Option<&str>,
    items: &[ShipmentItemInput],
) -> Result<ShipmentRecord, ApiError> {
    if items.is_empty() || items.len() > 256 {
        return Err(ApiError::Validation("shipment must contain between 1 and 256 items".into()));
    }
    ensure_active_facility_tx(tx, deployment_id, origin_facility_id).await?;
    ensure_active_facility_tx(tx, deployment_id, destination_facility_id).await?;
    ensure_active_party_tx(tx, deployment_id, carrier_party_id).await?;
    ensure_active_party_tx(tx, deployment_id, created_by_party_id).await?;
    let row = sqlx::query(
        "INSERT INTO v2_shipments(shipment_id,deployment_id,origin_facility_id,destination_facility_id,carrier_party_id,created_by_party_id,status,planned_departure,notes) VALUES($1,$2,$3,$4,$5,$6,'DRAFT',$7,$8) RETURNING shipment_id,deployment_id,origin_facility_id,destination_facility_id,carrier_party_id,created_by_party_id,status,planned_departure,departed_at,delivered_at,notes,tx_signature",
    )
    .bind(shipment_id.to_vec()).bind(deployment_id.to_vec()).bind(origin_facility_id.to_vec()).bind(destination_facility_id.to_vec()).bind(carrier_party_id.to_vec()).bind(created_by_party_id.to_vec()).bind(planned_departure).bind(notes)
    .fetch_one(&mut **tx).await.map_err(map_constraint_error)?;
    let mut seen = std::collections::HashSet::new();
    for (position, item) in items.iter().enumerate() {
        if !seen.insert(item.asset_id.as_str()) { return Err(ApiError::Validation("shipment assets must be unique".into())); }
        let asset_id = crate::model::parse_hex32("assetId", &item.asset_id)?;
        if item.quantity == 0 || item.weight_grams == 0 { return Err(ApiError::Validation("shipment quantity and weight must be positive".into())); }
        let asset = sqlx::query_scalar::<_, String>(
            "SELECT status FROM v2_assets WHERE deployment_id=$1 AND asset_id=$2",
        ).bind(deployment_id.to_vec()).bind(asset_id.to_vec()).fetch_optional(&mut **tx).await.map_err(db_error)?
            .ok_or_else(|| ApiError::NotFound("shipment asset not found".into()))?;
        if asset == "RECALLED" { return Err(ApiError::Conflict("recalled assets cannot be shipped".into())); }
        sqlx::query("INSERT INTO v2_shipment_items(deployment_id,shipment_id,position,asset_id,quantity,weight_grams) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(deployment_id.to_vec()).bind(shipment_id.to_vec()).bind(i32::try_from(position).map_err(|_| ApiError::Internal)?)
            .bind(asset_id.to_vec()).bind(i64::try_from(item.quantity).map_err(|_| ApiError::Validation("quantity is out of range".into()))?)
            .bind(i64::try_from(item.weight_grams).map_err(|_| ApiError::Validation("weight is out of range".into()))?)
            .execute(&mut **tx).await.map_err(map_constraint_error)?;
    }
    decode_shipment(&row)
}

pub async fn set_shipment_status_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    shipment_id: [u8; 32],
    next_status: &str,
) -> Result<ShipmentRecord, ApiError> {
    let allowed = match next_status { "DISPATCHED" | "IN_TRANSIT" | "DELIVERED" | "REJECTED" => true, _ => false };
    if !allowed { return Err(ApiError::Validation("invalid shipment status".into())); }
    let row = sqlx::query(
        "UPDATE v2_shipments SET status=$3,departed_at=CASE WHEN $3 IN ('DISPATCHED','IN_TRANSIT') AND departed_at IS NULL THEN now() ELSE departed_at END,delivered_at=CASE WHEN $3='DELIVERED' THEN now() ELSE delivered_at END,updated_at=now() WHERE deployment_id=$1 AND shipment_id=$2 AND ((status='DRAFT' AND $3='DISPATCHED') OR (status='DISPATCHED' AND $3 IN ('IN_TRANSIT','REJECTED')) OR (status='IN_TRANSIT' AND $3 IN ('DELIVERED','REJECTED'))) RETURNING shipment_id,deployment_id,origin_facility_id,destination_facility_id,carrier_party_id,created_by_party_id,status,planned_departure,departed_at,delivered_at,notes,tx_signature",
    ).bind(deployment_id.to_vec()).bind(shipment_id.to_vec()).bind(next_status).fetch_optional(&mut **tx).await.map_err(db_error)?
        .ok_or_else(|| ApiError::Conflict("invalid shipment lifecycle transition".into()))?;
    decode_shipment(&row)
}

pub async fn open_recall_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    recall_id: [u8; 32],
    opened_by_party_id: [u8; 32],
    scope_type: &str,
    scope_id: [u8; 32],
    reason: &str,
) -> Result<RecallRecord, ApiError> {
    validate_scope(scope_type)?;
    ensure_active_party_tx(tx, deployment_id, opened_by_party_id).await?;
    let seeds = seed_assets_tx(tx, deployment_id, scope_type, scope_id).await?;
    if seeds.is_empty() { return Err(ApiError::NotFound("recall scope has no assets".into())); }
    let edges = sqlx::query("SELECT parent_asset_id,child_asset_id FROM v2_lineage_edges WHERE deployment_id=$1 LIMIT 32769")
        .bind(deployment_id.to_vec()).fetch_all(&mut **tx).await.map_err(db_error)?;
    if edges.len() > 32768 { return Err(ApiError::Conflict("lineage is too large for a bounded recall snapshot".into())); }
    let mut graph: HashMap<[u8;32], Vec<([u8;32], &'static str)>> = HashMap::new();
    for edge in edges {
        let parent: [u8;32] = fixed(&edge, "parent_asset_id")?;
        let child: [u8;32] = fixed(&edge, "child_asset_id")?;
        graph.entry(parent).or_default().push((child, "DOWNSTREAM"));
        graph.entry(child).or_default().push((parent, "UPSTREAM"));
    }
    let mut members: HashMap<[u8;32], (u32, &'static str)> = HashMap::new();
    let mut queue = VecDeque::new();
    for seed in seeds { members.insert(seed, (0, "ROOT")); queue.push_back(seed); }
    while let Some(current) = queue.pop_front() {
        let (depth, _) = members[&current];
        if depth >= 64 { continue; }
        for (next, relation) in graph.get(&current).into_iter().flatten() {
            let candidate = (depth + 1, *relation);
            let replace = match members.get(next) { None => true, Some((old_depth, old_relation)) => candidate.0 < *old_depth || (candidate.0 == *old_depth && *old_relation != "ROOT" && *relation == "ROOT") };
            if replace { members.insert(*next, candidate); queue.push_back(*next); }
        }
    }
    let mut ordered: Vec<_> = members.iter().map(|(asset, (depth, relation))| (*asset, *depth, *relation)).collect();
    ordered.sort_by_key(|(asset, depth, relation)| (*asset, *depth, *relation));
    let mut hasher = Sha256::new();
    hasher.update(b"LASTRO_V2_RECALL_SNAPSHOT\0");
    for (asset, depth, relation) in &ordered { hasher.update(asset); hasher.update(depth.to_le_bytes()); hasher.update(relation.as_bytes()); hasher.update([0]); }
    let snapshot_root: [u8;32] = hasher.finalize().into();
    let row = sqlx::query("INSERT INTO v2_recalls(recall_id,deployment_id,opened_by_party_id,scope_type,scope_id,reason,status,snapshot_root) VALUES($1,$2,$3,$4,$5,$6,'OPEN',$7) RETURNING recall_id,deployment_id,opened_by_party_id,scope_type,scope_id,reason,status,snapshot_root")
        .bind(recall_id.to_vec()).bind(deployment_id.to_vec()).bind(opened_by_party_id.to_vec()).bind(scope_type).bind(scope_id.to_vec()).bind(reason).bind(snapshot_root.to_vec()).fetch_one(&mut **tx).await.map_err(map_constraint_error)?;
    for (asset, depth, relation) in ordered {
        sqlx::query("INSERT INTO v2_recall_members(deployment_id,recall_id,asset_id,traversal_depth,relation) VALUES($1,$2,$3,$4,$5)")
            .bind(deployment_id.to_vec()).bind(recall_id.to_vec()).bind(asset.to_vec()).bind(i32::try_from(depth).map_err(|_| ApiError::Internal)?).bind(relation)
            .execute(&mut **tx).await.map_err(map_constraint_error)?;
    }
    decode_recall(&row)
}

pub async fn close_recall_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    recall_id: [u8; 32],
) -> Result<RecallRecord, ApiError> {
    let row = sqlx::query("UPDATE v2_recalls SET status='CLOSED',closed_at=now() WHERE deployment_id=$1 AND recall_id=$2 AND status='OPEN' RETURNING recall_id,deployment_id,opened_by_party_id,scope_type,scope_id,reason,status,snapshot_root")
        .bind(deployment_id.to_vec()).bind(recall_id.to_vec()).fetch_optional(&mut **tx).await.map_err(db_error)?
        .ok_or_else(|| ApiError::Conflict("recall is not open".into()))?;
    decode_recall(&row)
}

async fn seed_assets_tx(
    tx: &mut Transaction<'_, Postgres>,
    deployment_id: [u8; 32],
    scope_type: &str,
    scope_id: [u8; 32],
) -> Result<Vec<[u8; 32]>, ApiError> {
    let rows = match scope_type {
        "LOT" => sqlx::query("SELECT asset_id FROM v2_lot_assets WHERE deployment_id=$1 AND lot_id=$2").bind(deployment_id.to_vec()).bind(scope_id.to_vec()).fetch_all(&mut **tx).await,
        "TRANSFORMATION" => sqlx::query("SELECT parent_asset_id AS asset_id FROM v2_lineage_edges WHERE deployment_id=$1 AND transformation_id=$2 UNION SELECT child_asset_id AS asset_id FROM v2_lineage_edges WHERE deployment_id=$1 AND transformation_id=$2").bind(deployment_id.to_vec()).bind(scope_id.to_vec()).fetch_all(&mut **tx).await,
        "ANIMAL" | "PRODUCT" | "ASSET" => sqlx::query("SELECT asset_id FROM v2_assets WHERE deployment_id=$1 AND asset_id=$2").bind(deployment_id.to_vec()).bind(scope_id.to_vec()).fetch_all(&mut **tx).await,
        _ => return Err(ApiError::Validation("invalid recall scope".into())),
    }.map_err(db_error)?;
    rows.iter().map(|row| fixed(row, "asset_id")).collect()
}

fn validate_processing_kind(kind: &str) -> Result<(), ApiError> {
    if matches!(kind, "SLAUGHTER" | "BUTCHERY" | "PROCESSING") { Ok(()) } else { Err(ApiError::Validation("operationKind must be SLAUGHTER, BUTCHERY or PROCESSING".into())) }
}

fn normalize_direction(value: &str) -> Result<&'static str, ApiError> {
    match value { "INPUT" => Ok("INPUT"), "OUTPUT" => Ok("OUTPUT"), "BYPRODUCT" => Ok("BYPRODUCT"), "LOSS" => Ok("LOSS"), _ => Err(ApiError::Validation("processing direction is invalid".into())) }
}

fn direction_matches_kind(direction: &str, kind: &str, asset_type: i16) -> bool {
    match (direction, kind) {
        ("INPUT", "SLAUGHTER") => asset_type == 1,
        ("OUTPUT", "SLAUGHTER") => asset_type == 3,
        ("INPUT", "BUTCHERY") => asset_type == 3,
        ("OUTPUT", "BUTCHERY") => matches!(asset_type, 4..=6),
        ("INPUT", "PROCESSING") => matches!(asset_type, 4..=6),
        ("OUTPUT", "PROCESSING") => matches!(asset_type, 5..=6),
        ("BYPRODUCT", _) => asset_type == 7,
        ("LOSS", _) => asset_type == 8,
        _ => false,
    }
}

fn validate_scope(scope_type: &str) -> Result<(), ApiError> {
    if matches!(scope_type, "LOT" | "ANIMAL" | "TRANSFORMATION" | "PRODUCT" | "ASSET") { Ok(()) } else { Err(ApiError::Validation("scopeType is invalid".into())) }
}

async fn ensure_active_party_tx(tx: &mut Transaction<'_, Postgres>, deployment_id: [u8;32], party_id: [u8;32]) -> Result<(), ApiError> {
    let active = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM v2_parties WHERE deployment_id=$1 AND party_id=$2 AND status='ACTIVE')").bind(deployment_id.to_vec()).bind(party_id.to_vec()).fetch_one(&mut **tx).await.map_err(db_error)?;
    if active { Ok(()) } else { Err(ApiError::Conflict("party is not active in this deployment".into())) }
}

async fn ensure_active_facility_tx(tx: &mut Transaction<'_, Postgres>, deployment_id: [u8;32], facility_id: [u8;32]) -> Result<(), ApiError> {
    let active = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM v2_facilities WHERE deployment_id=$1 AND facility_id=$2 AND status='ACTIVE')").bind(deployment_id.to_vec()).bind(facility_id.to_vec()).fetch_one(&mut **tx).await.map_err(db_error)?;
    if active { Ok(()) } else { Err(ApiError::Conflict("facility is not active in this deployment".into())) }
}

fn decode_processing(row: &PgRow) -> Result<ProcessingRecord, ApiError> {
    Ok(ProcessingRecord { operation_id: fixed(row,"operation_id")?, deployment_id: fixed(row,"deployment_id")?, facility_id: fixed(row,"facility_id")?, lot_id: optional_fixed(row,"lot_id")?, transformation_id: optional_fixed(row,"transformation_id")?, operator_party_id: fixed(row,"operator_party_id")?, operation_kind: row.try_get("operation_kind").map_err(db_error)?, status: row.try_get("status").map_err(db_error)?, notes: row.try_get("notes").map_err(db_error)?, tx_signature: row.try_get("tx_signature").map_err(db_error)? })
}
fn decode_processing_item(row: &PgRow) -> Result<ProcessingItemRecord, ApiError> { Ok(ProcessingItemRecord { position: unsigned_i32(row,"position")?, asset_id: optional_fixed(row,"asset_id")?, direction: row.try_get("direction").map_err(db_error)?, quantity: unsigned_i64(row,"quantity")?, weight_grams: unsigned_i64(row,"weight_grams")? }) }
fn decode_shipment(row: &PgRow) -> Result<ShipmentRecord, ApiError> { Ok(ShipmentRecord { shipment_id: fixed(row,"shipment_id")?, deployment_id: fixed(row,"deployment_id")?, origin_facility_id: fixed(row,"origin_facility_id")?, destination_facility_id: fixed(row,"destination_facility_id")?, carrier_party_id: fixed(row,"carrier_party_id")?, created_by_party_id: fixed(row,"created_by_party_id")?, status: row.try_get("status").map_err(db_error)?, planned_departure: row.try_get("planned_departure").map_err(db_error)?, departed_at: row.try_get("departed_at").map_err(db_error)?, delivered_at: row.try_get("delivered_at").map_err(db_error)?, notes: row.try_get("notes").map_err(db_error)?, tx_signature: row.try_get("tx_signature").map_err(db_error)? }) }
fn decode_shipment_item(row: &PgRow) -> Result<ShipmentItemRecord, ApiError> { Ok(ShipmentItemRecord { position: unsigned_i32(row,"position")?, asset_id: fixed(row,"asset_id")?, quantity: unsigned_i64(row,"quantity")?, weight_grams: unsigned_i64(row,"weight_grams")? }) }
fn decode_recall(row: &PgRow) -> Result<RecallRecord, ApiError> { Ok(RecallRecord { recall_id: fixed(row,"recall_id")?, deployment_id: fixed(row,"deployment_id")?, opened_by_party_id: fixed(row,"opened_by_party_id")?, scope_type: row.try_get("scope_type").map_err(db_error)?, scope_id: fixed(row,"scope_id")?, reason: row.try_get("reason").map_err(db_error)?, status: row.try_get("status").map_err(db_error)?, snapshot_root: fixed(row,"snapshot_root")? }) }
fn decode_recall_member(row: &PgRow) -> Result<RecallMemberRecord, ApiError> { Ok(RecallMemberRecord { asset_id: fixed(row,"asset_id")?, traversal_depth: unsigned_i32(row,"traversal_depth")?, relation: row.try_get("relation").map_err(db_error)? }) }
fn fixed<const N: usize>(row: &PgRow, column: &str) -> Result<[u8;N], ApiError> { let value: Vec<u8> = row.try_get(column).map_err(db_error)?; value.try_into().map_err(|_| ApiError::Internal) }
fn optional_fixed<const N: usize>(row: &PgRow, column: &str) -> Result<Option<[u8;N]>, ApiError> { let value: Option<Vec<u8>> = row.try_get(column).map_err(db_error)?; value.map(|bytes| bytes.try_into().map_err(|_| ApiError::Internal)).transpose() }
fn unsigned_i32(row: &PgRow, column: &str) -> Result<u32, ApiError> { let value: i32 = row.try_get(column).map_err(db_error)?; u32::try_from(value).map_err(|_| ApiError::Internal) }
fn unsigned_i64(row: &PgRow, column: &str) -> Result<u64, ApiError> { let value: i64 = row.try_get(column).map_err(db_error)?; u64::try_from(value).map_err(|_| ApiError::Internal) }
fn map_constraint_error(error: sqlx::Error) -> ApiError { match error { sqlx::Error::Database(database) if database.code().as_deref()==Some("23505") => ApiError::Conflict("operational identity already exists".into()), sqlx::Error::Database(database) if database.code().as_deref()==Some("23503") => ApiError::Conflict("operational reference does not exist".into()), _ => db_error(error) } }
fn db_error(_: sqlx::Error) -> ApiError { ApiError::Unavailable("postgres query failed".into()) }
