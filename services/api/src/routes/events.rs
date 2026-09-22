//! Event transaction preparation, submission tracking, and canonical confirmation.
//!
//! `transaction-data` returns inspectable instruction descriptors and never receives wallet
//! private keys. `submit` stores a transaction signature only after confirmed RPC proves the exact
//! transaction envelope. `confirm` advances PostgreSQL only after the same transaction is finalized
//! and the resulting canonical accounts match the immutable StationEvent.

use axum::{extract::{Path, State}, Json};
use lastro_protocol::{Action, StationEvent};

use crate::{
    error::ApiError,
    model::{parse_hex32, AnimalResponse, ConfirmEventRequest, EventStatus, EventSubmissionResponse, TransactionDataResponse},
    repository::{animals, events::{self, EventRecord}},
    routes::animals::to_response,
    solana::{
        rpc::{BindingStatus, CanonicalAnimalState, CanonicalRfidBinding},
        transaction_builder::build_transaction_data,
    },
    state::AppState,
};

pub async fn transaction_data(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
) -> Result<Json<TransactionDataResponse>, ApiError> {
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    let record = load_preparable_event(&state, event_hash).await?;
    verify_station_registration(&state, &record).await?;
    verify_canonical_predecessor(&state, &record.event).await?;
    Ok(Json(build_transaction_data(&state.config.lastro_program_id, &record)?))
}

pub async fn submit(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
    Json(body): Json<ConfirmEventRequest>,
) -> Result<Json<EventSubmissionResponse>, ApiError> {
    validate_transaction_signature(&body.tx_signature)?;
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    let record = events::find_by_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    verify_event_deployment(&state, &record)?;

    match record.status.as_str() {
        "FINALIZED" => {
            require_same_transaction(&record, &body.tx_signature)?;
            return Ok(Json(EventSubmissionResponse {
                status: EventStatus::Finalized,
                tx_signature: body.tx_signature,
            }));
        }
        "SUBMITTED" => {
            require_same_transaction(&record, &body.tx_signature)?;
            return Ok(Json(EventSubmissionResponse {
                status: EventStatus::Submitted,
                tx_signature: body.tx_signature,
            }));
        }
        "EVIDENCE_ACCEPTED" => {}
        _ => return Err(ApiError::Conflict("event is not eligible for transaction submission".into())),
    }
    if let Some(existing) = record.tx_signature.as_deref() {
        if existing != body.tx_signature {
            return Err(ApiError::Conflict("event already references a different transaction".into()));
        }
    }

    verify_station_registration(&state, &record).await?;
    let expected_transaction = build_transaction_data(&state.config.lastro_program_id, &record)?;
    if !state.rpc.transaction_matches_confirmed(&body.tx_signature, &expected_transaction).await? {
        return Err(ApiError::Conflict(
            "transaction is not a confirmed exact Lastro transaction for this event".into(),
        ));
    }
    events::mark_submitted(&state.db, event_hash, &body.tx_signature).await?;
    Ok(Json(EventSubmissionResponse {
        status: EventStatus::Submitted,
        tx_signature: body.tx_signature,
    }))
}

