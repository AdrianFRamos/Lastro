//! `POST /api/v2/sponsor/submit` — the platform pays the fee and rent of a transaction the
//! authorizing wallet already signed (SPEC v2.1, S1).
//!
//! The caller names what the transaction is for; the API re-derives that exact transaction
//! with the same code as the `transaction-data` routes and co-signs only a byte-identical
//! message. The sponsor never signs anything it did not prepare, never becomes an authority,
//! and stops at the per-transaction and rolling daily budgets.

use axum::{Json, extract::State};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use solana_pubkey::Pubkey;
use std::str::FromStr;

use crate::{
    error::ApiError,
    model::{
        CustodyTransferPhase, RegisterAssetRequest, SponsorPurpose, SponsorSubmitRequest,
        SponsorSubmitResponse, TransactionDataResponse, parse_hex32,
    },
    repository,
    routes::{assets, domain_v2, operations, sessions},
    solana::v2_transaction_builder::with_priority_fee,
    sponsor::{self, SponsorableDiscriminators},
    state::AppState,
};

/// Solana's legacy wire limit; anything larger cannot be a prepared Lastro transaction.
const MAX_TRANSACTION_BYTES: usize = 1232;
/// `create_intent` data: discriminator, intent id, asset id, type, version, nonce, then
/// `expires_at` at this offset.
const CREATE_INTENT_EXPIRES_AT_OFFSET: usize = 8 + 32 + 32 + 2 + 8 + 8;

pub async fn submit(
    State(state): State<AppState>,
    Json(body): Json<SponsorSubmitRequest>,
) -> Result<Json<SponsorSubmitResponse>, ApiError> {
    if state.sponsor().is_none() {
        return Err(ApiError::Conflict("fee sponsorship is not enabled".into()));
    }
    let wire = BASE64
        .decode(&body.transaction_base64)
        .ok()
        .filter(|wire| {
            wire.len() <= MAX_TRANSACTION_BYTES && BASE64.encode(wire) == body.transaction_base64
        })
        .ok_or_else(|| {
            ApiError::Validation(
                "transactionBase64 must be a canonical legacy transaction of at most 1232 bytes"
                    .into(),
            )
        })?;

    let expected = prepare(&state, &body.purpose, &wire).await?;
    Ok(Json(
        sponsor_and_send(&state, &body.purpose, &wire, &expected).await?,
    ))
}

/// Co-signs `wire` (already signed by every other signer) as the fee sponsor, within the
/// budgets, and broadcasts it. `expected` is the exact transaction prepared for `purpose`.
pub(crate) async fn sponsor_and_send(
    state: &AppState,
    purpose: &SponsorPurpose,
    wire: &[u8],
    expected: &TransactionDataResponse,
) -> Result<SponsorSubmitResponse, ApiError> {
    let sponsor = state
        .sponsor()
        .ok_or_else(|| ApiError::Conflict("fee sponsorship is not enabled".into()))?;
    // I7: the sponsor must never also be the deployment authority it would be paying for.
    let config = state
        .rpc
        .v2_protocol_config(state.config.deployment_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("canonical v2 ProtocolConfig not found".into()))?;
    if config.authority == sponsor.pubkey() {
        return Err(ApiError::Conflict(
            "the fee sponsor must not be the deployment authority".into(),
        ));
    }
    // The sponsor pays only for wallets the platform knows; a throwaway key gets nothing.
    let signer = Pubkey::from_str(&expected.required_signer).map_err(|_| ApiError::Internal)?;
    if signer != config.authority
        && !repository::sponsor::is_known_signer(
            &state.db,
            state.config.deployment_id,
            signer.to_bytes(),
        )
        .await?
    {
        return Err(ApiError::Conflict(
            "sponsorship is limited to active parties and organization wallets".into(),
        ));
    }

    let signed = sponsor::co_sign(sponsor, wire, expected)?;
    let lamports =
        sponsor::estimate_cost_lamports(expected, &SponsorableDiscriminators::default())?;
    if lamports > sponsor.limits.max_lamports_per_transaction {
        return Err(ApiError::Conflict(
            "transaction exceeds the per-transaction sponsorship limit".into(),
        ));
    }
    let tx_signature = bs58::encode(signed.signatures[0]).into_string();
    repository::sponsor::reserve(
        &state.db,
        state.config.deployment_id,
        signer.to_bytes(),
        purpose.label(),
        &tx_signature,
        lamports,
        &sponsor.limits,
    )
    .await?;
    match state.rpc.send_transaction(&signed.encode()).await {
        Ok(returned) if returned == tx_signature => {}
        Ok(_) => {
            // The network acknowledged some transaction; keep the reservation (conservative).
            return Err(ApiError::Unavailable(
                "Solana RPC returned a different transaction signature".into(),
            ));
        }
        // Only a definite preflight rejection frees the budget; after a transport failure the
        // transaction may still land, so the reservation is kept (conservative).
        Err(error @ ApiError::Conflict(_)) => {
            repository::sponsor::release(&state.db, &tx_signature).await?;
            return Err(error);
        }
        Err(error) => return Err(error),
    }
    tracing::info!(
        purpose = purpose.label(),
        signer = %signer,
        lamports,
        tx = %tx_signature,
        "sponsored transaction broadcast"
    );
    Ok(SponsorSubmitResponse {
        tx_signature,
        fee_payer: sponsor.pubkey().to_string(),
        reserved_lamports: lamports,
        signed_transaction_base64: BASE64.encode(signed.encode()),
    })
}

