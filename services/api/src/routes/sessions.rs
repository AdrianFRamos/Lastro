//! Handling sessions (SPEC v2.1, S3/S4a/S8).
//!
//! 1. `POST /api/v2/sessions` prepares `open_session` for the custodian (one signature).
//! 2. `POST /api/v2/sessions/{id}/confirm` admits the finalized grant and freezes the manifest.
//! 3. The Agent reads `GET .../manifest` once, works offline, and delivers each Station-signed
//!    presence proof to `POST .../observations` whenever it has signal.
//! 4. `POST .../batches` prepares `anchor_observation_batch` for up to 1 024 pending proofs;
//!    `POST .../batches/{index}/confirm` admits the finalized `BatchAnchor`.
//! 5. `POST .../revoke` prepares `revoke_session`; `POST .../sync` mirrors the chain.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use lastro_protocol::{
    crypto::{derive_station_id, verify_station_signature_bytes},
    rfid::hash_canonical_rfid,
    v2::{
        DomainEventEnvelope, EventType, MAX_BATCH_EVENT_AGE_SECONDS, MAX_BATCH_LEAVES,
        MAX_SESSION_EVENTS, MAX_SESSION_SECONDS, SESSION_ACTION_OBSERVE, SessionAssetContext,
        V2_ENVELOPE_LEN, batch_leaf_hash, batch_merkle_root,
    },
};
use solana_pubkey::Pubkey;
use time::OffsetDateTime;

use crate::{
    domain::v2::{decode_base64_array, decode_hex_array},
    error::ApiError,
    model::{
        ConfirmEventRequest, CreateSessionRequest, PreparedBatchResponse, PreparedSessionResponse,
        SessionBatchResponse, SessionManifestAsset, SessionManifestResponse,
        SessionObservationRequest, SessionObservationResponse, SessionResponse,
        TransactionDataResponse, parse_hex32,
    },
    repository::sessions::{self, BatchRecord, SessionObservation, SessionRecord},
    routes::{access::authorize_custodian, agent::authorize, validate_transaction_signature},
    solana::{
        rpc::{CanonicalV2Session, SESSION_STATUS_REVOKED},
        v2_session::{
            BatchTerms, SessionTerms, build_anchor_batch, build_open_session, build_revoke_session,
        },
        v2_transaction_builder::{parse_program_id, with_priority_fee},
    },
    sponsor,
    state::AppState,
};

/// Sessions a custodian may hold at once (prepared within the opening window, or open).
pub const MAX_LIVE_SESSIONS_PER_CUSTODIAN: i64 = 16;

pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<PreparedSessionResponse>), ApiError> {
    let custodian = parse_hex32("custodian", &body.custodian)?;
    if custodian == [0; 32]
        || !(1..=MAX_SESSION_EVENTS).contains(&body.max_events)
        || !(1..=MAX_SESSION_SECONDS).contains(&body.duration_seconds)
    {
        return Err(ApiError::Validation(
            "custodian must be non-zero, maxEvents within 1..=2000 and durationSeconds within 1..=43200"
                .into(),
        ));
    }
    // A1: only the custodian's own organization, or the operator for a known custodian, may
    // prepare a session the sponsor would pay for; and a custodian holds a bounded number.
    authorize_custodian(&state, &headers, custodian, false).await?;
    if sessions::count_live(&state.db, state.config.deployment_id, custodian).await?
        >= MAX_LIVE_SESSIONS_PER_CUSTODIAN
    {
        return Err(ApiError::RateLimited(
            "this custodian already has the maximum number of live sessions".into(),
        ));
    }
    let station_id =
        derive_station_id(&state.config.station_pubkey33).map_err(|_| ApiError::Internal)?;
    // S5: a session is located at the facility its Station is installed at.
    let station = state
        .rpc
        .v2_station(state.config.deployment_id, station_id)
        .await?
        .ok_or_else(|| ApiError::Conflict("the configured Station is not registered".into()))?;
    if station.facility_id == [0; 32] {
        return Err(ApiError::Conflict(
            "the Station is not installed at a facility; bind it before opening sessions".into(),
        ));
    }
    let valid_from = OffsetDateTime::now_utc().unix_timestamp();
    let session = SessionRecord {
        // The program only accepts the id derived from custodian, Station and terms (M1).
        session_id: lastro_protocol::v2::session_id(
            &state.config.deployment_id,
            &custodian,
            &station_id,
            valid_from,
            valid_from + body.duration_seconds,
            body.max_events,
        ),
        deployment_id: state.config.deployment_id,
        custodian,
        station_id,
        facility_id: station.facility_id,
        max_events: body.max_events,
        valid_from,
        expires_at: valid_from + body.duration_seconds,
        revoked_at: None,
        status: "PREPARED".into(),
        open_tx_signature: None,
    };
    sessions::insert_prepared(&state.db, &session).await?;
    let transaction = with_priority_fee(
        open_transaction(&state, &session, None)?,
        state.config.priority_fee_micro_lamports,
    )?;
    Ok((
        StatusCode::CREATED,
        Json(PreparedSessionResponse {
            session: session_response(&session),
            transaction,
        }),
    ))
}

