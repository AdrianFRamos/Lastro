//! Facility boundaries (SPEC v2.1, S5): the canonical bytes a FacilityRecord's
//! `geo_commitment` hashes. Immutable once stored.

use sqlx::{PgPool, Row};

use crate::error::ApiError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundaryRecord {
    pub facility_id: [u8; 32],
    pub car_code: String,
    pub boundary: Vec<u8>,
    pub geo_commitment: [u8; 32],
}

/// Stores a boundary; an identical repeat is idempotent, any other one is a conflict.
pub async fn insert(pool: &PgPool, record: &BoundaryRecord) -> Result<bool, ApiError> {
    let inserted = sqlx::query(
        "INSERT INTO v2_facility_boundaries(facility_id,car_code,boundary,geo_commitment) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING",
    )
    .bind(record.facility_id.to_vec())
    .bind(&record.car_code)
    .bind(&record.boundary)
    .bind(record.geo_commitment.to_vec())
    .execute(pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.is_foreign_key_violation() => {
            ApiError::NotFound("facility not found".into())
        }
        _ => ApiError::Unavailable("postgres query failed".into()),
    })?
    .rows_affected()
        == 1;
    if inserted {
        return Ok(true);
    }
    match find(pool, record.facility_id).await? {
        Some(stored) if stored == *record => Ok(false),
        _ => Err(ApiError::Conflict(
            "this facility already has a different boundary (boundaries are immutable)".into(),
        )),
    }
}

pub async fn find(
    pool: &PgPool,
    facility_id: [u8; 32],
) -> Result<Option<BoundaryRecord>, ApiError> {
    let row = sqlx::query(
        "SELECT facility_id,car_code,boundary,geo_commitment FROM v2_facility_boundaries WHERE facility_id=$1",
    )
    .bind(facility_id.to_vec())
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::Unavailable("postgres query failed".into()))?;
    row.map(|row| {
        let fixed = |column: &str| -> Result<[u8; 32], ApiError> {
            let value: Vec<u8> = row
                .try_get(column)
                .map_err(|_| ApiError::Unavailable("postgres query failed".into()))?;
            value.try_into().map_err(|_| ApiError::Internal)
        };
        Ok(BoundaryRecord {
            facility_id: fixed("facility_id")?,
            car_code: row
                .try_get("car_code")
                .map_err(|_| ApiError::Unavailable("postgres query failed".into()))?,
            boundary: row
                .try_get("boundary")
                .map_err(|_| ApiError::Unavailable("postgres query failed".into()))?,
            geo_commitment: fixed("geo_commitment")?,
        })
    })
    .transpose()
}