pub async fn confirm(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
    Json(body): Json<ConfirmEventRequest>,
) -> Result<Json<AnimalResponse>, ApiError> {
    validate_transaction_signature(&body.tx_signature)?;
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    let record = events::find_by_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    verify_event_deployment(&state, &record)?;
    if record.status == "FINALIZED" {
        require_same_transaction(&record, &body.tx_signature)?;
    } else if record.status == "SUBMITTED" {
        require_same_transaction(&record, &body.tx_signature)?;
    } else {
        return Err(ApiError::Conflict(
            "event transaction has not been verified at confirmed commitment".into(),
        ));
    }

    verify_station_registration(&state, &record).await?;
    let expected_transaction = build_transaction_data(&state.config.lastro_program_id, &record)?;
    if !state.rpc.transaction_matches(&body.tx_signature, &expected_transaction).await? {
        return Err(ApiError::Conflict(
            "transaction is not a finalized exact Lastro transaction for this event".into(),
        ));
    }
    verify_canonical_terminal(&state, &record.event).await?;

    if record.status == "FINALIZED" {
        let local = animals::find_by_animal_id(&state.db, record.event.animal_id)
            .await?
            .ok_or_else(|| ApiError::Conflict("local projection is missing finalized animal state".into()))?;
        if !animals::matches_terminal(&local, &record.event) {
            return Err(ApiError::Conflict("local projection disagrees with finalized canonical state".into()));
        }
        return Ok(Json(to_response(local)));
    }

    let mut tx = state.db.begin().await
        .map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;
    let projected = animals::apply_confirmed_state(&mut tx, &record.event).await?;
    events::mark_finalized(&mut tx, event_hash, &body.tx_signature).await?;
    tx.commit().await.map_err(|_| ApiError::Unavailable("postgres commit failed".into()))?;
    Ok(Json(to_response(projected)))
}

async fn load_preparable_event(state: &AppState, event_hash: [u8; 32]) -> Result<EventRecord, ApiError> {
    let record = events::find_by_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    if record.event.deployment_id != state.config.deployment_id {
        return Err(ApiError::Conflict("event belongs to a different deployment".into()));
    }
    if record.status != "EVIDENCE_ACCEPTED" {
        return Err(ApiError::Conflict("event is not awaiting wallet authorization".into()));
    }
    Ok(record)
}

fn verify_event_deployment(state: &AppState, record: &EventRecord) -> Result<(), ApiError> {
    if record.event.deployment_id != state.config.deployment_id {
        return Err(ApiError::Conflict("event belongs to a different deployment".into()));
    }
    Ok(())
}

fn require_same_transaction(record: &EventRecord, transaction_signature: &str) -> Result<(), ApiError> {
    if record.tx_signature.as_deref() != Some(transaction_signature) {
        return Err(ApiError::Conflict("event is already associated with a different transaction".into()));
    }
    Ok(())
}

async fn verify_station_registration(state: &AppState, record: &EventRecord) -> Result<(), ApiError> {
    let canonical_key = state.rpc.protocol_station_pubkey(state.config.deployment_id).await?;
    if canonical_key != state.config.station_pubkey33 || canonical_key != record.station_pubkey {
        return Err(ApiError::Conflict(
            "event Station key disagrees with canonical ProtocolConfig".into(),
        ));
    }
    Ok(())
}

async fn verify_canonical_predecessor(state: &AppState, event: &StationEvent) -> Result<(), ApiError> {
    let canonical = state.rpc.animal_state(event.deployment_id, event.animal_id).await?;
    match event.action {
        Action::Origin => {
            if canonical.is_some() {
                return Err(ApiError::Conflict("ORIGIN already exists canonically for this AnimalID".into()));
            }
            if state.rpc.rfid_binding(event.deployment_id, event.new_rfid_hash).await?.is_some() {
                return Err(ApiError::Conflict("RFID has already appeared in canonical history".into()));
            }
        }
        Action::Transfer => {
            let canonical = canonical.ok_or_else(|| ApiError::Conflict("TRANSFER requires canonical AnimalState".into()))?;
            verify_predecessor_fields(event, &canonical, event.identity_revision)?;
            require_active_binding(state, event, event.old_rfid_hash).await?;
        }
        Action::Reidentify => {
            let canonical = canonical.ok_or_else(|| ApiError::Conflict("REIDENTIFY requires canonical AnimalState".into()))?;
            let previous_revision = event.identity_revision.checked_sub(1).ok_or(ApiError::Internal)?;
            verify_predecessor_fields(event, &canonical, previous_revision)?;
            require_active_binding(state, event, event.old_rfid_hash).await?;
            if state.rpc.rfid_binding(event.deployment_id, event.new_rfid_hash).await?.is_some() {
                return Err(ApiError::Conflict("new RFID has already appeared in canonical history".into()));
            }
        }
    }
    Ok(())
}

