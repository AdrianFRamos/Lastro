//! Authenticated operational registry endpoints for the v2 chain.
//!
//! The operator token is a bootstrap boundary only. It is not a substitute for
//! wallet/role authorization; every mutation writes an append-only audit record.

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde_json::json;
use serde::Deserialize;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::{
    error::ApiError,
    model::{
        AcceptCustodyTransferRequest, AuditEntryResponse, AuthorityGrantResponse,
        CreateCustodyTransferRequest, CreateFacilityRequest, CreateLotRequest, CreatePartyRequest,
        CustodyTransferResponse, FacilityResponse, GrantAuthorityRequest, LotAssetResponse,
        LotResponse, PartyResponse, RevokeAuthorityRequest, SetStatusRequest, parse_hex32,
    },
    repository::operations::{self, CustodyTransferRecord, FacilityRecord, LotRecord, PartyRecord},
    routes::agent::authorize,
    state::AppState,
};

pub async fn create_party(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreatePartyRequest>,
) -> Result<(StatusCode, Json<PartyResponse>), ApiError> {
    authorize_operator(&state, &headers)?;
    let party_id = parse_hex32("partyId", &body.party_id)?;
    let tax_id_hash = body
        .tax_id_hash
        .as_deref()
        .map(|value| parse_hex32("taxIdHash", value))
        .transpose()?;
    let wallet = parse_hex32("wallet", &body.wallet)?;
    validate_party(&body.legal_name, body.role)?;

    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = operations::insert_party_tx(
        &mut tx,
        state.config.deployment_id,
        party_id,
        &body.legal_name,
        tax_id_hash,
        wallet,
        body.role,
    )
    .await?;
    operations::append_audit_tx(
        &mut tx,
        state.config.deployment_id,
        Some(party_id),
        Some(wallet),
        "PARTY_REGISTERED",
        "PARTY",
        Some(party_id),
        json!({ "role": body.role, "legalName": body.legal_name }),
    )
    .await?;
    tx.commit().await.map_err(db_unavailable)?;
    Ok((StatusCode::CREATED, Json(party_response(record))))
}

pub async fn get_party(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(party_id): Path<String>,
) -> Result<Json<PartyResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let party_id = parse_hex32("partyId", &party_id)?;
    let record = operations::find_party(&state.db, state.config.deployment_id, party_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("party not found".into()))?;
    Ok(Json(party_response(record)))
}

pub async fn create_facility(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateFacilityRequest>,
) -> Result<(StatusCode, Json<FacilityResponse>), ApiError> {
    authorize_operator(&state, &headers)?;
    let facility_id = parse_hex32("facilityId", &body.facility_id)?;
    let owner_party_id = parse_hex32("ownerPartyId", &body.owner_party_id)?;
    let credential_hash = parse_hex32("credentialHash", &body.credential_hash)?;
    validate_facility(&body.display_name, body.facility_type, body.valid_from, body.valid_until)?;

    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = operations::insert_facility_tx(
        &mut tx,
        state.config.deployment_id,
        facility_id,
        owner_party_id,
        body.facility_type,
        &body.display_name,
        credential_hash,
        body.valid_from,
        body.valid_until,
    )
    .await?;
    operations::append_audit_tx(
        &mut tx,
        state.config.deployment_id,
        Some(owner_party_id),
        None,
        "FACILITY_REGISTERED",
        "FACILITY",
        Some(facility_id),
        json!({ "facilityType": body.facility_type, "displayName": body.display_name }),
    )
    .await?;
    tx.commit().await.map_err(db_unavailable)?;
    Ok((StatusCode::CREATED, Json(facility_response(record))))
}

pub async fn get_facility(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(facility_id): Path<String>,
) -> Result<Json<FacilityResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let facility_id = parse_hex32("facilityId", &facility_id)?;
    let record = operations::find_facility(&state.db, state.config.deployment_id, facility_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("facility not found".into()))?;
    Ok(Json(facility_response(record)))
}

