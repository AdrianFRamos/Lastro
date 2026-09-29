//! HTTP boundary for v2 domain-event evidence and public timelines.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::{
    domain::v2::verify_observation,
    error::ApiError,
    model::{
        ConfirmEventRequest, DomainEventAnchorResponse, DomainObservationRequest,
        TransactionDataResponse, parse_hex32,
    },
    repository::domain_v2::{self, DomainAnchorRecord},
    routes::agent::authorize,
    routes::events::validate_transaction_signature,
    solana::v2_transaction_builder,
    state::AppState,
};

pub async fn ingest_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DomainObservationRequest>,
) -> Result<(StatusCode, Json<DomainEventAnchorResponse>), ApiError> {
    authorize(&headers, &state.config.agent_token)?;
    // v2 has a separate ProtocolConfigV2 layout and PDA seed from the v1
    // ProtocolConfig. Finalized on-chain admission still proves the station
    // registry account through record_observation; ingestion only admits the
    // configured, cryptographically verified evidence into the durable queue.
    let observation = verify_observation(
        &body,
        state.config.deployment_id,
        &state.config.station_pubkey33,
    )?;
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;
    let inserted = domain_v2::insert_or_match_exact(&mut tx, &observation).await?;
    tx.commit()
        .await
        .map_err(|_| ApiError::Unavailable("postgres commit failed".into()))?;

    let record = domain_v2::find_by_event_hash(&state.db, observation.event_hash)
        .await?
        .ok_or(ApiError::Internal)?;
    Ok((
        if inserted {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(to_response(record)),
    ))
}

pub async fn transaction_data(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
) -> Result<Json<TransactionDataResponse>, ApiError> {
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("v2 event not found".into()))?;
    ensure_deployment(&state, &record)?;
    if !matches!(record.status.as_str(), "EVIDENCE_ACCEPTED" | "SUBMITTED") {
        return Err(ApiError::Conflict(
            "v2 event is not eligible for transaction preparation".into(),
        ));
    }
    let config = state
        .rpc
        .v2_protocol_config(state.config.deployment_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("canonical v2 ProtocolConfig not found".into()))?;
    Ok(Json(v2_transaction_builder::build_transaction_data(
        &state.config.lastro_program_id,
        &record,
        config.authority,
        config.station_registry,
    )?))
}

pub async fn submit(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
    Json(body): Json<ConfirmEventRequest>,
) -> Result<Json<DomainEventAnchorResponse>, ApiError> {
    validate_transaction_signature(&body.tx_signature)?;
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("v2 event not found".into()))?;
    ensure_deployment(&state, &record)?;
    if matches!(record.status.as_str(), "SUBMITTED" | "FINALIZED") {
        require_same_transaction(&record, &body.tx_signature)?;
        return Ok(Json(to_response(record)));
    }
    if record.status != "EVIDENCE_ACCEPTED" {
        return Err(ApiError::Conflict(
            "v2 event is not eligible for transaction submission".into(),
        ));
    }
    let config = state
        .rpc
        .v2_protocol_config(state.config.deployment_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("canonical v2 ProtocolConfig not found".into()))?;
    let expected = v2_transaction_builder::build_transaction_data(
        &state.config.lastro_program_id,
        &record,
        config.authority,
        config.station_registry,
    )?;
    if !state
        .rpc
        .transaction_matches_confirmed(&body.tx_signature, &expected)
        .await?
    {
        return Err(ApiError::Conflict(
            "transaction is not a confirmed exact v2 observation transaction".into(),
        ));
    }
    domain_v2::mark_submitted(&state.db, event_hash, &body.tx_signature).await?;
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or(ApiError::Internal)?;
    Ok(Json(to_response(record)))
}

pub async fn confirm(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
    Json(body): Json<ConfirmEventRequest>,
) -> Result<Json<DomainEventAnchorResponse>, ApiError> {
    validate_transaction_signature(&body.tx_signature)?;
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    reconcile_finalized_event(&state, event_hash, &body.tx_signature).await?;
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or(ApiError::Internal)?;
    Ok(Json(to_response(record)))
}

