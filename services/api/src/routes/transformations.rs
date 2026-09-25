//! Public queries for transformation commitments and asset lineage.

use axum::{
    Json,
    extract::{Path, State},
};

use crate::{
    error::ApiError,
    model::{LineageEdgeResponse, TransformationResponse, parse_hex32},
    repository::transformations::{self, LineageEdgeRecord, TransformationRecord},
    state::AppState,
};

pub async fn get(
    State(state): State<AppState>,
    Path(transformation_id): Path<String>,
) -> Result<Json<TransformationResponse>, ApiError> {
    let transformation_id = parse_hex32("transformationId", &transformation_id)?;
    let record = transformations::find_transformation(
        &state.db,
        state.config.deployment_id,
        transformation_id,
    )
    .await?
    .ok_or_else(|| ApiError::NotFound("v2 transformation not found".into()))?;
    Ok(Json(to_response(record)))
}

pub async fn lineage(
    State(state): State<AppState>,
    Path(asset_id): Path<String>,
) -> Result<Json<Vec<LineageEdgeResponse>>, ApiError> {
    let asset_id = parse_hex32("assetId", &asset_id)?;
    let edges = transformations::list_lineage(&state.db, state.config.deployment_id, asset_id)
        .await?
        .into_iter()
        .map(to_edge_response)
        .collect();
    Ok(Json(edges))
}

fn to_response(record: TransformationRecord) -> TransformationResponse {
    TransformationResponse {
        transformation_id: hex::encode(record.transformation_id),
        deployment_id: hex::encode(record.deployment_id),
        facility_id: hex::encode(record.facility_id),
        transformation_type: record.transformation_type,
        input_root: hex::encode(record.input_root),
        output_root: hex::encode(record.output_root),
        input_count: record.input_count,
        output_count: record.output_count,
        input_weight_grams: record.input_weight_grams,
        output_weight_grams: record.output_weight_grams,
        byproduct_weight_grams: record.byproduct_weight_grams,
        loss_weight_grams: record.loss_weight_grams,
        tolerance_basis_points: record.tolerance_basis_points,
        manifest_nonce: record.manifest_nonce,
        manifest_hash: hex::encode(record.manifest_hash),
        manifest_bytes_base64: transformations::manifest_base64(&record),
        status: record.status,
        sequence: record.sequence,
        expires_at: record.expires_at,
        tx_signature: record.tx_signature,
    }
}

fn to_edge_response(record: LineageEdgeRecord) -> LineageEdgeResponse {
    LineageEdgeResponse {
        transformation_id: hex::encode(record.transformation_id),
        parent_asset_id: hex::encode(record.parent_asset_id),
        child_asset_id: hex::encode(record.child_asset_id),
        role: record.role,
        position: record.position,
        quantity: record.quantity,
        weight_grams: record.weight_grams,
    }
}
