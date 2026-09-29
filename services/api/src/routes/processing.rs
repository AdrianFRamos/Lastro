//! Authenticated operational workflow routes for slaughter, processing, shipments and recalls.

use axum::{Json, extract::{Path, State}, http::{HeaderMap, StatusCode}};
use serde_json::json;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    error::ApiError,
    model::{CloseRecallRequest, CreateProcessingOperationRequest, CreateShipmentRequest, OpenRecallRequest, ProcessingItemResponse, ProcessingOperationResponse, RecallMemberResponse, RecallResponse, SetShipmentStatusRequest, ShipmentItemResponse, ShipmentResponse, parse_hex32},
    repository::processing::{self, ProcessingRecord, RecallRecord, ShipmentRecord},
    repository::operations,
    routes::agent::authorize,
    state::AppState,
};

pub async fn create_processing(
    State(state): State<AppState>, headers: HeaderMap,
    Json(body): Json<CreateProcessingOperationRequest>,
) -> Result<(StatusCode, Json<ProcessingOperationResponse>), ApiError> {
    authorize_operator(&state, &headers)?;
    let operation_id = parse_hex32("operationId", &body.operation_id)?;
    let facility_id = parse_hex32("facilityId", &body.facility_id)?;
    let lot_id = body.lot_id.as_deref().map(|value| parse_hex32("lotId", value)).transpose()?;
    let transformation_id = body.transformation_id.as_deref().map(|value| parse_hex32("transformationId", value)).transpose()?;
    let operator_party_id = parse_hex32("operatorPartyId", &body.operator_party_id)?;
    if body.notes.as_ref().is_some_and(|value| value.len() > 2000) { return Err(ApiError::Validation("notes is too long".into())); }
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = processing::insert_processing_tx(&mut tx, state.config.deployment_id, operation_id, facility_id, lot_id, transformation_id, operator_party_id, &body.operation_kind, body.notes.as_deref(), &body.items).await?;
    operations::append_audit_tx(&mut tx, state.config.deployment_id, Some(operator_party_id), None, "PROCESSING_REGISTERED", "PROCESSING_OPERATION", Some(operation_id), json!({ "operationKind": body.operation_kind, "itemCount": body.items.len() })).await?;
    tx.commit().await.map_err(db_unavailable)?;
    let items = processing::list_processing_items(&state.db, state.config.deployment_id, operation_id).await?;
    Ok((StatusCode::CREATED, Json(processing_response(record, items))))
}

pub async fn get_processing(State(state): State<AppState>, headers: HeaderMap, Path(operation_id): Path<String>) -> Result<Json<ProcessingOperationResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let operation_id = parse_hex32("operationId", &operation_id)?;
    let record = processing::find_processing(&state.db, state.config.deployment_id, operation_id).await?.ok_or_else(|| ApiError::NotFound("processing operation not found".into()))?;
    let items = processing::list_processing_items(&state.db, state.config.deployment_id, operation_id).await?;
    Ok(Json(processing_response(record, items)))
}

pub async fn ready_processing(State(state): State<AppState>, headers: HeaderMap, Path(operation_id): Path<String>) -> Result<Json<ProcessingOperationResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let operation_id = parse_hex32("operationId", &operation_id)?;
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = processing::mark_processing_ready_tx(&mut tx, state.config.deployment_id, operation_id).await?;
    operations::append_audit_tx(&mut tx, state.config.deployment_id, Some(record.operator_party_id), None, "PROCESSING_READY_FOR_CHAIN", "PROCESSING_OPERATION", Some(operation_id), json!({})).await?;
    tx.commit().await.map_err(db_unavailable)?;
    let items = processing::list_processing_items(&state.db, state.config.deployment_id, operation_id).await?;
    Ok(Json(processing_response(record, items)))
}

pub async fn finalize_processing(State(state): State<AppState>, headers: HeaderMap, Path(operation_id): Path<String>) -> Result<Json<ProcessingOperationResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let operation_id = parse_hex32("operationId", &operation_id)?;
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = processing::finalize_processing_tx(&mut tx, state.config.deployment_id, operation_id).await?;
    operations::append_audit_tx(&mut tx, state.config.deployment_id, Some(record.operator_party_id), None, "PROCESSING_FINALIZED", "PROCESSING_OPERATION", Some(operation_id), json!({ "txSignature": record.tx_signature })).await?;
    tx.commit().await.map_err(db_unavailable)?;
    let items = processing::list_processing_items(&state.db, state.config.deployment_id, operation_id).await?;
    Ok(Json(processing_response(record, items)))
}