pub async fn create_lot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateLotRequest>,
) -> Result<(StatusCode, Json<LotResponse>), ApiError> {
    authorize_operator(&state, &headers)?;
    let lot_id = parse_hex32("lotId", &body.lot_id)?;
    let facility_id = parse_hex32("facilityId", &body.facility_id)?;
    let owner_party_id = parse_hex32("ownerPartyId", &body.owner_party_id)?;
    validate_lot(&body)?;

    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = operations::insert_lot_tx(
        &mut tx,
        state.config.deployment_id,
        lot_id,
        facility_id,
        owner_party_id,
        body.external_reference.as_deref(),
        body.head_count,
        body.live_weight_grams,
        &body.assets,
    )
    .await?;
    operations::append_audit_tx(
        &mut tx,
        state.config.deployment_id,
        Some(owner_party_id),
        None,
        "LOT_REGISTERED",
        "LOT",
        Some(lot_id),
        json!({ "facilityId": body.facility_id, "headCount": body.head_count, "assetCount": body.assets.len() }),
    )
    .await?;
    tx.commit().await.map_err(db_unavailable)?;
    let assets = operations::list_lot_assets(&state.db, state.config.deployment_id, lot_id).await?;
    Ok((StatusCode::CREATED, Json(lot_response(record, assets))))
}

pub async fn get_lot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lot_id): Path<String>,
) -> Result<Json<LotResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let lot_id = parse_hex32("lotId", &lot_id)?;
    let record = operations::find_lot(&state.db, state.config.deployment_id, lot_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("lot not found".into()))?;
    let assets = operations::list_lot_assets(&state.db, state.config.deployment_id, lot_id).await?;
    Ok(Json(lot_response(record, assets)))
}

pub async fn create_custody_transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateCustodyTransferRequest>,
) -> Result<(StatusCode, Json<CustodyTransferResponse>), ApiError> {
    authorize_operator(&state, &headers)?;
    let asset_id = parse_hex32("assetId", &body.asset_id)?;
    let from_party_id = body
        .from_party_id
        .as_deref()
        .map(|value| parse_hex32("fromPartyId", value))
        .transpose()?;
    let to_party_id = parse_hex32("toPartyId", &body.to_party_id)?;
    let from_facility_id = body
        .from_facility_id
        .as_deref()
        .map(|value| parse_hex32("fromFacilityId", value))
        .transpose()?;
    let to_facility_id = parse_hex32("toFacilityId", &body.to_facility_id)?;
    let created_by_party_id = body
        .created_by_party_id
        .as_deref()
        .map(|value| parse_hex32("createdByPartyId", value))
        .transpose()?;
    if body.reason.trim().len() < 2 || body.reason.len() > 500 {
        return Err(ApiError::Validation("reason length is invalid".into()));
    }

    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = operations::insert_transfer_tx(
        &mut tx,
        state.config.deployment_id,
        body.transfer_id,
        asset_id,
        from_party_id,
        to_party_id,
        from_facility_id,
        to_facility_id,
        &body.reason,
        created_by_party_id,
    )
    .await?;
    operations::append_audit_tx(
        &mut tx,
        state.config.deployment_id,
        created_by_party_id,
        None,
        "CUSTODY_PROPOSED",
        "CUSTODY_TRANSFER",
        None,
        json!({ "transferId": body.transfer_id, "assetId": body.asset_id, "toPartyId": body.to_party_id, "toFacilityId": body.to_facility_id }),
    )
    .await?;
    tx.commit().await.map_err(db_unavailable)?;
    Ok((StatusCode::CREATED, Json(transfer_response(record))))
}

pub async fn get_custody_transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(transfer_id): Path<Uuid>,
) -> Result<Json<CustodyTransferResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let record = operations::find_transfer(&state.db, state.config.deployment_id, transfer_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("custody transfer not found".into()))?;
    Ok(Json(transfer_response(record)))
}

pub async fn accept_custody_transfer(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(transfer_id): Path<Uuid>,
    Json(body): Json<AcceptCustodyTransferRequest>,
) -> Result<Json<CustodyTransferResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let accepted_by_party_id = parse_hex32("acceptedByPartyId", &body.accepted_by_party_id)?;
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = operations::accept_transfer_tx(
        &mut tx,
        state.config.deployment_id,
        transfer_id,
        accepted_by_party_id,
    )
    .await?;
    operations::append_audit_tx(
        &mut tx,
        state.config.deployment_id,
        Some(accepted_by_party_id),
        None,
        "CUSTODY_ACCEPTED",
        "CUSTODY_TRANSFER",
        None,
        json!({ "transferId": transfer_id }),
    )
    .await?;
    tx.commit().await.map_err(db_unavailable)?;
    Ok(Json(transfer_response(record)))
}

