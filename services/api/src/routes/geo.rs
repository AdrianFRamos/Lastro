//! Facility boundaries and Station installation (SPEC v2.1, S5). Operator token only; the
//! resulting transactions are signed by the deployment authority wallet.
//!
//! 1. `POST /api/v2/facilities/{id}/boundary` stores the CAR code and polygon (immutable) and
//!    returns its `geoCommitment`.
//! 2. `POST /api/v2/facilities/{id}/transaction-data` prepares `register_facility` carrying it.
//! 3. `POST /api/v2/stations/{id}/facility/transaction-data` prepares `set_station_facility`.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use lastro_protocol::v2::{FacilityBoundary, VertexE7};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;

use crate::{
    error::ApiError,
    model::{TransactionDataResponse, parse_hex32},
    repository::{facility_boundaries, operations},
    routes::operations::authorize_operator,
    solana::{
        v2_registry::{FacilityTerms, build_register_facility, build_set_station_facility},
        v2_transaction_builder::parse_program_id,
    },
    state::AppState,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundaryRequest {
    pub car_code: String,
    /// Rings of `[longitude, latitude]` WGS84 degrees; ring 0 is the outer boundary, each
    /// ring closed (first point equals last).
    pub rings: Vec<Vec<[f64; 2]>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoundaryResponse {
    pub facility_id: String,
    pub car_code: String,
    pub geo_commitment: String,
    pub vertex_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StationFacilityRequest {
    pub facility_id: String,
}

pub async fn set_boundary(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(facility_id): Path<String>,
    Json(body): Json<BoundaryRequest>,
) -> Result<(StatusCode, Json<BoundaryResponse>), ApiError> {
    authorize_operator(&state, &headers)?;
    let facility_id = parse_hex32("facilityId", &facility_id)?;
    operations::find_facility(&state.db, state.config.deployment_id, facility_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("facility not found".into()))?;
    let boundary = FacilityBoundary {
        car_code: body.car_code.clone(),
        rings: body
            .rings
            .iter()
            .map(|ring| ring.iter().map(to_e7).collect::<Result<Vec<_>, _>>())
            .collect::<Result<Vec<_>, _>>()?,
    };
    let encoded = boundary.encode().map_err(|_| {
        ApiError::Validation(
            "boundary must have a printable CAR code, closed rings of 4+ points, at most 64 rings and 10 000 points in WGS84 range"
                .into(),
        )
    })?;
    let geo_commitment = boundary.commitment().map_err(|_| ApiError::Internal)?;
    let inserted = facility_boundaries::insert(
        &state.db,
        &facility_boundaries::BoundaryRecord {
            facility_id,
            car_code: body.car_code.clone(),
            boundary: encoded,
            geo_commitment,
        },
    )
    .await?;
    Ok((
        if inserted {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(BoundaryResponse {
            facility_id: hex::encode(facility_id),
            car_code: body.car_code,
            geo_commitment: hex::encode(geo_commitment),
            vertex_count: boundary.rings.iter().map(Vec::len).sum(),
        }),
    ))
}

/// `register_facility` for the authority: the projected facility plus its boundary commitment
/// (all zero when no boundary was stored).
pub async fn facility_transaction(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(facility_id): Path<String>,
) -> Result<Json<TransactionDataResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let facility_id = parse_hex32("facilityId", &facility_id)?;
    let deployment = state.config.deployment_id;
    let facility = operations::find_facility(&state.db, deployment, facility_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("facility not found".into()))?;
    let owner = operations::find_party(&state.db, deployment, facility.owner_party_id)
        .await?
        .ok_or_else(|| ApiError::Conflict("facility owner party not found".into()))?;
    let geo_commitment = facility_boundaries::find(&state.db, facility_id)
        .await?
        .map_or([0; 32], |boundary| boundary.geo_commitment);
    let authority = authority(&state).await?;
    Ok(Json(build_register_facility(
        parse_program_id(&state.config.lastro_program_id)?,
        &deployment,
        authority,
        FacilityTerms {
            facility_id,
            owner: Pubkey::new_from_array(owner.wallet),
            facility_type: u8::try_from(facility.facility_type).map_err(|_| ApiError::Internal)?,
            credential_hash: facility.credential_hash,
            valid_from: facility.valid_from,
            valid_until: facility.valid_until,
            geo_commitment,
        },
    )?))
}

/// `set_station_facility` for the authority; the facility must already be on-chain.
pub async fn station_facility_transaction(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(station_id): Path<String>,
    Json(body): Json<StationFacilityRequest>,
) -> Result<Json<TransactionDataResponse>, ApiError> {
    authorize_operator(&state, &headers)?;
    let station_id = parse_hex32("stationId", &station_id)?;
    let facility_id = parse_hex32("facilityId", &body.facility_id)?;
    let deployment = state.config.deployment_id;
    state
        .rpc
        .v2_station(deployment, station_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("canonical StationRecord not found".into()))?;
    let facility = state
        .rpc
        .v2_facility(deployment, facility_id)
        .await?
        .ok_or_else(|| ApiError::Conflict("facility is not registered on-chain yet".into()))?;
    if facility.status != crate::solana::rpc::FACILITY_STATUS_ACTIVE {
        return Err(ApiError::Conflict("facility is not active on-chain".into()));
    }
    let authority = authority(&state).await?;
    Ok(Json(build_set_station_facility(
        parse_program_id(&state.config.lastro_program_id)?,
        &deployment,
        authority,
        station_id,
        facility_id,
    )?))
}

async fn authority(state: &AppState) -> Result<Pubkey, ApiError> {
    Ok(state
        .rpc
        .v2_protocol_config(state.config.deployment_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("canonical v2 ProtocolConfig not found".into()))?
        .authority)
}

/// Degrees -> degrees x 10^7, rounded half away from zero; rejects NaN and out-of-range.
fn to_e7(point: &[f64; 2]) -> Result<VertexE7, ApiError> {
    let scale = |value: f64, limit: f64| -> Result<i32, ApiError> {
        if !value.is_finite() || value.abs() > limit {
            return Err(ApiError::Validation(
                "boundary coordinates must be finite WGS84 degrees".into(),
            ));
        }
        Ok((value * 1e7).round() as i32)
    };
    Ok((scale(point[0], 180.0)?, scale(point[1], 90.0)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degrees_become_rounded_e7_integers() {
        assert_eq!(
            to_e7(&[-56.1, -15.5]).unwrap(),
            (-561_000_000, -155_000_000)
        );
        assert_eq!(to_e7(&[0.000_000_05, 0.0]).unwrap(), (1, 0));
        assert!(to_e7(&[181.0, 0.0]).is_err());
        assert!(to_e7(&[f64::NAN, 0.0]).is_err());
    }
}
