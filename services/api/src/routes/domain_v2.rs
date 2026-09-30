//! HTTP boundary for v2 domain-event evidence and public timelines.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::{
    domain::v2::verify_observation,
    error::ApiError,
    model::{
        ConfirmEventRequest, DomainEventAnchorResponse, DomainObservationRequest,
        TransactionDataResponse, parse_hex32,
    },
    repository::{
        captures_v2,
        domain_v2::{self, DomainAnchorRecord},
    },
    routes::{agent::authorize, validate_transaction_signature},
    solana::v2_transaction_builder::{self, IdentityArgs},
    state::AppState,
};
use lastro_protocol::rfid::hash_canonical_rfid;
use solana_pubkey::Pubkey;

const EVENT_TYPE_OBSERVATION: u16 = 2;
const EVENT_TYPE_IDENTIFIER_BOUND: u16 = 18;
const EVENT_TYPE_IDENTIFIER_REPLACED: u16 = 19;

/// The exact transaction a v2 event must be anchored with: captures are signed by the custodian
/// the capture named; capture-less observations are relayed by the deployment authority.
async fn expected_transaction(
    state: &AppState,
    record: &DomainAnchorRecord,
) -> Result<TransactionDataResponse, ApiError> {
    let config = state
        .rpc
        .v2_protocol_config(state.config.deployment_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("canonical v2 ProtocolConfig not found".into()))?;
    let (signer, identity) = match record.envelope.event_type {
        // A captured presence proof is signed by the custodian the capture named; a
        // capture-less Agent observation is relayed by the deployment authority.
        EVENT_TYPE_OBSERVATION => match record.capture_id {
            Some(capture_id) => (
                Pubkey::new_from_array(
                    captures_v2::find(&state.db, capture_id)
                        .await?
                        .ok_or(ApiError::Internal)?
                        .required_signer,
                ),
                None,
            ),
            None => (config.authority, None),
        },
        event_type @ (EVENT_TYPE_IDENTIFIER_BOUND | EVENT_TYPE_IDENTIFIER_REPLACED) => {
            let capture_id = record.capture_id.ok_or_else(|| {
                ApiError::Conflict("identity event is not linked to a capture".into())
            })?;
            let capture = captures_v2::find(&state.db, capture_id)
                .await?
                .ok_or(ApiError::Internal)?;
            let observed = record.observed_rfid.ok_or(ApiError::Internal)?;
            let old_rfid_hash = if event_type == EVENT_TYPE_IDENTIFIER_BOUND {
                [0; 32]
            } else {
                capture.expected_rfid_hash
            };
            (
                Pubkey::new_from_array(capture.required_signer),
                Some(IdentityArgs {
                    old_rfid_hash,
                    new_rfid_hash: hash_canonical_rfid(&observed),
                }),
            )
        }
        _ => {
            return Err(ApiError::Conflict(
                "v2 event type cannot be anchored by a Station transaction".into(),
            ));
        }
    };
    v2_transaction_builder::build_transaction_data(
        &state.config.lastro_program_id,
        record,
        signer,
        config.station_registry,
        identity,
    )
}

pub async fn ingest_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<DomainObservationRequest>,
) -> Result<(StatusCode, Json<DomainEventAnchorResponse>), ApiError> {
    authorize(&headers, &state.config.agent_token)?;
    // Finalized on-chain admission proves the station registry account through
    // record_observation; ingestion only admits the
    // configured, cryptographically verified evidence into the durable queue.
    let observation = verify_observation(
        &body,
        state.config.deployment_id,
        &state.config.station_pubkey33,
    )?;
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;
    let inserted = domain_v2::insert_or_match_exact(&mut tx, &observation).await?;
    tx.commit()
        .await
        .map_err(|_| ApiError::Unavailable("postgres commit failed".into()))?;

    let record = domain_v2::find_by_event_hash(&state.db, observation.event_hash)
        .await?
        .ok_or(ApiError::Internal)?;
    Ok((
        if inserted {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(to_response(record)),
    ))
}

pub async fn transaction_data(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
) -> Result<Json<TransactionDataResponse>, ApiError> {
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("v2 event not found".into()))?;
    ensure_deployment(&state, &record)?;
    if !matches!(record.status.as_str(), "EVIDENCE_ACCEPTED" | "SUBMITTED") {
        return Err(ApiError::Conflict(
            "v2 event is not eligible for transaction preparation".into(),
        ));
    }
    Ok(Json(v2_transaction_builder::with_priority_fee(
        expected_transaction(&state, &record).await?,
        state.config.priority_fee_micro_lamports,
    )?))
}

