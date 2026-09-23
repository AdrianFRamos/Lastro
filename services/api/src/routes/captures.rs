//! Capture creation/status endpoints. Context is derived from canonical Solana state, never request-supplied history.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ed25519_dalek::{Signature, VerifyingKey};
use lastro_protocol::crypto::derive_station_id;
use sqlx::{Postgres, Transaction};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{
    error::ApiError,
    model::{
        CaptureAction, CaptureAuthorizationChallengeResponse, CaptureAuthorizationProof,
        CaptureIntentRequest, CaptureResponse, CaptureStatus, CreateCaptureRequest, EventStatus,
        parse_hex32,
    },
    repository::{
        animals,
        capture_authorizations::{self, NewCaptureAuthorization},
        captures::{self, CaptureRecord, CaptureState, NewCapture},
        events::{self, EventRecord},
    },
    state::AppState,
};

const ZERO32: [u8; 32] = [0; 32];
const CAPTURE_AUTHORIZATION_TTL: Duration = Duration::minutes(2);

enum PreparedCapture {
    Existing {
        capture: CaptureRecord,
        event: EventRecord,
        required_signer: [u8; 32],
    },
    New {
        capture: NewCapture,
        required_signer: [u8; 32],
    },
    Superseding {
        capture: NewCapture,
        required_signer: [u8; 32],
        superseded_capture_id: Uuid,
        superseded_event_hash: [u8; 32],
    },
}

struct NewCapturePlan {
    capture: NewCapture,
    required_signer: [u8; 32],
}

impl PreparedCapture {
    fn required_signer(&self) -> [u8; 32] {
        match self {
            Self::Existing {
                required_signer, ..
            }
            | Self::New {
                required_signer, ..
            }
            | Self::Superseding {
                required_signer, ..
            } => *required_signer,
        }
    }
}

pub async fn authorization_challenge(
    State(state): State<AppState>,
    Json(body): Json<CaptureIntentRequest>,
) -> Result<(StatusCode, Json<CaptureAuthorizationChallengeResponse>), ApiError> {
    let animal_id = parse_hex32("animalId", &body.animal_id)?;
    let next = body
        .next_custodian
        .as_deref()
        .map(|value| parse_hex32("nextCustodian", value))
        .transpose()?;
    capture_authorizations::preflight_rate(&state.db, animal_id).await?;
    let prepared = prepare_capture(
        &state,
        body.action,
        animal_id,
        next,
        body.supersede_capture_id,
    )
    .await?;
    let required_signer = prepared.required_signer();

    let challenge_id = Uuid::new_v4();
    let expires_at = OffsetDateTime::now_utc() + CAPTURE_AUTHORIZATION_TTL;
    let message = authorization_message(
        challenge_id,
        state.config.deployment_id,
        &state.config.lastro_program_id,
        body.action,
        animal_id,
        next,
        body.supersede_capture_id,
        required_signer,
        expires_at.unix_timestamp(),
    );
    let record = capture_authorizations::insert(
        &state.db,
        &NewCaptureAuthorization {
            challenge_id,
            deployment_id: state.config.deployment_id,
            action: action_code(body.action),
            animal_id,
            next_custodian: next,
            supersede_capture_id: body.supersede_capture_id,
            required_signer,
            message_bytes: message.as_bytes(),
            expires_at,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(CaptureAuthorizationChallengeResponse {
            challenge_id: record.challenge_id,
            deployment_id: hex::encode(record.deployment_id),
            required_signer: bs58::encode(record.required_signer).into_string(),
            message_base64: BASE64.encode(&record.message_bytes),
            expires_at_unix: record.expires_at.unix_timestamp(),
        }),
    ))
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateCaptureRequest>,
) -> Result<(StatusCode, Json<CaptureResponse>), ApiError> {
    let animal_id = parse_hex32("animalId", &body.animal_id)?;
    let next = body
        .next_custodian
        .as_deref()
        .map(|value| parse_hex32("nextCustodian", value))
        .transpose()?;
    let prepared = prepare_capture(
        &state,
        body.action,
        animal_id,
        next,
        body.supersede_capture_id,
    )
    .await?;

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;
    verify_and_consume_authorization(
        &state,
        &mut tx,
        &body.authorization,
        body.action,
        animal_id,
        next,
        body.supersede_capture_id,
        prepared.required_signer(),
    )
    .await?;

    let response = match prepared {
        PreparedCapture::Existing { capture, event, .. } => {
            events::require_active_event_tx(&mut tx, event.event_hash).await?;
            (StatusCode::OK, Json(to_response(capture, Some(event))?))
        }
        PreparedCapture::New { capture, .. } => {
            let record = captures::insert_pending_tx(&mut tx, &capture).await?;
            (StatusCode::CREATED, Json(to_response(record, None)?))
        }
        PreparedCapture::Superseding {
            capture,
            superseded_capture_id,
            superseded_event_hash,
            ..
        } => {
            events::mark_rejected_tx(&mut tx, superseded_event_hash).await?;
            captures::cancel_evidence_accepted_tx(&mut tx, superseded_capture_id).await?;
            let record = captures::insert_pending_tx(&mut tx, &capture).await?;
            (StatusCode::CREATED, Json(to_response(record, None)?))
        }
    };
    tx.commit()
        .await
        .map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;
    Ok(response)
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CaptureResponse>, ApiError> {
    let record = captures::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound("capture not found".into()))?;
    let event = events::find_by_capture_id(&state.db, id).await?;
    Ok(Json(to_response(record, event)?))
}

