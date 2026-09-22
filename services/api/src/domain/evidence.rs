//! EvidencePackage assembly from immutable event rows.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use lastro_protocol::evidence::{EvidenceEvent, EvidencePackage};
use sqlx::PgPool;

use crate::{error::ApiError, repository::events};

pub async fn assemble_package(
    pool: &PgPool,
    deployment_id: [u8; 32],
    animal_id: [u8; 32],
) -> Result<EvidencePackage, ApiError> {
    let rows = events::list_for_animal(pool, animal_id).await?;
    if rows.is_empty() {
        return Err(ApiError::NotFound("animal has no evidence events".into()));
    }

    let mut expected_sequence = 1u64;
    let mut package_events = Vec::with_capacity(rows.len());
    for row in rows {
        if row.event.event_sequence != expected_sequence {
            return Err(ApiError::Conflict("event history is not contiguous".into()));
        }
        if row.event.animal_id != animal_id || row.event.deployment_id != deployment_id {
            return Err(ApiError::Conflict("event history contains a foreign identity/deployment".into()));
        }
        if row.status != "FINALIZED" {
            return Err(ApiError::Conflict("evidence package requires finalized event history".into()));
        }
        let tx_signature = row.tx_signature.ok_or_else(|| {
            ApiError::Conflict("finalized event is missing its transaction signature".into())
        })?;
        package_events.push(EvidenceEvent {
            event_bytes_base64: BASE64.encode(row.event_bytes),
            observed_rfid_hex: hex::encode(row.observed_rfid),
            station_pubkey_hex: hex::encode(row.station_pubkey),
            station_signature_hex: hex::encode(row.station_signature),
            tx_signature: Some(tx_signature),
        });
        expected_sequence = expected_sequence.checked_add(1).ok_or(ApiError::Internal)?;
    }

    let package = EvidencePackage {
        version: 1,
        deployment_id: hex::encode(deployment_id),
        animal_id: hex::encode(animal_id),
        events: package_events,
    };
    package
        .validate_off_chain_chain()
        .map_err(|error| ApiError::Conflict(format!("stored evidence chain is invalid: {error}")))?;
    Ok(package)
}