pub async fn create_authority_grant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<GrantAuthorityRequest>,
) -> Result<(StatusCode, Json<AuthorityGrantResponse>), ApiError> {
    authorize_operator(&state, &headers)?;
    let party_id = parse_hex32("partyId", &body.party_id)?;
    let facility_id = body
        .facility_id
        .as_deref()
        .map(|value| parse_hex32("facilityId", value))
        .transpose()?;
    let granted_by = body
        .granted_by_party_id
        .as_deref()
        .ok_or_else(|| ApiError::Validation("grantedByPartyId is required".into()))
        .and_then(|value| parse_hex32("grantedByPartyId", value))?;
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = operations::insert_authority_grant_tx(
        &mut tx,
        state.config.deployment_id,
        body.grant_id,
        party_id,
        facility_id,
        &body.capability,
        Some(granted_by),
        body.valid_from,
        body.valid_until,
        body.reason.as_deref(),
    )
    .await?;
    operations::append_audit_tx(
        &mut tx,
        state.config.deployment_id,
        Some(granted_by),
        None,
        "AUTHORITY_GRANTED",
        "AUTHORITY_GRANT",
        None,
        json!({ "grantId": body.grant_id, "partyId": body.party_id, "capability": body.capability }),
    )
    .await?;
    tx.commit().await.map_err(db_unavailable)?;
    Ok((StatusCode::CREATED, Json(grant_response(record))))
}

pub async fn revoke_authority_grant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(grant_id): Path<Uuid>,
    Json(body): Json<RevokeAuthorityRequest>,
) -> Result<Json<AuthorityGrantResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let actor = body
        .revoked_by_party_id
        .as_deref()
        .ok_or_else(|| ApiError::Validation("revokedByPartyId is required".into()))
        .and_then(|value| parse_hex32("revokedByPartyId", value))?;
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = operations::revoke_authority_grant_tx(
        &mut tx,
        state.config.deployment_id,
        grant_id,
    )
    .await?;
    operations::append_audit_tx(
        &mut tx,
        state.config.deployment_id,
        Some(actor),
        None,
        "AUTHORITY_REVOKED",
        "AUTHORITY_GRANT",
        None,
        json!({ "grantId": grant_id }),
    )
    .await?;
    tx.commit().await.map_err(db_unavailable)?;
    Ok(Json(grant_response(record)))
}

pub async fn set_party_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(party_id): Path<String>,
    Json(body): Json<SetStatusRequest>,
) -> Result<Json<PartyResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let party_id = parse_hex32("partyId", &party_id)?;
    let actor = body
        .actor_party_id
        .as_deref()
        .ok_or_else(|| ApiError::Validation("actorPartyId is required".into()))
        .and_then(|value| parse_hex32("actorPartyId", value))?;
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = operations::set_party_status_tx(
        &mut tx,
        state.config.deployment_id,
        party_id,
        &body.status,
    )
    .await?;
    operations::append_audit_tx(
        &mut tx,
        state.config.deployment_id,
        Some(actor),
        None,
        "PARTY_STATUS_CHANGED",
        "PARTY",
        Some(party_id),
        json!({ "status": body.status }),
    )
    .await?;
    tx.commit().await.map_err(db_unavailable)?;
    Ok(Json(party_response(record)))
}

pub async fn set_facility_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(facility_id): Path<String>,
    Json(body): Json<SetStatusRequest>,
) -> Result<Json<FacilityResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let facility_id = parse_hex32("facilityId", &facility_id)?;
    let actor = body
        .actor_party_id
        .as_deref()
        .ok_or_else(|| ApiError::Validation("actorPartyId is required".into()))
        .and_then(|value| parse_hex32("actorPartyId", value))?;
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = operations::set_facility_status_tx(
        &mut tx,
        state.config.deployment_id,
        facility_id,
        &body.status,
    )
    .await?;
    operations::append_audit_tx(
        &mut tx,
        state.config.deployment_id,
        Some(actor),
        None,
        "FACILITY_STATUS_CHANGED",
        "FACILITY",
        Some(facility_id),
        json!({ "status": body.status }),
    )
    .await?;
    tx.commit().await.map_err(db_unavailable)?;
    Ok(Json(facility_response(record)))
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub async fn list_audit_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuditQuery>,
) -> Result<Json<Vec<AuditEntryResponse>>, ApiError> {
    authorize_operator(&state, &headers)?;
    let entries = operations::list_audit(
        &state.db,
        state.config.deployment_id,
        query.limit.unwrap_or(100),
        query.offset.unwrap_or(0),
    )
    .await?;
    Ok(Json(entries.into_iter().map(audit_response).collect()))
}

fn authorize_operator(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let token = state.config.operator_token.as_deref().ok_or_else(|| {
        ApiError::Unavailable("operational API token is not configured".into())
    })?;
    authorize(headers, token)
}

fn validate_party(name: &str, role: u16) -> Result<(), ApiError> {
    if name.trim().len() < 2 || name.len() > 200 || !(1..=11).contains(&role) {
        return Err(ApiError::Validation("party identity is invalid".into()));
    }
    Ok(())
}

