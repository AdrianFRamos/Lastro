//! Authenticated Agent transport endpoints. Bearer authentication does not grant custody authority.

use axum::{extract::{Path, State}, http::{HeaderMap, StatusCode}, Json};
use lastro_protocol::crypto::derive_station_id;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{
    crypto::verify_agent_evidence_locally,
    error::ApiError,
    model::{parse_hex32, AgentCommandResponse, AgentEvidenceRequest, AgentEvidenceStatus, AgentEvidenceStatusResponse, CaptureAction},
    repository::{captures::{self, CaptureState}, events},
    state::AppState,
};

pub async fn poll_command(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Option<AgentCommandResponse>>, ApiError> {
    authorize(&headers, &state.config.agent_token)?;
    let station_id = derive_station_id(&state.config.station_pubkey33)
        .map_err(|_| ApiError::Config("configured Station public key is invalid".into()))?;
    let command = captures::claim_next_for_station(&state.db, station_id).await?;
    Ok(Json(command.map(|capture| AgentCommandResponse {
        capture_id: capture.capture_id,
        action: match capture.action { 1 => CaptureAction::Origin, 2 => CaptureAction::Transfer, 3 => CaptureAction::Reidentify, _ => unreachable!("database CHECK") },
        deployment_id: hex::encode(state.config.deployment_id),
        animal_id: hex::encode(capture.animal_id),
        event_sequence: capture.event_sequence,
        identity_revision: capture.identity_revision,
        previous_event_hash: hex::encode(capture.previous_event_hash),
        expected_old_rfid_hash: hex::encode(capture.expected_old_rfid_hash),
        from_custodian: hex::encode(capture.from_custodian),
        to_custodian: hex::encode(capture.to_custodian),
    })))
}

pub async fn submit_evidence(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<AgentEvidenceRequest>,
) -> Result<StatusCode, ApiError> {
    authorize(&headers, &state.config.agent_token)?;
    let evidence = verify_agent_evidence_locally(&body)?;
    let capture = captures::load_for_evidence(&state.db, body.capture_id).await?
        .ok_or_else(|| ApiError::NotFound("capture not found".into()))?;
    if !matches!(capture.status, CaptureState::Dispatched | CaptureState::EvidenceAccepted) {
        return Err(ApiError::Conflict("capture is not active for evidence admission".into()));
    }

    let canonical_station_key = state.rpc.protocol_station_pubkey(state.config.deployment_id).await?;
    if canonical_station_key != state.config.station_pubkey33 {
        return Err(ApiError::Conflict("configured Station key disagrees with canonical ProtocolConfig".into()));
    }
    if evidence.station_pubkey33 != canonical_station_key {
        return Err(ApiError::Conflict(
            "submitted Station public key is not registered for this deployment".into(),
        ));
    }
    if evidence.event.deployment_id != state.config.deployment_id
        || evidence.event.animal_id != capture.animal_id
        || evidence.event.action as u8 != capture.action
        || evidence.event.event_sequence != capture.event_sequence
        || evidence.event.identity_revision != capture.identity_revision
        || evidence.event.previous_event_hash != capture.previous_event_hash
        || evidence.event.old_rfid_hash != capture.expected_old_rfid_hash
        || evidence.event.from_custodian != capture.from_custodian
        || evidence.event.to_custodian != capture.to_custodian
        || evidence.event.station_id != capture.station_id
    {
        return Err(ApiError::Conflict("StationEvent does not match immutable capture context".into()));
    }

    let mut tx = state.db.begin().await.map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;
    let inserted = events::insert_or_match_exact(&mut tx, body.capture_id, &evidence).await?;
    captures::mark_evidence_accepted(&mut tx, body.capture_id).await?;
    tx.commit().await.map_err(|_| ApiError::Unavailable("postgres commit failed".into()))?;
    Ok(if inserted { StatusCode::CREATED } else { StatusCode::OK })
}

pub async fn evidence_status(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AgentEvidenceStatusResponse>, ApiError> {
    authorize(&headers, &state.config.agent_token)?;
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    let record = events::find_by_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    let status = match record.status.as_str() {
        "EVIDENCE_ACCEPTED" => AgentEvidenceStatus::EvidenceAccepted,
        "SUBMITTED" => AgentEvidenceStatus::Submitted,
        "FINALIZED" => AgentEvidenceStatus::Finalized,
        _ => return Err(ApiError::Internal),
    };
    Ok(Json(AgentEvidenceStatusResponse { status }))
}

fn authorize(headers: &HeaderMap, expected_token: &str) -> Result<(), ApiError> {
    let Some(value) = headers.get(axum::http::header::AUTHORIZATION) else { return Err(ApiError::Unauthorized); };
    let Ok(value) = value.to_str() else { return Err(ApiError::Unauthorized); };
    let Some(token) = value.strip_prefix("Bearer ") else { return Err(ApiError::Unauthorized); };
    if constant_time_eq(token.as_bytes(), expected_token.as_bytes()) { Ok(()) } else { Err(ApiError::Unauthorized) }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let left_hash: [u8; 32] = Sha256::digest(left).into();
    let right_hash: [u8; 32] = Sha256::digest(right).into();
    bool::from(left_hash.ct_eq(&right_hash))
}
