//! Physical capture endpoints (protocol v2). The capture context always comes from the
//! canonical Solana AssetState, never from the request.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ed25519_dalek::{Signature, VerifyingKey};
use lastro_protocol::{crypto::derive_station_id, v2::capture_event_id};
use sqlx::{Postgres, Transaction};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{
    error::ApiError,
    model::{
        CaptureAction, CaptureAuthorizationChallengeResponse, CaptureAuthorizationProof,
        CaptureIntentRequest, CaptureResponse, CaptureStatus, CreateCaptureRequest,
        DomainEventStatus, parse_hex32,
    },
    repository::{
        captures_v2::{self, CaptureRecord, CaptureStatus as StoredStatus},
        domain_v2,
    },
    routes::domain_v2 as domain_v2_routes,
    solana::rpc::STATION_STATUS_ACTIVE,
    state::AppState,
};

const ZERO32: [u8; 32] = [0; 32];
const CHALLENGE_TTL: Duration = Duration::minutes(2);
/// The Station must observe the tag within this window; it is also the signed validity.
const CAPTURE_WINDOW_SECONDS: i64 = 300;
/// The program requires `observed_at <= Clock::unix_timestamp`, and the cluster clock (above all
/// at finalized commitment, used for preflight) runs behind wall time. The window therefore
/// opens this much before the capture, so `observed_at` is a lower bound, not the read instant.
const CLOCK_SKEW_ALLOWANCE_SECONDS: i64 = 300;
const ASSET_STATUS_CONSUMED: u8 = 3;
const ASSET_STATUS_CLOSED: u8 = 4;
const ASSET_STATUS_RETIRED: u8 = 7;

/// Context a new capture would be dispatched with, derived from canonical chain state.
struct CapturePlan {
    required_signer: [u8; 32],
    state_version: u64,
    previous_event_hash: [u8; 32],
    expected_rfid_hash: [u8; 32],
}

