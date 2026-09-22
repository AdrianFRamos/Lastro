//! Animal HTTP endpoints. Registration is off-chain only; canonical current state remains on Solana.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    domain::animals::generate_animal_id,
    error::ApiError,
    model::{parse_hex32, AnimalResponse, CreateAnimalRequest},
    repository::animals::{self, AnimalRecord},
    solana::rpc::BindingStatus,
    state::AppState,
};

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateAnimalRequest>,
) -> Result<(StatusCode, Json<AnimalResponse>), ApiError> {
    let visual = body.visual_recovery_id.trim();
    if visual.is_empty() || visual.len() > 64 || visual != body.visual_recovery_id {
        return Err(ApiError::Validation(
            "visualRecoveryId must contain 1..64 characters without surrounding whitespace".into(),
        ));
    }
    let animal_id = generate_animal_id();
    if animal_id == [0; 32] {
        return Err(ApiError::Internal);
    }
    let record = animals::insert_registration(&state.db, animal_id, visual).await?;
    Ok((StatusCode::CREATED, Json(to_response(record))))
}

pub async fn get(
    State(state): State<AppState>,
    Path(animal_id): Path<String>,
) -> Result<Json<AnimalResponse>, ApiError> {
    let animal_id = parse_hex32("animalId", &animal_id)?;
    let record = animals::find_by_animal_id(&state.db, animal_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("animal not found".into()))?;
    Ok(Json(to_response(record)))
}

pub async fn by_recovery(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AnimalResponse>, ApiError> {
    if id.is_empty() || id.len() > 64 || id.trim() != id {
        return Err(ApiError::Validation("invalid visual recovery identifier".into()));
    }
    let record = animals::find_by_visual_recovery_id(&state.db, &id)
        .await?
        .ok_or_else(|| ApiError::NotFound("animal not found".into()))?;
    Ok(Json(to_response(record)))
}

pub async fn by_rfid(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Json<AnimalResponse>, ApiError> {
    let rfid_hash = parse_hex32("rfidHash", &hash)?;
    let binding = state
        .rpc
        .rfid_binding(state.config.deployment_id, rfid_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("active RFID binding not found".into()))?;
    if binding.status != BindingStatus::Active {
        return Err(ApiError::NotFound("active RFID binding not found".into()));
    }
    let record = animals::find_by_current_rfid_hash(&state.db, rfid_hash)
        .await?
        .ok_or_else(|| ApiError::Conflict("local projection is missing canonical RFID binding".into()))?;
    if record.animal_id != binding.animal_id {
        return Err(ApiError::Conflict(
            "local RFID projection disagrees with canonical Solana binding".into(),
        ));
    }
    Ok(Json(to_response(record)))
}

pub(crate) fn to_response(record: AnimalRecord) -> AnimalResponse {
    AnimalResponse {
        animal_id: hex::encode(record.animal_id),
        visual_recovery_id: record.visual_recovery_id,
        current_rfid_hash: record.current_rfid_hash.map(hex::encode),
        current_custodian: record.current_custodian.map(hex::encode),
        identity_revision: record.identity_revision,
        event_sequence: record.event_sequence,
        last_event_hash: record.last_event_hash.map(hex::encode),
    }
}
