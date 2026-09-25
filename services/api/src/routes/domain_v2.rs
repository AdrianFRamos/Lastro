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
    model::{DomainEventAnchorResponse, DomainObservationRequest, parse_hex32},
    repository::domain_v2::{self, DomainAnchorRecord},
    routes::agent::authorize,
    state::AppState,
};

pub async fn ingest_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DomainObservationRequest>,
) -> Result<(StatusCode, Json<DomainEventAnchorResponse>), ApiError> {
    authorize(&headers, &state.config.agent_token)?;
    let canonical_key = state
        .rpc
        .protocol_station_pubkey(state.config.deployment_id)
        .await?;
    if canonical_key != state.config.station_pubkey33 {
        return Err(ApiError::Conflict(
            "configured Station key disagrees with canonical ProtocolConfig".into(),
        ));
    }
    let observation = verify_observation(&body, state.config.deployment_id, &canonical_key)?;
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