/// Finalize one v2 observation only after the exact transaction and the resulting
/// EventAnchor + AssetState are proven at finalized commitment.
pub(crate) async fn reconcile_finalized_event(
    state: &AppState,
    event_hash: [u8; 32],
    tx_signature: &str,
) -> Result<(), ApiError> {
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("v2 event not found".into()))?;
    ensure_deployment(state, &record)?;
    if !matches!(record.status.as_str(), "SUBMITTED" | "FINALIZED") {
        return Err(ApiError::Conflict(
            "v2 event transaction has not been verified at confirmed commitment".into(),
        ));
    }
    require_same_transaction(&record, tx_signature)?;
    let config = state
        .rpc
        .v2_protocol_config(state.config.deployment_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("canonical v2 ProtocolConfig not found".into()))?;
    let expected = v2_transaction_builder::build_transaction_data(
        &state.config.lastro_program_id,
        &record,
        config.authority,
        config.station_registry,
    )?;
    if !state
        .rpc
        .transaction_matches(tx_signature, &expected)
        .await?
    {
        return Err(ApiError::Conflict(
            "transaction is not a finalized exact v2 observation transaction".into(),
        ));
    }

    let anchor = state
        .rpc
        .v2_event_anchor(state.config.deployment_id, record.envelope.event_id)
        .await?
        .ok_or_else(|| {
            ApiError::Conflict("finalized transaction did not produce EventAnchor".into())
        })?;
    verify_anchor(&record, &anchor)?;
    let asset = state
        .rpc
        .v2_asset_state(state.config.deployment_id, record.envelope.subject_id)
        .await?
        .ok_or_else(|| {
            ApiError::Conflict("finalized transaction did not produce AssetState".into())
        })?;
    if asset.state_version != record.envelope.state_version
        || asset.last_event_hash != record.event_hash
    {
        return Err(ApiError::Conflict(
            "canonical v2 AssetState does not match the finalized event".into(),
        ));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;
    domain_v2::upsert_asset_projection_tx(&mut tx, &asset).await?;
    domain_v2::mark_finalized_tx(&mut tx, event_hash, tx_signature).await?;
    tx.commit()
        .await
        .map_err(|_| ApiError::Unavailable("postgres commit failed".into()))?;
    Ok(())
}

fn ensure_deployment(state: &AppState, record: &DomainAnchorRecord) -> Result<(), ApiError> {
    if record.envelope.deployment_id != state.config.deployment_id {
        return Err(ApiError::Conflict(
            "v2 event belongs to a different deployment".into(),
        ));
    }
    Ok(())
}

fn require_same_transaction(
    record: &DomainAnchorRecord,
    tx_signature: &str,
) -> Result<(), ApiError> {
    if record.tx_signature.as_deref() != Some(tx_signature) {
        return Err(ApiError::Conflict(
            "v2 event is already associated with a different transaction".into(),
        ));
    }
    Ok(())
}

fn verify_anchor(
    record: &DomainAnchorRecord,
    anchor: &crate::solana::rpc::CanonicalV2EventAnchor,
) -> Result<(), ApiError> {
    if anchor.event_id != record.envelope.event_id
        || anchor.deployment_id != record.envelope.deployment_id
        || anchor.subject_id != record.envelope.subject_id
        || anchor.source_id != record.envelope.source_id
        || anchor.event_type != record.envelope.event_type
        || anchor.state_version != record.envelope.state_version
        || anchor.observed_at != record.envelope.observed_at
        || anchor.expires_at != record.envelope.expires_at
        || anchor.expected_previous_hash != record.envelope.expected_previous_hash
        || anchor.payload_hash != record.envelope.payload_hash
        || anchor.event_hash != record.event_hash
    {
        return Err(ApiError::Conflict(
            "canonical v2 EventAnchor does not match the accepted evidence".into(),
        ));
    }
    Ok(())
}

pub async fn get_event(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
) -> Result<Json<DomainEventAnchorResponse>, ApiError> {
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("v2 event not found".into()))?;
    if record.envelope.deployment_id != state.config.deployment_id {
        return Err(ApiError::NotFound("v2 event not found".into()));
    }
    Ok(Json(to_response(record)))
}

pub async fn timeline(
    State(state): State<AppState>,
    Path(asset_id): Path<String>,
) -> Result<Json<Vec<DomainEventAnchorResponse>>, ApiError> {
    let asset_id = parse_hex32("assetId", &asset_id)?;
    let records =
        domain_v2::list_by_subject(&state.db, state.config.deployment_id, asset_id).await?;
    let records = records
        .into_iter()
        .filter(|record| record.envelope.deployment_id == state.config.deployment_id)
        .map(to_response)
        .collect();
    Ok(Json(records))
}

fn to_response(record: DomainAnchorRecord) -> DomainEventAnchorResponse {
    DomainEventAnchorResponse {
        event_id: hex::encode(record.envelope.event_id),
        event_hash: hex::encode(record.event_hash),
        deployment_id: hex::encode(record.envelope.deployment_id),
        subject_id: hex::encode(record.envelope.subject_id),
        source_id: hex::encode(record.envelope.source_id),
        event_type: record.envelope.event_type,
        state_version: record.envelope.state_version,
        observed_at: record.envelope.observed_at,
        expires_at: record.envelope.expires_at,
        expected_previous_hash: hex::encode(record.envelope.expected_previous_hash),
        payload_hash: hex::encode(record.envelope.payload_hash),
        envelope_bytes_base64: BASE64.encode(record.envelope_bytes),
        station_pubkey_hex: hex::encode(record.station_pubkey33),
        station_signature_hex: hex::encode(record.station_signature64),
        status: match record.status.as_str() {
            "EVIDENCE_ACCEPTED" => crate::model::DomainEventStatus::EvidenceAccepted,
            "SUBMITTED" => crate::model::DomainEventStatus::Submitted,
            "FINALIZED" => crate::model::DomainEventStatus::Finalized,
            "REJECTED" => crate::model::DomainEventStatus::Rejected,
            _ => unreachable!("database CHECK"),
        },
        tx_signature: record.tx_signature,
    }
}