pub async fn get(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionResponse>, ApiError> {
    let session = load(&state, &session_id).await?;
    Ok(Json(session_response(&session)))
}

/// Admits a finalized `open_session` and freezes what the Agent will be told.
pub async fn confirm(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(body): Json<ConfirmEventRequest>,
) -> Result<Json<SessionResponse>, ApiError> {
    validate_transaction_signature(&body.tx_signature)?;
    let session = load(&state, &session_id).await?;
    if session.status != "PREPARED" {
        if session.open_tx_signature.as_deref() == Some(body.tx_signature.as_str()) {
            return Ok(Json(session_response(&session)));
        }
        return Err(ApiError::Conflict(
            "session is not awaiting its opening".into(),
        ));
    }
    let custodian = Pubkey::new_from_array(session.custodian);
    if !matches_any(
        &state,
        &body.tx_signature,
        |payer| open_transaction(&state, &session, Some(payer)),
        custodian,
    )
    .await?
    {
        return Err(ApiError::Conflict(
            "transaction is not the finalized opening of this session".into(),
        ));
    }
    let canonical = canonical_session(&state, &session).await?;
    if canonical.custodian != custodian
        || canonical.station_id != session.station_id
        || canonical.max_events != session.max_events
        || canonical.valid_from != session.valid_from
        || canonical.expires_at != session.expires_at
        || canonical.allowed_actions != SESSION_ACTION_OBSERVE
        || canonical.facility_id != session.facility_id
    {
        return Err(ApiError::Conflict(
            "canonical SessionGrant differs from the prepared terms".into(),
        ));
    }
    sessions::open_with_manifest(
        &state.db,
        session.session_id,
        &body.tx_signature,
        MAX_SESSION_EVENTS,
    )
    .await?;
    Ok(Json(session_response(&load(&state, &session_id).await?)))
}

/// The frozen manifest an Agent needs to build every presence proof offline.
pub async fn manifest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Result<Json<SessionManifestResponse>, ApiError> {
    authorize(&headers, &state.config.agent_token)?;
    let session = load(&state, &session_id).await?;
    if session.status == "PREPARED" {
        return Err(ApiError::Conflict("session is not open".into()));
    }
    let assets = sessions::manifest(&state.db, session.session_id)
        .await?
        .into_iter()
        .map(|entry| SessionManifestAsset {
            asset_id: hex::encode(entry.asset_id),
            rfid_hash: hex::encode(entry.rfid_hash),
            state_version: entry.state_version,
            last_event_hash: hex::encode(entry.last_event_hash),
        })
        .collect();
    Ok(Json(SessionManifestResponse {
        session_id: hex::encode(session.session_id),
        deployment_id: hex::encode(session.deployment_id),
        station_id: hex::encode(session.station_id),
        valid_from: session.valid_from,
        expires_at: session.observation_deadline(),
        assets,
    }))
}

/// Admits one Station-signed presence proof read during the session.
pub async fn ingest_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
    Json(body): Json<SessionObservationRequest>,
) -> Result<(StatusCode, Json<SessionObservationResponse>), ApiError> {
    authorize(&headers, &state.config.agent_token)?;
    let envelope_bytes =
        decode_base64_array::<V2_ENVELOPE_LEN>("envelopeBase64", &body.envelope_base64)?;
    let observed_rfid = decode_hex_array::<8>("observedRfidHex", &body.observed_rfid_hex)?;
    let station_pubkey = decode_hex_array::<33>("stationPubkeyHex", &body.station_pubkey_hex)?;
    let station_signature =
        decode_hex_array::<64>("stationSignatureHex", &body.station_signature_hex)?;
    let session = load(&state, &session_id).await?;
    if session.status == "PREPARED" {
        return Err(ApiError::Conflict("session is not open".into()));
    }
    let envelope = DomainEventEnvelope::decode(&envelope_bytes)
        .map_err(|error| ApiError::Validation(format!("invalid v2 domain envelope: {error}")))?;
    if envelope.deployment_id != state.config.deployment_id
        || envelope.event_type != EventType::ObservationRecorded as u16
    {
        return Err(ApiError::Validation(
            "session evidence must be an ObservationRecorded envelope of this deployment".into(),
        ));
    }
    if station_pubkey != state.config.station_pubkey33
        || derive_station_id(&station_pubkey).ok() != Some(envelope.source_id)
        || envelope.source_id != session.station_id
    {
        return Err(ApiError::Conflict(
            "observation was not signed by this session's Station".into(),
        ));
    }
    let now = OffsetDateTime::now_utc().unix_timestamp();
    if envelope.observed_at < session.valid_from
        || envelope.observed_at > session.observation_deadline()
        || envelope.observed_at > now
        || now > envelope.observed_at + MAX_BATCH_EVENT_AGE_SECONDS
    {
        return Err(ApiError::Conflict(
            "observation is outside the session window or its offline anchoring deadline".into(),
        ));
    }
    let entry = sessions::manifest_entry(&state.db, session.session_id, envelope.subject_id)
        .await?
        .ok_or_else(|| ApiError::Conflict("asset is not in this session's manifest".into()))?;
    let context = SessionAssetContext {
        deployment_id: state.config.deployment_id,
        session_id: session.session_id,
        asset_id: entry.asset_id,
        state_version: entry.state_version,
        previous_event_hash: entry.last_event_hash,
        rfid_hash: entry.rfid_hash,
    };
    if hash_canonical_rfid(&observed_rfid) != entry.rfid_hash || !context.matches(&envelope) {
        return Err(ApiError::Conflict(
            "observation does not match this asset's session context".into(),
        ));
    }
    verify_station_signature_bytes(&envelope_bytes, &station_pubkey, &station_signature)
        .map_err(|error| ApiError::Validation(format!("invalid v2 Station signature: {error}")))?;
    let event_hash = envelope.event_hash().map_err(|_| ApiError::Internal)?;
    let inserted = sessions::insert_observation(
        &state.db,
        &SessionObservation {
            event_hash,
            session_id: session.session_id,
            asset_id: entry.asset_id,
            envelope: envelope_bytes,
            station_pubkey,
            station_signature,
            observed_rfid,
            observed_at: envelope.observed_at,
            batch_index: None,
            leaf_index: None,
            status: "PENDING".into(),
        },
    )
    .await?;
    Ok((
        if inserted {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(SessionObservationResponse {
            event_hash: hex::encode(event_hash),
            asset_id: hex::encode(entry.asset_id),
            status: "PENDING".into(),
        }),
    ))
}

/// Prepares the next batch: the oldest pending proofs that still fit the session capacity.
/// Repeating the call returns the same prepared batch until it is anchored.
pub async fn prepare_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Result<Json<PreparedBatchResponse>, ApiError> {
    let session = load(&state, &session_id).await?;
    authorize_custodian(&state, &headers, session.custodian, true).await?;
    let canonical = canonical_session(&state, &session).await?;
    let index = canonical.batch_count;
    let batch = match sessions::find_batch(&state.db, session.session_id, index).await? {
        Some(batch) if batch.status == "PREPARED" => batch,
        Some(_) => {
            return Err(ApiError::Conflict(
                "the chain has not admitted the previous batch yet; confirm it first".into(),
            ));
        }
        None => {
            let capacity = canonical
                .max_events
                .saturating_sub(canonical.events_used)
                .min(MAX_BATCH_LEAVES);
            let pending =
                sessions::pending_observations(&state.db, session.session_id, capacity).await?;
            let (Some(first), Some(last)) = (pending.first(), pending.last()) else {
                return Err(ApiError::Conflict(
                    "no pending observations fit this session".into(),
                ));
            };
            let leaves: Vec<[u8; 32]> = pending
                .iter()
                .map(|observation| {
                    batch_leaf_hash(&observation.envelope, &observation.station_signature)
                })
                .collect();
            let batch = BatchRecord {
                session_id: session.session_id,
                batch_index: index,
                merkle_root: batch_merkle_root(&leaves).map_err(|_| ApiError::Internal)?,
                leaf_count: u32::try_from(leaves.len()).map_err(|_| ApiError::Internal)?,
                first_observed_at: first.observed_at,
                last_observed_at: last.observed_at,
                status: "PREPARED".into(),
                tx_signature: None,
            };
            let event_hashes: Vec<[u8; 32]> = pending
                .iter()
                .map(|observation| observation.event_hash)
                .collect();
            sessions::prepare_batch(&state.db, &batch, &event_hashes).await?;
            batch
        }
    };
    let transaction = with_priority_fee(
        batch_transaction(&state, &session, &batch, None)?,
        state.config.priority_fee_micro_lamports,
    )?;
    Ok(Json(PreparedBatchResponse {
        batch: batch_response(&batch),
        transaction,
    }))
}

/// Admits a finalized batch: the exact transaction and the BatchAnchor it created.
pub async fn confirm_batch(
    State(state): State<AppState>,
    Path((session_id, batch_index)): Path<(String, u32)>,
    Json(body): Json<ConfirmEventRequest>,
) -> Result<Json<SessionBatchResponse>, ApiError> {
    validate_transaction_signature(&body.tx_signature)?;
    let session = load(&state, &session_id).await?;
    let batch = sessions::find_batch(&state.db, session.session_id, batch_index)
        .await?
        .ok_or_else(|| ApiError::NotFound("batch not found".into()))?;
    if batch.status == "ANCHORED" {
        if batch.tx_signature.as_deref() == Some(body.tx_signature.as_str()) {
            return Ok(Json(batch_response(&batch)));
        }
        return Err(ApiError::Conflict(
            "batch is anchored by another transaction".into(),
        ));
    }
    let custodian = Pubkey::new_from_array(session.custodian);
    if !matches_any(
        &state,
        &body.tx_signature,
        |payer| batch_transaction(&state, &session, &batch, Some(payer)),
        custodian,
    )
    .await?
    {
        return Err(ApiError::Conflict(
            "transaction is not the finalized anchoring of this batch".into(),
        ));
    }
    let anchored = state
        .rpc
        .v2_batch(state.config.deployment_id, session.session_id, batch_index)
        .await?
        .ok_or_else(|| {
            ApiError::Conflict("finalized transaction did not produce BatchAnchor".into())
        })?;
    if anchored.merkle_root != batch.merkle_root
        || anchored.leaf_count != batch.leaf_count
        || anchored.first_observed_at != batch.first_observed_at
        || anchored.last_observed_at != batch.last_observed_at
        || anchored.custodian != custodian
        || anchored.station_id != session.station_id
        || anchored.facility_id != session.facility_id
    {
        return Err(ApiError::Conflict(
            "canonical BatchAnchor differs from the prepared batch".into(),
        ));
    }
    sessions::mark_batch_anchored(
        &state.db,
        session.session_id,
        batch_index,
        &body.tx_signature,
    )
    .await?;
    let batch = sessions::find_batch(&state.db, session.session_id, batch_index)
        .await?
        .ok_or(ApiError::Internal)?;
    Ok(Json(batch_response(&batch)))
}

/// `revoke_session` for the custodian: no observation after it will be admitted.
pub async fn revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<String>,
) -> Result<Json<TransactionDataResponse>, ApiError> {
    let session = load(&state, &session_id).await?;
    authorize_custodian(&state, &headers, session.custodian, false).await?;
    if session.status != "OPEN" {
        return Err(ApiError::Conflict("session is not open".into()));
    }
    with_priority_fee(
        revoke_transaction(&state, &session, None)?,
        state.config.priority_fee_micro_lamports,
    )
    .map(Json)
}