pub async fn create_shipment(State(state): State<AppState>, headers: HeaderMap, Json(body): Json<CreateShipmentRequest>) -> Result<(StatusCode, Json<ShipmentResponse>), ApiError> {
    authorize_operator(&state, &headers)?;
    let shipment_id = parse_hex32("shipmentId", &body.shipment_id)?;
    let origin_facility_id = parse_hex32("originFacilityId", &body.origin_facility_id)?;
    let destination_facility_id = parse_hex32("destinationFacilityId", &body.destination_facility_id)?;
    let carrier_party_id = parse_hex32("carrierPartyId", &body.carrier_party_id)?;
    let created_by_party_id = parse_hex32("createdByPartyId", &body.created_by_party_id)?;
    let planned_departure = body.planned_departure.as_deref().map(parse_timestamp).transpose()?;
    if body.notes.as_ref().is_some_and(|value| value.len() > 2000) { return Err(ApiError::Validation("notes is too long".into())); }
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = processing::insert_shipment_tx(&mut tx, state.config.deployment_id, shipment_id, origin_facility_id, destination_facility_id, carrier_party_id, created_by_party_id, planned_departure, body.notes.as_deref(), &body.items).await?;
    operations::append_audit_tx(&mut tx, state.config.deployment_id, Some(created_by_party_id), None, "SHIPMENT_REGISTERED", "SHIPMENT", Some(shipment_id), json!({ "itemCount": body.items.len() })).await?;
    tx.commit().await.map_err(db_unavailable)?;
    let items = processing::list_shipment_items(&state.db, state.config.deployment_id, shipment_id).await?;
    Ok((StatusCode::CREATED, Json(shipment_response(record, items))))
}

pub async fn get_shipment(State(state): State<AppState>, headers: HeaderMap, Path(shipment_id): Path<String>) -> Result<Json<ShipmentResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let shipment_id = parse_hex32("shipmentId", &shipment_id)?;
    let record = processing::find_shipment(&state.db, state.config.deployment_id, shipment_id).await?.ok_or_else(|| ApiError::NotFound("shipment not found".into()))?;
    let items = processing::list_shipment_items(&state.db, state.config.deployment_id, shipment_id).await?;
    Ok(Json(shipment_response(record, items)))
}

pub async fn set_shipment_status(State(state): State<AppState>, headers: HeaderMap, Path(shipment_id): Path<String>, Json(body): Json<SetShipmentStatusRequest>) -> Result<Json<ShipmentResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let shipment_id = parse_hex32("shipmentId", &shipment_id)?;
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = processing::set_shipment_status_tx(&mut tx, state.config.deployment_id, shipment_id, &body.status).await?;
    operations::append_audit_tx(&mut tx, state.config.deployment_id, None, None, "SHIPMENT_STATUS_CHANGED", "SHIPMENT", Some(shipment_id), json!({ "status": body.status })).await?;
    tx.commit().await.map_err(db_unavailable)?;
    let items = processing::list_shipment_items(&state.db, state.config.deployment_id, shipment_id).await?;
    Ok(Json(shipment_response(record, items)))
}

pub async fn open_recall(State(state): State<AppState>, headers: HeaderMap, Json(body): Json<OpenRecallRequest>) -> Result<(StatusCode, Json<RecallResponse>), ApiError> {
    authorize_operator(&state, &headers)?;
    let recall_id = parse_hex32("recallId", &body.recall_id)?;
    let opened_by_party_id = parse_hex32("openedByPartyId", &body.opened_by_party_id)?;
    let scope_id = parse_hex32("scopeId", &body.scope_id)?;
    if body.reason.trim().len() < 5 || body.reason.len() > 2000 { return Err(ApiError::Validation("recall reason length is invalid".into())); }
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = processing::open_recall_tx(&mut tx, state.config.deployment_id, recall_id, opened_by_party_id, &body.scope_type, scope_id, &body.reason).await?;
    operations::append_audit_tx(&mut tx, state.config.deployment_id, Some(opened_by_party_id), None, "RECALL_OPENED", "RECALL", Some(recall_id), json!({ "scopeType": body.scope_type, "scopeId": body.scope_id, "memberSnapshotRoot": hex::encode(record.snapshot_root) })).await?;
    tx.commit().await.map_err(db_unavailable)?;
    let members = processing::list_recall_members(&state.db, state.config.deployment_id, recall_id).await?;
    Ok((StatusCode::CREATED, Json(recall_response(record, members))))
}