fn validate_facility(
    name: &str,
    facility_type: u16,
    valid_from: i64,
    valid_until: i64,
) -> Result<(), ApiError> {
    if name.trim().len() < 2
        || name.len() > 200
        || !(1..=7).contains(&facility_type)
        || valid_from < 0
        || valid_until <= valid_from
    {
        return Err(ApiError::Validation("facility identity or validity is invalid".into()));
    }
    Ok(())
}

fn validate_lot(body: &CreateLotRequest) -> Result<(), ApiError> {
    if body.head_count == 0 || body.live_weight_grams == 0 || body.assets.is_empty() {
        return Err(ApiError::Validation("lot counts and assets must be positive".into()));
    }
    if body.assets.len() > 512 {
        return Err(ApiError::Validation("lot cannot contain more than 512 assets".into()));
    }
    Ok(())
}

fn party_response(record: PartyRecord) -> PartyResponse {
    PartyResponse {
        party_id: hex::encode(record.party_id),
        deployment_id: hex::encode(record.deployment_id),
        legal_name: record.legal_name,
        tax_id_hash: record.tax_id_hash.map(hex::encode),
        wallet: hex::encode(record.wallet),
        role: record.role,
        status: record.status,
    }
}

fn facility_response(record: FacilityRecord) -> FacilityResponse {
    FacilityResponse {
        facility_id: hex::encode(record.facility_id),
        deployment_id: hex::encode(record.deployment_id),
        owner_party_id: hex::encode(record.owner_party_id),
        facility_type: record.facility_type,
        display_name: record.display_name,
        credential_hash: hex::encode(record.credential_hash),
        valid_from: record.valid_from,
        valid_until: record.valid_until,
        status: record.status,
    }
}

fn lot_response(record: LotRecord, assets: Vec<operations::LotAssetRecord>) -> LotResponse {
    LotResponse {
        lot_id: hex::encode(record.lot_id),
        deployment_id: hex::encode(record.deployment_id),
        facility_id: hex::encode(record.facility_id),
        owner_party_id: hex::encode(record.owner_party_id),
        external_reference: record.external_reference,
        head_count: record.head_count,
        live_weight_grams: record.live_weight_grams,
        status: record.status,
        assets: assets
            .into_iter()
            .map(|asset| LotAssetResponse {
                asset_id: hex::encode(asset.asset_id),
                quantity: asset.quantity,
                weight_grams: asset.weight_grams,
                role: asset.role,
            })
            .collect(),
    }
}

fn transfer_response(record: CustodyTransferRecord) -> CustodyTransferResponse {
    CustodyTransferResponse {
        transfer_id: record.transfer_id,
        deployment_id: hex::encode(record.deployment_id),
        asset_id: hex::encode(record.asset_id),
        from_party_id: record.from_party_id.map(hex::encode),
        to_party_id: hex::encode(record.to_party_id),
        from_facility_id: record.from_facility_id.map(hex::encode),
        to_facility_id: hex::encode(record.to_facility_id),
        reason: record.reason,
        status: record.status,
        event_id: record.event_id.map(hex::encode),
        tx_signature: record.tx_signature,
        created_by_party_id: record.created_by_party_id.map(hex::encode),
        accepted_by_party_id: record.accepted_by_party_id.map(hex::encode),
    }
}

fn grant_response(record: operations::AuthorityGrantRecord) -> AuthorityGrantResponse {
    AuthorityGrantResponse {
        grant_id: record.grant_id,
        deployment_id: hex::encode(record.deployment_id),
        party_id: hex::encode(record.party_id),
        facility_id: record.facility_id.map(hex::encode),
        capability: record.capability,
        status: record.status,
        granted_by_party_id: record.granted_by_party_id.map(hex::encode),
        valid_from: record.valid_from,
        valid_until: record.valid_until,
        reason: record.reason,
    }
}

fn audit_response(record: operations::AuditRecord) -> AuditEntryResponse {
    AuditEntryResponse {
        audit_id: record.audit_id,
        deployment_id: hex::encode(record.deployment_id),
        actor_party_id: record.actor_party_id.map(hex::encode),
        action: record.action,
        resource_type: record.resource_type,
        resource_id: record.resource_id.map(hex::encode),
        payload: record.payload,
        created_at: record
            .created_at
            .format(&Rfc3339)
            .unwrap_or_else(|_| record.created_at.unix_timestamp().to_string()),
    }
}

fn db_unavailable(_: sqlx::Error) -> ApiError {
    ApiError::Unavailable("postgres transaction failed".into())
}
