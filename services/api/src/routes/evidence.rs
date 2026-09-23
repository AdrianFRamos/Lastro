//! Public EvidencePackage export endpoint.

use axum::{
    Json,
    extract::{Path, State},
};
use lastro_protocol::evidence::EvidencePackage;

use crate::{
    domain::evidence::assemble_package, error::ApiError, model::parse_hex32, state::AppState,
};

pub async fn package(
    State(state): State<AppState>,
    Path(animal_id): Path<String>,
) -> Result<Json<EvidencePackage>, ApiError> {
    let animal_id = parse_hex32("animalId", &animal_id)?;
    Ok(Json(
        assemble_package(&state.db, state.config.deployment_id, animal_id).await?,
    ))
}