pub async fn get_recall(State(state): State<AppState>, headers: HeaderMap, Path(recall_id): Path<String>) -> Result<Json<RecallResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let recall_id = parse_hex32("recallId", &recall_id)?;
    let record = processing::find_recall(&state.db, state.config.deployment_id, recall_id).await?.ok_or_else(|| ApiError::NotFound("recall not found".into()))?;
    let members = processing::list_recall_members(&state.db, state.config.deployment_id, recall_id).await?;
    Ok(Json(recall_response(record, members)))
}

pub async fn close_recall(State(state): State<AppState>, headers: HeaderMap, Path(recall_id): Path<String>, Json(body): Json<CloseRecallRequest>) -> Result<Json<RecallResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let recall_id = parse_hex32("recallId", &recall_id)?;
    let closed_by_party_id = parse_hex32("closedByPartyId", &body.closed_by_party_id)?;
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = processing::close_recall_tx(&mut tx, state.config.deployment_id, recall_id).await?;
    operations::append_audit_tx(&mut tx, state.config.deployment_id, Some(closed_by_party_id), None, "RECALL_CLOSED", "RECALL", Some(recall_id), json!({})).await?;
    tx.commit().await.map_err(db_unavailable)?;
    let members = processing::list_recall_members(&state.db, state.config.deployment_id, recall_id).await?;
    Ok(Json(recall_response(record, members)))
}

fn authorize_operator(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let token = state.config.operator_token.as_deref().ok_or_else(|| ApiError::Unavailable("operational API token is not configured".into()))?;
    authorize(headers, token)
}

fn parse_timestamp(value: &str) -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|_| ApiError::Validation("timestamp must be RFC3339".into()))
}

fn processing_response(record: ProcessingRecord, items: Vec<processing::ProcessingItemRecord>) -> ProcessingOperationResponse {
    ProcessingOperationResponse { operation_id: hex::encode(record.operation_id), deployment_id: hex::encode(record.deployment_id), facility_id: hex::encode(record.facility_id), lot_id: record.lot_id.map(hex::encode), transformation_id: record.transformation_id.map(hex::encode), operator_party_id: hex::encode(record.operator_party_id), operation_kind: record.operation_kind, status: record.status, notes: record.notes, tx_signature: record.tx_signature, items: items.into_iter().map(|item| ProcessingItemResponse { position: item.position, asset_id: item.asset_id.map(hex::encode), direction: item.direction, quantity: item.quantity, weight_grams: item.weight_grams }).collect() }
}

fn shipment_response(record: ShipmentRecord, items: Vec<processing::ShipmentItemRecord>) -> ShipmentResponse {
    ShipmentResponse { shipment_id: hex::encode(record.shipment_id), deployment_id: hex::encode(record.deployment_id), origin_facility_id: hex::encode(record.origin_facility_id), destination_facility_id: hex::encode(record.destination_facility_id), carrier_party_id: hex::encode(record.carrier_party_id), created_by_party_id: hex::encode(record.created_by_party_id), status: record.status, planned_departure: format_time(record.planned_departure), departed_at: format_time(record.departed_at), delivered_at: format_time(record.delivered_at), notes: record.notes, tx_signature: record.tx_signature, items: items.into_iter().map(|item| ShipmentItemResponse { position: item.position, asset_id: hex::encode(item.asset_id), quantity: item.quantity, weight_grams: item.weight_grams }).collect() }
}

fn recall_response(record: RecallRecord, members: Vec<processing::RecallMemberRecord>) -> RecallResponse {
    RecallResponse { recall_id: hex::encode(record.recall_id), deployment_id: hex::encode(record.deployment_id), opened_by_party_id: hex::encode(record.opened_by_party_id), scope_type: record.scope_type, scope_id: hex::encode(record.scope_id), reason: record.reason, status: record.status, snapshot_root: hex::encode(record.snapshot_root), members: members.into_iter().map(|member| RecallMemberResponse { asset_id: hex::encode(member.asset_id), traversal_depth: member.traversal_depth, relation: member.relation }).collect() }
}

fn format_time(value: Option<OffsetDateTime>) -> Option<String> { value.and_then(|value| value.format(&Rfc3339).ok()) }
fn db_unavailable(_: sqlx::Error) -> ApiError { ApiError::Unavailable("postgres transaction failed".into()) }
