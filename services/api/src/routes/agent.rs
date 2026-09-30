//! Authenticated Agent transport endpoints (protocol v2). Bearer authentication does not
//! grant custody authority: evidence only becomes state through a wallet-signed transaction.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use lastro_protocol::crypto::derive_station_id;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{
    domain::v2::{decode_capture_evidence, verify_capture_evidence},
    error::ApiError,
    model::{
        AgentCommandResponse, AgentEvidenceRequest, AgentEvidenceStatus,
        AgentEvidenceStatusResponse, CaptureAction, parse_hex32,
    },
    repository::{
        captures_v2::{self, CaptureStatus},
        domain_v2,
    },
    state::AppState,
};

pub async fn poll_command(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Option<AgentCommandResponse>>, ApiError> {
    authorize(&headers, &state.config.agent_token)?;
    let station_id = derive_station_id(&state.config.station_pubkey33)
        .map_err(|_| ApiError::Config("configured Station public key is invalid".into()))?;
    let command = captures_v2::claim_next_for_station(&state.db, station_id).await?;
    Ok(Json(command.map(|capture| {
        AgentCommandResponse {
            capture_id: capture.capture_id,
            event_type: CaptureAction::from_event_type(capture.event_type)
                .map(CaptureAction::wire_name)
                .unwrap_or("UNKNOWN"),
            deployment_id: hex::encode(capture.deployment_id),
            asset_id: hex::encode(capture.asset_id),
            state_version: capture.state_version,
            previous_event_hash: hex::encode(capture.previous_event_hash),
            expected_rfid_hash: hex::encode(capture.expected_rfid_hash),
            observed_at: capture.observed_at,
            expires_at: capture.expires_at,
        }
    })))
}

/// Admit Station evidence for the capture it was dispatched for. 201 = new, 200 = exact retry.
pub async fn submit_evidence(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<AgentEvidenceRequest>,
) -> Result<StatusCode, ApiError> {
    authorize(&headers, &state.config.agent_token)?;
    let decoded = decode_capture_evidence(&body)?;
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;
    let capture = captures_v2::for_evidence_tx(&mut tx, body.capture_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("capture not found".into()))?;
    if capture.deployment_id != state.config.deployment_id {
        return Err(ApiError::NotFound("capture not found".into()));
    }
    let (evidence, observed_rfid) =
        verify_capture_evidence(&decoded, &capture, &state.config.station_pubkey33)?;

    let inserted = match capture.status {
        CaptureStatus::Dispatched => {
            let inserted = domain_v2::insert_capture_evidence_tx(
                &mut tx,
                &evidence,
                capture.capture_id,
                observed_rfid,
            )
            .await?;
            captures_v2::mark_evidence_accepted_tx(
                &mut tx,
                capture.capture_id,
                evidence.event_hash,
            )
            .await?;
            inserted
        }
        // Idempotent Agent retry after the API already admitted this exact evidence.
        CaptureStatus::EvidenceAccepted if capture.event_hash == Some(evidence.event_hash) => false,
        _ => {
            return Err(ApiError::Conflict(
                "capture is not active for evidence admission".into(),
            ));
        }
    };
    tx.commit()
        .await
        .map_err(|_| ApiError::Unavailable("postgres commit failed".into()))?;
    Ok(if inserted {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    })
}

pub async fn evidence_status(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AgentEvidenceStatusResponse>, ApiError> {
    authorize(&headers, &state.config.agent_token)?;
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    let status = match record.status.as_str() {
        "EVIDENCE_ACCEPTED" => AgentEvidenceStatus::EvidenceAccepted,
        "SUBMITTED" => AgentEvidenceStatus::Submitted,
        "FINALIZED" => AgentEvidenceStatus::Finalized,
        _ => return Err(ApiError::NotFound("event is no longer active".into())),
    };
    Ok(Json(AgentEvidenceStatusResponse { status }))
}

pub(crate) fn authorize(headers: &HeaderMap, expected_token: &str) -> Result<(), ApiError> {
    let Some(value) = headers.get(axum::http::header::AUTHORIZATION) else {
        return Err(ApiError::Unauthorized);
    };
    let Ok(value) = value.to_str() else {
        return Err(ApiError::Unauthorized);
    };
    let Some(token) = value.strip_prefix("Bearer ") else {
        return Err(ApiError::Unauthorized);
    };
    if constant_time_eq(token.as_bytes(), expected_token.as_bytes()) {
        Ok(())
    } else {
        Err(ApiError::Unauthorized)
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let left_hash: [u8; 32] = Sha256::digest(left).into();
    let right_hash: [u8; 32] = Sha256::digest(right).into();
    bool::from(left_hash.ct_eq(&right_hash))
}
