//! Capture creation/status endpoints. Context is derived from canonical Solana state, never request-supplied history.

use axum::{extract::{Path, State}, http::StatusCode, Json};
use lastro_protocol::crypto::derive_station_id;
use uuid::Uuid;

use crate::{
    error::ApiError,
    model::{parse_hex32, CaptureAction, CaptureResponse, CaptureStatus, CreateCaptureRequest, EventStatus},
    repository::{animals, captures::{self, CaptureRecord, CaptureState, NewCapture}, events::{self, EventRecord}},
    state::AppState,
};

const ZERO32: [u8; 32] = [0; 32];

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateCaptureRequest>,
) -> Result<(StatusCode, Json<CaptureResponse>), ApiError> {
    let animal_id = parse_hex32("animalId", &body.animal_id)?;
    let local = animals::find_by_animal_id(&state.db, animal_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("animal not registered".into()))?;
    let next = body.next_custodian.as_deref().map(|value| parse_hex32("nextCustodian", value)).transpose()?;

    if let Some(event) = events::find_unfinalized_for_animal(&state.db, animal_id).await? {
        if !retry_intent_matches(&body.action, next, &event) {
            return Err(ApiError::Conflict(
                "animal already has a different accepted transition awaiting canonical finalization".into(),
            ));
        }
        let capture = captures::find_by_id(&state.db, event.capture_id).await?
            .ok_or_else(|| ApiError::Conflict("accepted event is missing its capture context".into()))?;
        return Ok((StatusCode::OK, Json(to_response(capture, Some(event))?)));
    }

    let canonical = state.rpc.animal_state(state.config.deployment_id, animal_id).await?;
    let station_id = derive_station_id(&state.config.station_pubkey33)
        .map_err(|_| ApiError::Config("configured Station public key is invalid".into()))?;
    let capture = match body.action {
        CaptureAction::Origin => {
            if canonical.is_some() || local.event_sequence != 0 || local.identity_revision != 0 {
                return Err(ApiError::Conflict("ORIGIN requires an unoriginated AnimalID".into()));
            }
            let to = next.ok_or_else(|| ApiError::Validation("ORIGIN requires nextCustodian".into()))?;
            if to == ZERO32 { return Err(ApiError::Validation("nextCustodian must be non-zero".into())); }
            NewCapture { station_id, action: 1, animal_id, event_sequence: 1, identity_revision: 1,
                expected_old_rfid_hash: ZERO32, from_custodian: ZERO32, to_custodian: to, previous_event_hash: ZERO32 }
        }
        CaptureAction::Transfer => {
            let current = canonical.ok_or_else(|| ApiError::Conflict("TRANSFER requires canonical AnimalState".into()))?;
            let to = next.ok_or_else(|| ApiError::Validation("TRANSFER requires nextCustodian".into()))?;
            if to == ZERO32 || to == current.current_custodian {
                return Err(ApiError::Validation("TRANSFER destination must be non-zero and different from current custodian".into()));
            }
            NewCapture { station_id, action: 2, animal_id,
                event_sequence: current.event_sequence.checked_add(1).ok_or(ApiError::Internal)?,
                identity_revision: current.identity_revision,
                expected_old_rfid_hash: current.current_rfid_hash,
                from_custodian: current.current_custodian, to_custodian: to,
                previous_event_hash: current.last_event_hash }
        }
        CaptureAction::Reidentify => {
            if next.is_some() { return Err(ApiError::Validation("REIDENTIFY must not provide nextCustodian".into())); }
            let current = canonical.ok_or_else(|| ApiError::Conflict("REIDENTIFY requires canonical AnimalState".into()))?;
            NewCapture { station_id, action: 3, animal_id,
                event_sequence: current.event_sequence.checked_add(1).ok_or(ApiError::Internal)?,
                identity_revision: current.identity_revision.checked_add(1).ok_or(ApiError::Internal)?,
                expected_old_rfid_hash: current.current_rfid_hash,
                from_custodian: current.current_custodian, to_custodian: current.current_custodian,
                previous_event_hash: current.last_event_hash }
        }
    };
    let record = captures::insert_pending(&state.db, &capture).await?;
    Ok((StatusCode::CREATED, Json(to_response(record, None)?)))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CaptureResponse>, ApiError> {
    let record = captures::find_by_id(&state.db, id).await?
        .ok_or_else(|| ApiError::NotFound("capture not found".into()))?;
    let event = events::find_by_capture_id(&state.db, id).await?;
    Ok(Json(to_response(record, event)?))
}

fn retry_intent_matches(action: &CaptureAction, next: Option<[u8; 32]>, event: &EventRecord) -> bool {
    let action_matches = matches!(
        (action, event.event.action),
        (CaptureAction::Origin, lastro_protocol::Action::Origin)
            | (CaptureAction::Transfer, lastro_protocol::Action::Transfer)
            | (CaptureAction::Reidentify, lastro_protocol::Action::Reidentify)
    );
    if !action_matches {
        return false;
    }
    match action {
        CaptureAction::Origin | CaptureAction::Transfer => next == Some(event.event.to_custodian),
        CaptureAction::Reidentify => next.is_none(),
    }
}

fn to_response(record: CaptureRecord, event: Option<EventRecord>) -> Result<CaptureResponse, ApiError> {
    let (event_hash, event_status, tx_signature) = match event {
        None => (None, None, None),
        Some(event) => {
            let status = match event.status.as_str() {
                "EVIDENCE_ACCEPTED" => EventStatus::EvidenceAccepted,
                "SUBMITTED" => EventStatus::Submitted,
                "FINALIZED" => EventStatus::Finalized,
                "REJECTED" => EventStatus::Rejected,
                _ => return Err(ApiError::Internal),
            };
            (Some(hex::encode(event.event_hash)), Some(status), event.tx_signature)
        }
    };
    Ok(CaptureResponse {
        capture_id: record.capture_id,
        action: match record.action { 1 => CaptureAction::Origin, 2 => CaptureAction::Transfer, 3 => CaptureAction::Reidentify, _ => unreachable!("database CHECK") },
        animal_id: hex::encode(record.animal_id),
        status: match record.status {
            CaptureState::Pending => CaptureStatus::Pending,
            CaptureState::Dispatched => CaptureStatus::Dispatched,
            CaptureState::EvidenceAccepted => CaptureStatus::EvidenceAccepted,
            CaptureState::Expired => CaptureStatus::Expired,
            CaptureState::Cancelled => CaptureStatus::Cancelled,
        },
        event_hash,
        event_status,
        tx_signature,
    })
}