pub async fn submit(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
    Json(body): Json<ConfirmEventRequest>,
) -> Result<Json<DomainEventAnchorResponse>, ApiError> {
    validate_transaction_signature(&body.tx_signature)?;
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("v2 event not found".into()))?;
    ensure_deployment(&state, &record)?;
    if matches!(record.status.as_str(), "SUBMITTED" | "FINALIZED") {
        require_same_transaction(&record, &body.tx_signature)?;
        return Ok(Json(to_response(record)));
    }
    if record.status != "EVIDENCE_ACCEPTED" {
        return Err(ApiError::Conflict(
            "v2 event is not eligible for transaction submission".into(),
        ));
    }
    let expected = expected_transaction(&state, &record).await?;
    if !state
        .rpc
        .transaction_matches_confirmed(&body.tx_signature, &expected)
        .await?
    {
        return Err(ApiError::Conflict(
            "transaction is not the confirmed exact transaction for this v2 event".into(),
        ));
    }
    domain_v2::mark_submitted(&state.db, event_hash, &body.tx_signature).await?;
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or(ApiError::Internal)?;
    Ok(Json(to_response(record)))
}

pub async fn confirm(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
    Json(body): Json<ConfirmEventRequest>,
) -> Result<Json<DomainEventAnchorResponse>, ApiError> {
    validate_transaction_signature(&body.tx_signature)?;
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    reconcile_finalized_event(&state, event_hash, &body.tx_signature).await?;
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or(ApiError::Internal)?;
    Ok(Json(to_response(record)))
}

/// Finalize one v2 observation only after the exact transaction and the resulting
/// EventAnchor + AssetState are proven at finalized commitment.
pub(crate) async fn reconcile_finalized_event(
    state: &AppState,
    event_hash: [u8; 32],
    tx_signature: &str,
) -> Result<(), ApiError> {
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("v2 event not found".into()))?;
    ensure_deployment(state, &record)?;
    if !matches!(record.status.as_str(), "SUBMITTED" | "FINALIZED") {
        return Err(ApiError::Conflict(
            "v2 event transaction has not been verified at confirmed commitment".into(),
        ));
    }
    require_same_transaction(&record, tx_signature)?;
    let expected = expected_transaction(state, &record).await?;
    if !state
        .rpc
        .transaction_matches(tx_signature, &expected)
        .await?
    {
        return Err(ApiError::Conflict(
            "transaction is not the finalized exact transaction for this v2 event".into(),
        ));
    }

    let anchor = state
        .rpc
        .v2_event_anchor(state.config.deployment_id, record.envelope.event_id)
        .await?
        .ok_or_else(|| {
            ApiError::Conflict("finalized transaction did not produce EventAnchor".into())
        })?;
    verify_anchor(&record, &anchor)?;
    let asset = state
        .rpc
        .v2_asset_state(state.config.deployment_id, record.envelope.subject_id)
        .await?
        .ok_or_else(|| {
            ApiError::Conflict("finalized transaction did not produce AssetState".into())
        })?;
    let identity_matches = match (record.envelope.event_type, record.observed_rfid) {
        (EVENT_TYPE_IDENTIFIER_BOUND | EVENT_TYPE_IDENTIFIER_REPLACED, Some(observed)) => {
            asset.current_rfid_hash == hash_canonical_rfid(&observed)
        }
        (EVENT_TYPE_IDENTIFIER_BOUND | EVENT_TYPE_IDENTIFIER_REPLACED, None) => false,
        _ => true,
    };
    // The EventAnchor above already proves the event was anchored. While the asset is still
    // at that version its head must be this event; a later custody change or event may have
    // advanced it, which leaves the anchored event canonical.
    if asset.state_version < record.envelope.state_version
        || (asset.state_version == record.envelope.state_version
            && (asset.last_event_hash != record.event_hash || !identity_matches))
    {
        return Err(ApiError::Conflict(
            "canonical v2 AssetState does not match the finalized event".into(),
        ));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;
    domain_v2::upsert_asset_projection_tx(&mut tx, &asset).await?;
    domain_v2::mark_finalized_tx(&mut tx, event_hash, tx_signature).await?;
    tx.commit()
        .await
        .map_err(|_| ApiError::Unavailable("postgres commit failed".into()))?;
    Ok(())
}

/// Recover an event a wallet signed and broadcast but never reported (browser closed, lost
/// response). Returns `true` once it is FINALIZED; `false` when no EventAnchor exists on-chain.
/// The recovered signature must still be the exact expected transaction at finalized commitment.
pub(crate) async fn recover_unreported_event(
    state: &AppState,
    event_hash: [u8; 32],
) -> Result<bool, ApiError> {
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("v2 event not found".into()))?;
    ensure_deployment(state, &record)?;
    match record.status.as_str() {
        "FINALIZED" => return Ok(true),
        "EVIDENCE_ACCEPTED" => {}
        _ => return Ok(false),
    }
    let Some(anchor) = state
        .rpc
        .v2_event_anchor(state.config.deployment_id, record.envelope.event_id)
        .await?
    else {
        return Ok(false);
    };
    verify_anchor(&record, &anchor)?;
    let signature = state
        .rpc
        .v2_event_anchor_signature(state.config.deployment_id, record.envelope.event_id)
        .await?
        .ok_or_else(|| {
            ApiError::Unavailable("EventAnchor creation transaction is not indexed yet".into())
        })?;
    let expected = expected_transaction(state, &record).await?;
    if !state.rpc.transaction_matches(&signature, &expected).await? {
        return Err(ApiError::Conflict(
            "the transaction that created this EventAnchor is not the expected Lastro transaction"
                .into(),
        ));
    }
    domain_v2::mark_submitted(&state.db, event_hash, &signature).await?;
    reconcile_finalized_event(state, event_hash, &signature).await?;
    Ok(true)
}