/// The sponsor when it will really pay for `signer` (the deployment authority, an ACTIVE party
/// or an organization wallet, as `sponsor_and_send` requires); `None` otherwise, so the
/// transaction is prepared self-paid instead of naming a sponsor that would refuse it.
pub(crate) async fn sponsor_for(
    state: &AppState,
    signer: Pubkey,
    authority: Option<Pubkey>,
) -> Result<Option<&sponsor::Sponsor>, ApiError> {
    let Some(sponsor) = state.sponsor() else {
        return Ok(None);
    };
    if Some(signer) == authority
        || repository::sponsor::is_known_signer(
            &state.db,
            state.config.deployment_id,
            signer.to_bytes(),
        )
        .await?
    {
        Ok(Some(sponsor))
    } else {
        Ok(None)
    }
}

/// The exact transaction the matching `transaction-data` route prepares for `purpose`; a
/// custody proposal replays the expiry already inside `wire`.
async fn prepare(
    state: &AppState,
    purpose: &SponsorPurpose,
    wire: &[u8],
) -> Result<TransactionDataResponse, ApiError> {
    let expiry = match purpose {
        SponsorPurpose::CustodyPropose { .. } => Some(proposal_expiry(wire)?),
        _ => None,
    };
    prepare_purpose(state, purpose, expiry).await
}

/// The exact transaction the matching `transaction-data` route prepares for `purpose`.
/// `proposal_expiry` replays a custody proposal's expiry; `None` prepares a fresh one.
pub(crate) async fn prepare_purpose(
    state: &AppState,
    purpose: &SponsorPurpose,
    proposal_expiry: Option<i64>,
) -> Result<TransactionDataResponse, ApiError> {
    match purpose {
        SponsorPurpose::StationEvent { event_hash } => {
            let event_hash = parse_hex32("eventHash", event_hash)?;
            domain_v2::prepare_event_transaction(state, event_hash).await
        }
        SponsorPurpose::RegisterAsset {
            asset_id,
            custodian,
            asset_type,
            available_weight_grams,
        } => {
            assets::prepare_registration(
                state,
                &RegisterAssetRequest {
                    asset_id: asset_id.clone(),
                    custodian: custodian.clone(),
                    asset_type: *asset_type,
                    available_weight_grams: *available_weight_grams,
                },
            )
            .await
        }
        SponsorPurpose::CustodyPropose { transfer_id } => {
            // A replayed expiry is still bounded to the allowed window by the prepare step.
            operations::prepare_custody_transaction(
                state,
                *transfer_id,
                CustodyTransferPhase::Propose,
                proposal_expiry,
            )
            .await
        }
        SponsorPurpose::CustodyAccept { transfer_id } => {
            operations::prepare_custody_transaction(
                state,
                *transfer_id,
                CustodyTransferPhase::Accept,
                None,
            )
            .await
        }
        SponsorPurpose::SessionOpen { session_id } => {
            let session = sessions::load(state, session_id).await?;
            if session.status != "PREPARED" {
                return Err(ApiError::Conflict(
                    "session is not awaiting its opening".into(),
                ));
            }
            with_fee(state, sessions::open_transaction(state, &session, None)?)
        }
        SponsorPurpose::SessionBatch {
            session_id,
            batch_index,
        } => {
            let session = sessions::load(state, session_id).await?;
            let batch =
                repository::sessions::find_batch(&state.db, session.session_id, *batch_index)
                    .await?
                    .filter(|batch| batch.status == "PREPARED")
                    .ok_or_else(|| ApiError::Conflict("batch is not prepared".into()))?;
            with_fee(
                state,
                sessions::batch_transaction(state, &session, &batch, None)?,
            )
        }
        SponsorPurpose::SessionRevoke { session_id } => {
            let session = sessions::load(state, session_id).await?;
            if session.status != "OPEN" {
                return Err(ApiError::Conflict("session is not open".into()));
            }
            with_fee(state, sessions::revoke_transaction(state, &session, None)?)
        }
    }
}

fn with_fee(
    state: &AppState,
    transaction: TransactionDataResponse,
) -> Result<TransactionDataResponse, ApiError> {
    with_priority_fee(transaction, state.config.priority_fee_micro_lamports)
}

/// `expires_at` of the single `create_intent` instruction in `wire`. Only used to rebuild the
/// expected transaction, which is then compared byte for byte.
fn proposal_expiry(wire: &[u8]) -> Result<i64, ApiError> {
    let transaction = sponsor::LegacyTransaction::decode(wire)?;
    let data = transaction
        .instructions
        .first()
        .map(|instruction| instruction.data.as_slice())
        .filter(|data| data.len() == CREATE_INTENT_EXPIRES_AT_OFFSET + 8 + 32)
        .ok_or_else(|| {
            ApiError::Validation("transaction does not start with a custody proposal".into())
        })?;
    let bytes: [u8; 8] = data[CREATE_INTENT_EXPIRES_AT_OFFSET..CREATE_INTENT_EXPIRES_AT_OFFSET + 8]
        .try_into()
        .map_err(|_| ApiError::Internal)?;
    Ok(i64::from_le_bytes(bytes))
}