fn verify_predecessor_fields(
    event: &StationEvent,
    canonical: &CanonicalAnimalState,
    expected_revision: u32,
) -> Result<(), ApiError> {
    let expected_sequence = event.event_sequence.checked_sub(1).ok_or(ApiError::Internal)?;
    if canonical.animal_id != event.animal_id
        || canonical.current_rfid_hash != event.old_rfid_hash
        || canonical.current_custodian != event.from_custodian
        || canonical.identity_revision != expected_revision
        || canonical.event_sequence != expected_sequence
        || canonical.last_event_hash != event.previous_event_hash
    {
        return Err(ApiError::Conflict("canonical predecessor no longer matches StationEvent".into()));
    }
    Ok(())
}

async fn require_active_binding(
    state: &AppState,
    event: &StationEvent,
    rfid_hash: [u8; 32],
) -> Result<(), ApiError> {
    let binding = state.rpc.rfid_binding(event.deployment_id, rfid_hash).await?
        .ok_or_else(|| ApiError::Conflict("canonical RFID binding is missing".into()))?;
    if binding.status != BindingStatus::Active
        || binding.animal_id != event.animal_id
        || binding.rfid_hash != rfid_hash
    {
        return Err(ApiError::Conflict("canonical RFID binding is not ACTIVE for this AnimalID".into()));
    }
    Ok(())
}

async fn verify_canonical_terminal(state: &AppState, event: &StationEvent) -> Result<(), ApiError> {
    let canonical = state.rpc.animal_state(event.deployment_id, event.animal_id).await?
        .ok_or_else(|| ApiError::Conflict("finalized transaction did not produce AnimalState".into()))?;
    if canonical.animal_id != event.animal_id
        || canonical.current_rfid_hash != event.new_rfid_hash
        || canonical.current_custodian != event.to_custodian
        || canonical.identity_revision != event.identity_revision
        || canonical.event_sequence != event.event_sequence
        || canonical.last_event_hash != event.event_hash()
    {
        return Err(ApiError::Conflict("canonical AnimalState does not match event terminal state".into()));
    }

    let current = state.rpc.rfid_binding(event.deployment_id, event.new_rfid_hash).await?
        .ok_or_else(|| ApiError::Conflict("finalized transaction did not produce current RFID binding".into()))?;
    verify_binding(&current, event.animal_id, event.new_rfid_hash, BindingStatus::Active)?;

    if event.action == Action::Reidentify {
        let old = state.rpc.rfid_binding(event.deployment_id, event.old_rfid_hash).await?
            .ok_or_else(|| ApiError::Conflict("finalized REIDENTIFY lost the previous RFID binding".into()))?;
        verify_binding(&old, event.animal_id, event.old_rfid_hash, BindingStatus::Retired)?;
    }
    Ok(())
}

fn verify_binding(
    binding: &CanonicalRfidBinding,
    animal_id: [u8; 32],
    rfid_hash: [u8; 32],
    status: BindingStatus,
) -> Result<(), ApiError> {
    if binding.animal_id != animal_id || binding.rfid_hash != rfid_hash || binding.status != status {
        return Err(ApiError::Conflict("canonical RFID binding does not match event terminal state".into()));
    }
    Ok(())
}

fn validate_transaction_signature(signature: &str) -> Result<(), ApiError> {
    // A base58 encoding of a 64-byte Solana signature is never longer than 88 chars.
    // Reject longer attacker-controlled strings before allocating in the decoder.
    if signature.len() > 88 || !signature.is_ascii() {
        return Err(ApiError::Validation(
            "txSignature must be a canonical 64-byte base58 Solana signature".into(),
        ));
    }
    let decoded = bs58::decode(signature).into_vec()
        .map_err(|_| ApiError::Validation("txSignature must be a base58 Solana signature".into()))?;
    if decoded.len() != 64 || bs58::encode(&decoded).into_string() != signature {
        return Err(ApiError::Validation("txSignature must be a canonical 64-byte base58 Solana signature".into()));
    }
    Ok(())
}