async fn prepare_capture(
    state: &AppState,
    action: CaptureAction,
    animal_id: [u8; 32],
    next: Option<[u8; 32]>,
    supersede_capture_id: Option<Uuid>,
) -> Result<PreparedCapture, ApiError> {
    if supersede_capture_id.is_some() && action != CaptureAction::Reidentify {
        return Err(ApiError::Validation(
            "explicit same-action recapture is only available for REIDENTIFY".into(),
        ));
    }
    if let Some(event) = events::find_unfinalized_for_animal(&state.db, animal_id).await? {
        if let Some(expected) = supersede_capture_id {
            if expected != event.capture_id
                || event.event.action != lastro_protocol::Action::Reidentify
            {
                return Err(ApiError::Conflict(
                    "explicit recapture must name the current accepted REIDENTIFY capture".into(),
                ));
            }
        }
        if supersede_capture_id.is_none() && retry_intent_matches(&action, next, &event) {
            let capture = captures::find_by_id(&state.db, event.capture_id)
                .await?
                .ok_or_else(|| {
                    ApiError::Conflict("accepted event is missing its capture context".into())
                })?;
            let required_signer = match action {
                CaptureAction::Origin => event.event.to_custodian,
                CaptureAction::Transfer | CaptureAction::Reidentify => event.event.from_custodian,
            };
            return Ok(PreparedCapture::Existing {
                capture,
                event,
                required_signer,
            });
        }

        if event.status == "SUBMITTED" || event.tx_signature.is_some() {
            return Err(ApiError::Conflict(
                "animal already has a submitted transition awaiting canonical finalization".into(),
            ));
        }
        if event.status != "EVIDENCE_ACCEPTED" {
            return Err(ApiError::Conflict(
                "animal has an unfinalized transition in an invalid supersession state".into(),
            ));
        }

        let replacement = prepare_new_capture(state, action, animal_id, next).await?;
        return Ok(PreparedCapture::Superseding {
            capture: replacement.capture,
            required_signer: replacement.required_signer,
            superseded_capture_id: event.capture_id,
            superseded_event_hash: event.event_hash,
        });
    }

    if supersede_capture_id.is_some() {
        return Err(ApiError::Conflict(
            "the accepted capture to replace no longer exists".into(),
        ));
    }

    let plan = prepare_new_capture(state, action, animal_id, next).await?;
    Ok(PreparedCapture::New {
        capture: plan.capture,
        required_signer: plan.required_signer,
    })
}

