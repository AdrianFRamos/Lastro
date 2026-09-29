//! Controlled migration endpoints. Planning is non-destructive; promotion is
//! allowed only when v1 and canonical v2 projections match exactly.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    error::ApiError,
    model::{
        CreateMigrationRunRequest, MigrationCandidateResponse, MigrationRunResponse, parse_hex32,
    },
    repository::{migration, operations},
    routes::agent::authorize,
    state::AppState,
};

pub async fn plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateMigrationRunRequest>,
) -> Result<(StatusCode, Json<MigrationRunResponse>), ApiError> {
    authorize_operator(&state, &headers)?;
    let requested_by = body
        .requested_by_party_id
        .as_deref()
        .map(|value| parse_hex32("requestedByPartyId", value))
        .transpose()?;
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record = migration::plan_run_tx(
        &mut tx,
        state.config.deployment_id,
        body.run_id,
        requested_by,
    )
    .await?;
    operations::append_audit_tx(
        &mut tx,
        state.config.deployment_id,
        requested_by,
        None,
        "V1_MIGRATION_PLANNED",
        "MIGRATION_RUN",
        None,
        json!({ "runId": body.run_id, "source": "V1_ANIMALS", "sourceCount": record.source_count }),
    )
    .await?;
    tx.commit().await.map_err(db_unavailable)?;
    let candidates = migration::list_candidates(&state.db, body.run_id).await?;
    Ok((StatusCode::CREATED, Json(run_response(record, candidates))))
}

pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
) -> Result<Json<MigrationRunResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let record = migration::find_run(&state.db, state.config.deployment_id, run_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("migration run not found".into()))?;
    let candidates = migration::list_candidates(&state.db, run_id).await?;
    Ok(Json(run_response(record, candidates)))
}

pub async fn promote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((run_id, animal_id)): Path<(Uuid, String)>,
) -> Result<Json<MigrationCandidateResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let animal_id = parse_hex32("animalId", &animal_id)?;
    let mut tx = state.db.begin().await.map_err(db_unavailable)?;
    let record =
        migration::promote_candidate_tx(&mut tx, state.config.deployment_id, run_id, animal_id)
            .await?;
    operations::append_audit_tx(
        &mut tx,
        state.config.deployment_id,
        None,
        None,
        if record.status == "PROMOTED" {
            "V1_MIGRATION_PROMOTED"
        } else {
            "V1_MIGRATION_QUARANTINED"
        },
        "MIGRATION_CANDIDATE",
        Some(animal_id),
        json!({ "runId": run_id, "status": record.status, "reason": record.reason }),
    )
    .await?;
    tx.commit().await.map_err(db_unavailable)?;
    Ok(Json(candidate_response(record)))
}

fn authorize_operator(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let token =
        state.config.operator_token.as_deref().ok_or_else(|| {
            ApiError::Unavailable("operational API token is not configured".into())
        })?;
    authorize(headers, token)
}

fn run_response(
    record: migration::MigrationRunRecord,
    candidates: Vec<migration::MigrationCandidateRecord>,
) -> MigrationRunResponse {
    MigrationRunResponse {
        run_id: record.run_id,
        deployment_id: hex::encode(record.deployment_id),
        requested_by_party_id: record.requested_by_party_id.map(hex::encode),
        status: record.status,
        source_count: record.source_count,
        eligible_count: record.eligible_count,
        promoted_count: record.promoted_count,
        rejected_count: record.rejected_count,
        candidates: candidates.into_iter().map(candidate_response).collect(),
    }
}

fn candidate_response(record: migration::MigrationCandidateRecord) -> MigrationCandidateResponse {
    MigrationCandidateResponse {
        source_animal_id: hex::encode(record.source_animal_id),
        target_asset_id: record.target_asset_id.map(hex::encode),
        status: record.status,
        reason: record.reason,
        source_event_sequence: record.source_event_sequence,
        source_last_event_hash: record.source_last_event_hash.map(hex::encode),
    }
}

fn db_unavailable(_: sqlx::Error) -> ApiError {
    ApiError::Unavailable("postgres transaction failed".into())
}