/// Mirrors the canonical grant into the projection (e.g. after a revocation).
pub async fn sync(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionResponse>, ApiError> {
    let session = load(&state, &session_id).await?;
    if session.status == "OPEN" {
        let canonical = canonical_session(&state, &session).await?;
        if canonical.status == SESSION_STATUS_REVOKED {
            sessions::mark_revoked(&state.db, session.session_id, canonical.revoked_at).await?;
        }
    }
    Ok(Json(session_response(&load(&state, &session_id).await?)))
}

/// The exact `open_session` for this session; `payer` defaults to the current sponsor policy.
pub(crate) fn open_transaction(
    state: &AppState,
    session: &SessionRecord,
    payer: Option<Pubkey>,
) -> Result<TransactionDataResponse, ApiError> {
    let custodian = Pubkey::new_from_array(session.custodian);
    build_open_session(
        parse_program_id(&state.config.lastro_program_id)?,
        &session.deployment_id,
        custodian,
        payer.unwrap_or_else(|| sponsor::payer_for(state.sponsor(), custodian)),
        SessionTerms {
            session_id: session.session_id,
            station_id: session.station_id,
            facility_id: session.facility_id,
            allowed_actions: SESSION_ACTION_OBSERVE,
            max_events: session.max_events,
            valid_from: session.valid_from,
            expires_at: session.expires_at,
        },
    )
}

pub(crate) fn batch_transaction(
    state: &AppState,
    session: &SessionRecord,
    batch: &BatchRecord,
    payer: Option<Pubkey>,
) -> Result<TransactionDataResponse, ApiError> {
    let custodian = Pubkey::new_from_array(session.custodian);
    build_anchor_batch(
        parse_program_id(&state.config.lastro_program_id)?,
        &session.deployment_id,
        custodian,
        payer.unwrap_or_else(|| sponsor::payer_for(state.sponsor(), custodian)),
        BatchTerms {
            session_id: session.session_id,
            station_id: session.station_id,
            batch_index: batch.batch_index,
            merkle_root: batch.merkle_root,
            leaf_count: batch.leaf_count,
            first_observed_at: batch.first_observed_at,
            last_observed_at: batch.last_observed_at,
        },
    )
}

pub(crate) fn revoke_transaction(
    state: &AppState,
    session: &SessionRecord,
    payer: Option<Pubkey>,
) -> Result<TransactionDataResponse, ApiError> {
    let custodian = Pubkey::new_from_array(session.custodian);
    build_revoke_session(
        parse_program_id(&state.config.lastro_program_id)?,
        &session.deployment_id,
        &session.session_id,
        custodian,
        payer.unwrap_or_else(|| sponsor::payer_for(state.sponsor(), custodian)),
    )
}

pub(crate) async fn load(state: &AppState, session_id: &str) -> Result<SessionRecord, ApiError> {
    let session_id = parse_hex32("sessionId", session_id)?;
    sessions::find(&state.db, state.config.deployment_id, session_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("session not found".into()))
}

/// Whether `signature` is the finalized transaction `build` prepares for any accepted payer
/// (sponsored or self-paid), priority fee included or not.
async fn matches_any<F>(
    state: &AppState,
    signature: &str,
    build: F,
    signer: Pubkey,
) -> Result<bool, ApiError>
where
    F: Fn(Pubkey) -> Result<TransactionDataResponse, ApiError>,
{
    for payer in sponsor::payer_candidates(state.sponsor(), signer) {
        if state
            .rpc
            .transaction_matches(signature, &build(payer)?)
            .await?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn canonical_session(
    state: &AppState,
    session: &SessionRecord,
) -> Result<CanonicalV2Session, ApiError> {
    state
        .rpc
        .v2_session(state.config.deployment_id, session.session_id)
        .await?
        .ok_or_else(|| ApiError::Conflict("canonical SessionGrant not found".into()))
}

fn session_response(session: &SessionRecord) -> SessionResponse {
    SessionResponse {
        session_id: hex::encode(session.session_id),
        custodian: hex::encode(session.custodian),
        station_id: hex::encode(session.station_id),
        facility_id: hex::encode(session.facility_id),
        max_events: session.max_events,
        valid_from: session.valid_from,
        expires_at: session.expires_at,
        revoked_at: session.revoked_at,
        status: session.status.clone(),
        open_tx_signature: session.open_tx_signature.clone(),
    }
}

fn batch_response(batch: &BatchRecord) -> SessionBatchResponse {
    SessionBatchResponse {
        session_id: hex::encode(batch.session_id),
        batch_index: batch.batch_index,
        merkle_root: hex::encode(batch.merkle_root),
        leaf_count: batch.leaf_count,
        first_observed_at: batch.first_observed_at,
        last_observed_at: batch.last_observed_at,
        status: batch.status.clone(),
        tx_signature: batch.tx_signature.clone(),
    }
}