/// Settle accepted evidence whose window has closed without a reported transaction: recover it
/// when it was anchored, reject it otherwise (the program refuses events after `expires_at`).
pub(crate) async fn settle_unreported_evidence(state: &AppState) -> Result<usize, ApiError> {
    // One minute of slack for finalization of a transaction that landed right at expiry.
    let before = time::OffsetDateTime::now_utc().unix_timestamp() - 60;
    let mut settled = 0;
    for event_hash in domain_v2::list_unreported_expired(&state.db, before, 16).await? {
        if !recover_unreported_event(state, event_hash).await? {
            domain_v2::reject_unreported(&state.db, event_hash).await?;
        }
        settled += 1;
    }
    Ok(settled)
}

fn ensure_deployment(state: &AppState, record: &DomainAnchorRecord) -> Result<(), ApiError> {
    if record.envelope.deployment_id != state.config.deployment_id {
        return Err(ApiError::Conflict(
            "v2 event belongs to a different deployment".into(),
        ));
    }
    Ok(())
}

fn require_same_transaction(
    record: &DomainAnchorRecord,
    tx_signature: &str,
) -> Result<(), ApiError> {
    if record.tx_signature.as_deref() != Some(tx_signature) {
        return Err(ApiError::Conflict(
            "v2 event is already associated with a different transaction".into(),
        ));
    }
    Ok(())
}

fn verify_anchor(
    record: &DomainAnchorRecord,
    anchor: &crate::solana::rpc::CanonicalV2EventAnchor,
) -> Result<(), ApiError> {
    if anchor.event_id != record.envelope.event_id
        || anchor.deployment_id != record.envelope.deployment_id
        || anchor.subject_id != record.envelope.subject_id
        || anchor.source_id != record.envelope.source_id
        || anchor.event_type != record.envelope.event_type
        || anchor.state_version != record.envelope.state_version
        || anchor.observed_at != record.envelope.observed_at
        || anchor.expires_at != record.envelope.expires_at
        || anchor.expected_previous_hash != record.envelope.expected_previous_hash
        || anchor.payload_hash != record.envelope.payload_hash
        || anchor.event_hash != record.event_hash
    {
        return Err(ApiError::Conflict(
            "canonical v2 EventAnchor does not match the accepted evidence".into(),
        ));
    }
    Ok(())
}

pub async fn get_event(
    State(state): State<AppState>,
    Path(event_hash): Path<String>,
) -> Result<Json<DomainEventAnchorResponse>, ApiError> {
    let event_hash = parse_hex32("eventHash", &event_hash)?;
    let record = domain_v2::find_by_event_hash(&state.db, event_hash)
        .await?
        .ok_or_else(|| ApiError::NotFound("v2 event not found".into()))?;
    if record.envelope.deployment_id != state.config.deployment_id {
        return Err(ApiError::NotFound("v2 event not found".into()));
    }
    Ok(Json(to_response(record)))
}

pub async fn timeline(
    State(state): State<AppState>,
    Path(asset_id): Path<String>,
) -> Result<Json<Vec<DomainEventAnchorResponse>>, ApiError> {
    let asset_id = parse_hex32("assetId", &asset_id)?;
    let records =
        domain_v2::list_by_subject(&state.db, state.config.deployment_id, asset_id).await?;
    let records = records
        .into_iter()
        .filter(|record| record.envelope.deployment_id == state.config.deployment_id)
        .map(to_response)
        .collect();
    Ok(Json(records))
}

pub(crate) fn to_response(record: DomainAnchorRecord) -> DomainEventAnchorResponse {
    DomainEventAnchorResponse {
        event_id: hex::encode(record.envelope.event_id),
        event_hash: hex::encode(record.event_hash),
        deployment_id: hex::encode(record.envelope.deployment_id),
        subject_id: hex::encode(record.envelope.subject_id),
        source_id: hex::encode(record.envelope.source_id),
        event_type: record.envelope.event_type,
        state_version: record.envelope.state_version,
        observed_at: record.envelope.observed_at,
        expires_at: record.envelope.expires_at,
        expected_previous_hash: hex::encode(record.envelope.expected_previous_hash),
        payload_hash: hex::encode(record.envelope.payload_hash),
        envelope_bytes_base64: BASE64.encode(record.envelope_bytes),
        station_pubkey_hex: hex::encode(record.station_pubkey33),
        station_signature_hex: hex::encode(record.station_signature64),
        observed_rfid_hex: record.observed_rfid.map(hex::encode),
        status: match record.status.as_str() {
            "EVIDENCE_ACCEPTED" => crate::model::DomainEventStatus::EvidenceAccepted,
            "SUBMITTED" => crate::model::DomainEventStatus::Submitted,
            "FINALIZED" => crate::model::DomainEventStatus::Finalized,
            "REJECTED" => crate::model::DomainEventStatus::Rejected,
            _ => unreachable!("database CHECK"),
        },
        tx_signature: record.tx_signature,
    }
}