async fn prepare_new_capture(
    state: &AppState,
    action: CaptureAction,
    animal_id: [u8; 32],
    next: Option<[u8; 32]>,
) -> Result<NewCapturePlan, ApiError> {
    let local = animals::find_by_animal_id(&state.db, animal_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("animal not registered".into()))?;
    let canonical = state
        .rpc
        .animal_state(state.config.deployment_id, animal_id)
        .await?;
    let station_id = derive_station_id(&state.config.station_pubkey33)
        .map_err(|_| ApiError::Config("configured Station public key is invalid".into()))?;

    match action {
        CaptureAction::Origin => {
            if canonical.is_some() || local.event_sequence != 0 || local.identity_revision != 0 {
                return Err(ApiError::Conflict(
                    "ORIGIN requires an unoriginated AnimalID".into(),
                ));
            }
            let to =
                next.ok_or_else(|| ApiError::Validation("ORIGIN requires nextCustodian".into()))?;
            if to == ZERO32 {
                return Err(ApiError::Validation(
                    "nextCustodian must be non-zero".into(),
                ));
            }
            Ok(NewCapturePlan {
                required_signer: to,
                capture: NewCapture {
                    station_id,
                    action: 1,
                    animal_id,
                    event_sequence: 1,
                    identity_revision: 1,
                    expected_old_rfid_hash: ZERO32,
                    from_custodian: ZERO32,
                    to_custodian: to,
                    previous_event_hash: ZERO32,
                },
            })
        }
        CaptureAction::Transfer => {
            let current = canonical.ok_or_else(|| {
                ApiError::Conflict("TRANSFER requires canonical AnimalState".into())
            })?;
            let to =
                next.ok_or_else(|| ApiError::Validation("TRANSFER requires nextCustodian".into()))?;
            if to == ZERO32 || to == current.current_custodian {
                return Err(ApiError::Validation(
                    "TRANSFER destination must be non-zero and different from current custodian"
                        .into(),
                ));
            }
            Ok(NewCapturePlan {
                required_signer: current.current_custodian,
                capture: NewCapture {
                    station_id,
                    action: 2,
                    animal_id,
                    event_sequence: current
                        .event_sequence
                        .checked_add(1)
                        .ok_or(ApiError::Internal)?,
                    identity_revision: current.identity_revision,
                    expected_old_rfid_hash: current.current_rfid_hash,
                    from_custodian: current.current_custodian,
                    to_custodian: to,
                    previous_event_hash: current.last_event_hash,
                },
            })
        }
        CaptureAction::Reidentify => {
            if next.is_some() {
                return Err(ApiError::Validation(
                    "REIDENTIFY must not provide nextCustodian".into(),
                ));
            }
            let current = canonical.ok_or_else(|| {
                ApiError::Conflict("REIDENTIFY requires canonical AnimalState".into())
            })?;
            Ok(NewCapturePlan {
                required_signer: current.current_custodian,
                capture: NewCapture {
                    station_id,
                    action: 3,
                    animal_id,
                    event_sequence: current
                        .event_sequence
                        .checked_add(1)
                        .ok_or(ApiError::Internal)?,
                    identity_revision: current
                        .identity_revision
                        .checked_add(1)
                        .ok_or(ApiError::Internal)?,
                    expected_old_rfid_hash: current.current_rfid_hash,
                    from_custodian: current.current_custodian,
                    to_custodian: current.current_custodian,
                    previous_event_hash: current.last_event_hash,
                },
            })
        }
    }
}