pub async fn authorization_challenge(
    State(state): State<AppState>,
    Json(body): Json<CaptureIntentRequest>,
) -> Result<(StatusCode, Json<CaptureAuthorizationChallengeResponse>), ApiError> {
    let asset_id = parse_hex32("assetId", &body.asset_id)?;
    captures_v2::preflight_rate(&state.db, asset_id).await?;
    let plan = plan_capture(&state, body.action, asset_id).await?;

    let challenge_id = Uuid::new_v4();
    let expires_at = OffsetDateTime::now_utc() + CHALLENGE_TTL;
    let message = authorization_message(
        challenge_id,
        state.config.deployment_id,
        &state.config.lastro_program_id,
        body.action,
        asset_id,
        plan.state_version,
        plan.required_signer,
        expires_at.unix_timestamp(),
    );
    let record = captures_v2::insert_challenge(
        &state.db,
        challenge_id,
        state.config.deployment_id,
        asset_id,
        body.action.event_type(),
        plan.required_signer,
        message.as_bytes(),
        expires_at,
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
    let asset_id = parse_hex32("assetId", &body.asset_id)?;
    let plan = plan_capture(&state, body.action, asset_id).await?;
    let existing = captures_v2::find_open_for_asset(&state.db, asset_id).await?;

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;
    verify_and_consume_authorization(&state, &mut tx, &body.authorization, &body, asset_id, &plan)
        .await?;

    if let Some(open) = existing {
        let same_intent = open.event_type == body.action.event_type()
            && open.state_version == plan.state_version
            && open.required_signer == plan.required_signer;
        if same_intent {
            // Idempotent retry: the same wallet asked for the same transition again.
            tx.commit()
                .await
                .map_err(|_| ApiError::Unavailable("postgres commit failed".into()))?;
            return Ok((StatusCode::OK, Json(response(&state, open).await?)));
        }
        supersede_unsigned(&state, &mut tx, &open).await?;
    }

    let capture_id = Uuid::new_v4();
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let station_id = derive_station_id(&state.config.station_pubkey33)
        .map_err(|_| ApiError::Config("configured Station public key is invalid".into()))?;
    let record = captures_v2::insert_capture_tx(
        &mut tx,
        &CaptureRecord {
            capture_id,
            deployment_id: state.config.deployment_id,
            station_id,
            asset_id,
            event_type: body.action.event_type(),
            event_id: capture_event_id(capture_id.as_bytes()),
            state_version: plan.state_version,
            previous_event_hash: plan.previous_event_hash,
            expected_rfid_hash: plan.expected_rfid_hash,
            required_signer: plan.required_signer,
            observed_at: now - CLOCK_SKEW_ALLOWANCE_SECONDS,
            expires_at: now + CAPTURE_WINDOW_SECONDS,
            status: StoredStatus::Pending,
            event_hash: None,
        },
    )
    .await?;
    tx.commit()
        .await
        .map_err(|_| ApiError::Unavailable("postgres commit failed".into()))?;
    Ok((StatusCode::CREATED, Json(response(&state, record).await?)))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CaptureResponse>, ApiError> {
    let record = captures_v2::find(&state.db, id)
        .await?
        .filter(|record| record.deployment_id == state.config.deployment_id)
        .ok_or_else(|| ApiError::NotFound("capture not found".into()))?;
    Ok(Json(response(&state, record).await?))
}

/// A different transition may replace an open capture only while nothing was signed:
/// pending/dispatched captures, or accepted evidence without a registered transaction.
///
/// Accepted evidence may have been signed and broadcast without the API being told. It is
/// recovered when its EventAnchor exists, and it is never rejected while it can still land
/// on-chain (until its `expires_at`), so the canonical history can never lose a finalized event.
async fn supersede_unsigned(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    open: &CaptureRecord,
) -> Result<(), ApiError> {
    if let Some(event_hash) = open.event_hash {
        let anchor = domain_v2::find_by_event_hash(&state.db, event_hash).await?;
        if anchor
            .as_ref()
            .is_some_and(|record| record.tx_signature.is_some())
        {
            return Err(ApiError::Conflict(
                "asset has a submitted transition awaiting canonical finalization".into(),
            ));
        }
        if domain_v2_routes::recover_unreported_event(state, event_hash).await? {
            // It was anchored after all: that capture is complete, nothing to supersede.
            return Ok(());
        }
        if OffsetDateTime::now_utc().unix_timestamp() <= open.expires_at {
            return Err(ApiError::Conflict(format!(
                "the previous capture's evidence can still be signed until unix time {}; retry after it expires",
                open.expires_at
            )));
        }
        domain_v2::reject_unsigned_at_version_tx(tx, open.asset_id, open.state_version).await?;
    }
    captures_v2::cancel_tx(tx, open.capture_id).await
}

/// The program rejects events from a Station that is not ACTIVE or is outside its registered
/// validity window. Refuse the capture up front, before the Station and a wallet sign anything
/// that could never be anchored.
pub(crate) async fn require_usable_station(
    state: &AppState,
    horizon_seconds: i64,
) -> Result<(), ApiError> {
    let station_id = derive_station_id(&state.config.station_pubkey33)
        .map_err(|_| ApiError::Config("configured Station public key is invalid".into()))?;
    let station = state
        .rpc
        .v2_station(state.config.deployment_id, station_id)
        .await?
        .ok_or_else(|| {
            ApiError::Conflict("the configured Station is not registered on-chain".into())
        })?;
    if station.status != STATION_STATUS_ACTIVE || station.pubkey33 != state.config.station_pubkey33
    {
        return Err(ApiError::Conflict(
            "the configured Station is not ACTIVE on-chain".into(),
        ));
    }
    let now = OffsetDateTime::now_utc().unix_timestamp();
    if now < station.valid_from || now.saturating_add(horizon_seconds) > station.valid_until {
        return Err(ApiError::Conflict(
            "the configured Station registration is outside its validity window; rotate the Station key"
                .into(),
        ));
    }
    Ok(())
}

async fn plan_capture(
    state: &AppState,
    action: CaptureAction,
    asset_id: [u8; 32],
) -> Result<CapturePlan, ApiError> {
    let asset = state
        .rpc
        .v2_asset_state(state.config.deployment_id, asset_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("asset is not registered on Solana".into()))?;
    if matches!(
        asset.status,
        ASSET_STATUS_CONSUMED | ASSET_STATUS_CLOSED | ASSET_STATUS_RETIRED
    ) {
        return Err(ApiError::Conflict("asset is in a terminal state".into()));
    }
    require_usable_station(state, CAPTURE_WINDOW_SECONDS).await?;
    let tagged = asset.current_rfid_hash != ZERO32;
    let required_signer = match action {
        CaptureAction::BindIdentifier if tagged => {
            return Err(ApiError::Conflict(
                "asset already has an active RFID; use REPLACE_IDENTIFIER".into(),
            ));
        }
        CaptureAction::ReplaceIdentifier | CaptureAction::ObservePresence if !tagged => {
            return Err(ApiError::Conflict(
                "asset has no active RFID; use BIND_IDENTIFIER".into(),
            ));
        }
        // Every capture is authorized by the asset's current custodian; the program also
        // accepts the deployment authority for presence proofs, but the API never needs it.
        CaptureAction::BindIdentifier
        | CaptureAction::ReplaceIdentifier
        | CaptureAction::ObservePresence => asset.custodian.to_bytes(),
    };
    Ok(CapturePlan {
        required_signer,
        state_version: asset
            .state_version
            .checked_add(1)
            .ok_or(ApiError::Internal)?,
        previous_event_hash: asset.last_event_hash,
        expected_rfid_hash: asset.current_rfid_hash,
    })
}

async fn verify_and_consume_authorization(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    proof: &CaptureAuthorizationProof,
    body: &CreateCaptureRequest,
    asset_id: [u8; 32],
    plan: &CapturePlan,
) -> Result<(), ApiError> {
    let record = captures_v2::challenge_for_update(tx, proof.challenge_id)
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
        || record.asset_id != asset_id
        || record.event_type != body.action.event_type()
        || record.required_signer != plan.required_signer
    {
        return Err(ApiError::Conflict(
            "capture authorization challenge does not match the current capture intent".into(),
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
    let key =
        VerifyingKey::from_bytes(&plan.required_signer).map_err(|_| ApiError::Unauthorized)?;
    key.verify_strict(&record.message_bytes, &signature)
        .map_err(|_| ApiError::Unauthorized)?;
    if !captures_v2::consume_challenge_tx(tx, proof.challenge_id).await? {
        return Err(ApiError::Conflict(
            "capture authorization challenge is no longer active".into(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authorization_message(
    challenge_id: Uuid,
    deployment_id: [u8; 32],
    program_id: &str,
    action: CaptureAction,
    asset_id: [u8; 32],
    state_version: u64,
    required_signer: [u8; 32],
    expires_at_unix: i64,
) -> String {
    format!(
        "Lastro capture authorization v2\nchallengeId={challenge_id}\ndeploymentId={}\nprogramId={program_id}\naction={}\nassetId={}\nstateVersion={state_version}\nrequiredSigner={}\nexpiresAtUnix={expires_at_unix}\n",
        hex::encode(deployment_id),
        action.wire_name(),
        hex::encode(asset_id),
        bs58::encode(required_signer).into_string(),
    )
}

async fn response(state: &AppState, record: CaptureRecord) -> Result<CaptureResponse, ApiError> {
    let anchor = match record.event_hash {
        Some(hash) => domain_v2::find_by_event_hash(&state.db, hash).await?,
        None => None,
    };
    Ok(CaptureResponse {
        capture_id: record.capture_id,
        action: CaptureAction::from_event_type(record.event_type).ok_or(ApiError::Internal)?,
        asset_id: hex::encode(record.asset_id),
        state_version: record.state_version,
        status: match record.status {
            StoredStatus::Pending => CaptureStatus::Pending,
            StoredStatus::Dispatched => CaptureStatus::Dispatched,
            StoredStatus::EvidenceAccepted => CaptureStatus::EvidenceAccepted,
            StoredStatus::Expired => CaptureStatus::Expired,
            StoredStatus::Cancelled => CaptureStatus::Cancelled,
        },
        event_hash: record.event_hash.map(hex::encode),
        event_status: anchor.as_ref().map(|record| match record.status.as_str() {
            "SUBMITTED" => DomainEventStatus::Submitted,
            "FINALIZED" => DomainEventStatus::Finalized,
            "REJECTED" => DomainEventStatus::Rejected,
            _ => DomainEventStatus::EvidenceAccepted,
        }),
        tx_signature: anchor.and_then(|record| record.tx_signature),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_message_binds_every_intent_field() {
        // PURPOSE: the wallet signs exactly which asset, transition and version it authorizes.
        // ASSERT: the message names action, asset, version, signer and expiry.
        // FAILURE MEANS: a signed challenge could be replayed for a different transition.
        let message = authorization_message(
            Uuid::nil(),
            [1; 32],
            "Program",
            CaptureAction::ReplaceIdentifier,
            [2; 32],
            7,
            [3; 32],
            99,
        );
        assert!(message.starts_with("Lastro capture authorization v2\n"));
        assert!(message.contains("action=IDENTIFIER_REPLACED\n"));
        assert!(message.contains(&format!("assetId={}\n", hex::encode([2; 32]))));
        assert!(message.contains("stateVersion=7\n"));
        assert!(message.contains("expiresAtUnix=99\n"));
    }
}