async fn verify_and_consume_authorization(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    proof: &CaptureAuthorizationProof,
    action: CaptureAction,
    animal_id: [u8; 32],
    next: Option<[u8; 32]>,
    supersede_capture_id: Option<Uuid>,
    required_signer: [u8; 32],
) -> Result<(), ApiError> {
    let record = capture_authorizations::find_for_update(tx, proof.challenge_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    if record.used_at.is_some() {
        return Err(ApiError::Conflict(
            "capture authorization challenge was already consumed".into(),
        ));
    }
    if record.expires_at <= OffsetDateTime::now_utc() {
        return Err(ApiError::Conflict(
            "capture authorization challenge expired".into(),
        ));
    }
    if record.deployment_id != state.config.deployment_id
        || record.action != action_code(action)
        || record.animal_id != animal_id
        || record.next_custodian != next
        || record.supersede_capture_id != supersede_capture_id
        || record.required_signer != required_signer
    {
        return Err(ApiError::Conflict(
            "capture authorization challenge does not match current capture intent".into(),
        ));
    }

    let signature_bytes = BASE64
        .decode(&proof.signature_base64)
        .map_err(|_| ApiError::Unauthorized)?;
    if signature_bytes.len() != 64 || BASE64.encode(&signature_bytes) != proof.signature_base64 {
        return Err(ApiError::Unauthorized);
    }
    let signature =
        Signature::try_from(signature_bytes.as_slice()).map_err(|_| ApiError::Unauthorized)?;
    let key = VerifyingKey::from_bytes(&required_signer).map_err(|_| ApiError::Unauthorized)?;
    key.verify_strict(&record.message_bytes, &signature)
        .map_err(|_| ApiError::Unauthorized)?;

    if !capture_authorizations::consume_if_active_tx(tx, proof.challenge_id).await? {
        return Err(ApiError::Conflict(
            "capture authorization challenge is no longer active".into(),
        ));
    }
    Ok(())
}

fn authorization_message(
    challenge_id: Uuid,
    deployment_id: [u8; 32],
    program_id: &str,
    action: CaptureAction,
    animal_id: [u8; 32],
    next: Option<[u8; 32]>,
    supersede_capture_id: Option<Uuid>,
    required_signer: [u8; 32],
    expires_at_unix: i64,
) -> String {
    let next_custodian = next.map(hex::encode).unwrap_or_else(|| "none".into());
    format!(
        "Lastro capture authorization v1\nchallengeId={challenge_id}\ndeploymentId={}\nprogramId={program_id}\naction={}\nanimalId={}\nnextCustodian={next_custodian}\n{}requiredSigner={}\nexpiresAtUnix={expires_at_unix}\n",
        hex::encode(deployment_id),
        action_name(action),
        hex::encode(animal_id),
        supersede_capture_id
            .map(|id| format!("supersedeCaptureId={id}\n"))
            .unwrap_or_default(),
        bs58::encode(required_signer).into_string(),
    )
}

const fn action_code(action: CaptureAction) -> u8 {
    match action {
        CaptureAction::Origin => 1,
        CaptureAction::Transfer => 2,
        CaptureAction::Reidentify => 3,
    }
}

const fn action_name(action: CaptureAction) -> &'static str {
    match action {
        CaptureAction::Origin => "ORIGIN",
        CaptureAction::Transfer => "TRANSFER",
        CaptureAction::Reidentify => "REIDENTIFY",
    }
}

fn retry_intent_matches(
    action: &CaptureAction,
    next: Option<[u8; 32]>,
    event: &EventRecord,
) -> bool {
    let action_matches = matches!(
        (action, event.event.action),
        (CaptureAction::Origin, lastro_protocol::Action::Origin)
            | (CaptureAction::Transfer, lastro_protocol::Action::Transfer)
            | (
                CaptureAction::Reidentify,
                lastro_protocol::Action::Reidentify
            )
    );
    if !action_matches {
        return false;
    }
    match action {
        CaptureAction::Origin | CaptureAction::Transfer => next == Some(event.event.to_custodian),
        CaptureAction::Reidentify => next.is_none(),
    }
}

fn to_response(
    record: CaptureRecord,
    event: Option<EventRecord>,
) -> Result<CaptureResponse, ApiError> {
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
            (
                Some(hex::encode(event.event_hash)),
                Some(status),
                event.tx_signature,
            )
        }
    };
    Ok(CaptureResponse {
        capture_id: record.capture_id,
        action: match record.action {
            1 => CaptureAction::Origin,
            2 => CaptureAction::Transfer,
            3 => CaptureAction::Reidentify,
            _ => unreachable!("database CHECK"),
        },
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
