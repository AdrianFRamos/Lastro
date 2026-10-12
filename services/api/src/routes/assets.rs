//! Asset endpoints (protocol v2): registration transaction, canonical reads, RFID lookup and
//! the public evidence package. Every answer is derived from finalized Solana state; the
//! PostgreSQL projection is only refreshed from it.

use axum::{
    Json,
    extract::{Path, State},
};

use crate::{
    error::ApiError,
    model::{
        AssetResponse, BatchedObservationProof, CustodyProofResponse, EvidencePackageV2,
        FacilityBoundaryProof, RegisterAssetRequest, TransactionDataResponse, parse_hex32,
    },
    repository::{domain_v2, facility_boundaries, operations, sessions},
    routes::domain_v2::to_response as event_response,
    solana::{
        rpc::{BindingStatus, CanonicalV2AssetState},
        v2_transaction_builder::{build_register_asset, with_priority_fee},
    },
    sponsor,
    state::AppState,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use lastro_protocol::v2::{batch_leaf_hash, batch_merkle_proof};
use serde::Serialize;
use solana_pubkey::Pubkey;
use std::collections::HashMap;

const EVIDENCE_PACKAGE_SCHEMA: &str = "lastro.evidence-package.v2";

/// `register_asset` for the deployment authority's wallet. Nothing is stored: the asset
/// exists only once the authority signs and the chain finalizes it (then call `sync`).
pub async fn registration_transaction(
    State(state): State<AppState>,
    Json(body): Json<RegisterAssetRequest>,
) -> Result<Json<TransactionDataResponse>, ApiError> {
    Ok(Json(prepare_registration(&state, &body).await?))
}

pub(crate) async fn prepare_registration(
    state: &AppState,
    body: &RegisterAssetRequest,
) -> Result<TransactionDataResponse, ApiError> {
    let asset_id = parse_hex32("assetId", &body.asset_id)?;
    let custodian = parse_hex32("custodian", &body.custodian)?;
    if asset_id == [0; 32] || custodian == [0; 32] || !(1..=8).contains(&body.asset_type) {
        return Err(ApiError::Validation(
            "assetId/custodian must be non-zero and assetType within 1..=8".into(),
        ));
    }
    if state
        .rpc
        .v2_asset_state(state.config.deployment_id, asset_id)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict("asset is already registered".into()));
    }
    let config = state
        .rpc
        .v2_protocol_config(state.config.deployment_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("canonical v2 ProtocolConfig not found".into()))?;
    with_priority_fee(
        build_register_asset(
            &state.config.lastro_program_id,
            &state.config.deployment_id,
            config.authority,
            sponsor::payer_for(state.sponsor(), config.authority),
            asset_id,
            body.asset_type,
            Pubkey::new_from_array(custodian),
            body.available_weight_grams,
        )?,
        state.config.priority_fee_micro_lamports,
    )
}

/// Refresh the projection of one asset from finalized chain state.
pub async fn sync(
    State(state): State<AppState>,
    Path(asset_id): Path<String>,
) -> Result<Json<AssetResponse>, ApiError> {
    let asset = canonical(&state, &asset_id).await?;
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;
    domain_v2::upsert_asset_projection_tx(&mut tx, &asset).await?;
    tx.commit()
        .await
        .map_err(|_| ApiError::Unavailable("postgres commit failed".into()))?;
    Ok(Json(asset_response(&asset)))
}

pub async fn get(
    State(state): State<AppState>,
    Path(asset_id): Path<String>,
) -> Result<Json<AssetResponse>, ApiError> {
    Ok(Json(asset_response(&canonical(&state, &asset_id).await?)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RfidLookupResponse {
    pub rfid_hash: String,
    /// ACTIVE: this tag identifies the asset now. RETIRED: it did in the past.
    pub binding_status: &'static str,
    pub asset: AssetResponse,
}

/// Resolve a (possibly retired) RFID to its asset. Tags are never reused, so a retired tag
/// still leads to the animal it once identified.
pub async fn by_rfid(
    State(state): State<AppState>,
    Path(rfid_hash): Path<String>,
) -> Result<Json<RfidLookupResponse>, ApiError> {
    let rfid = parse_hex32("rfidHash", &rfid_hash)?;
    let binding = state
        .rpc
        .v2_rfid_binding(state.config.deployment_id, rfid)
        .await?
        .ok_or_else(|| ApiError::NotFound("RFID is not bound to any asset".into()))?;
    let asset = state
        .rpc
        .v2_asset_state(state.config.deployment_id, binding.asset_id)
        .await?
        .ok_or(ApiError::Internal)?;
    Ok(Json(RfidLookupResponse {
        rfid_hash: hex::encode(rfid),
        binding_status: match binding.status {
            BindingStatus::Active => "ACTIVE",
            BindingStatus::Retired => "RETIRED",
        },
        asset: asset_response(&asset),
    }))
}

/// Portable bundle for independent verification: the finalized Station events with their
/// signed envelopes and the accepted custody transfers, all re-checkable against Solana.
pub async fn evidence_package(
    State(state): State<AppState>,
    Path(asset_id): Path<String>,
) -> Result<Json<EvidencePackageV2>, ApiError> {
    let asset = canonical(&state, &asset_id).await?;
    let deployment = state.config.deployment_id;
    let events = domain_v2::list_finalized_by_subject(&state.db, deployment, asset.asset_id)
        .await?
        .into_iter()
        .map(event_response)
        .collect();
    let custody_transfers =
        operations::list_accepted_custody_proofs(&state.db, deployment, asset.asset_id)
            .await?
            .into_iter()
            .map(|(transfer_id, wallet, tx_signature)| CustodyProofResponse {
                transfer_id,
                new_custodian: hex::encode(wallet),
                tx_signature,
            })
            .collect();
    let batched = batched_observations(&state, asset.asset_id).await?;
    Ok(Json(EvidencePackageV2 {
        schema: EVIDENCE_PACKAGE_SCHEMA,
        deployment_id: hex::encode(deployment),
        lastro_program_id: state.config.lastro_program_id.clone(),
        asset: asset_response(&asset),
        events,
        custody_transfers,
        facilities: facility_proofs(&state, &batched).await?,
        batched_observations: batched,
    }))
}

/// The registered boundary of every facility the batched observations were made at; a
/// facility without one is listed with no boundary so the verifier expects a zero commitment.
async fn facility_proofs(
    state: &AppState,
    batched: &[BatchedObservationProof],
) -> Result<Vec<FacilityBoundaryProof>, ApiError> {
    let mut facility_ids: Vec<&String> = batched.iter().map(|proof| &proof.facility_id).collect();
    facility_ids.sort();
    facility_ids.dedup();
    let mut proofs = Vec::with_capacity(facility_ids.len());
    for facility_hex in facility_ids {
        let facility_id = parse_hex32("facilityId", facility_hex)?;
        let boundary = facility_boundaries::find(&state.db, facility_id).await?;
        proofs.push(FacilityBoundaryProof {
            facility_id: facility_hex.clone(),
            car_code: boundary.as_ref().map(|b| b.car_code.clone()),
            boundary_base64: boundary.as_ref().map(|b| BASE64.encode(&b.boundary)),
            geo_commitment: hex::encode(boundary.map_or([0; 32], |b| b.geo_commitment)),
        });
    }
    Ok(proofs)
}

async fn session_facility(
    state: &AppState,
    seen: &mut HashMap<[u8; 32], [u8; 32]>,
    session_id: [u8; 32],
) -> Result<[u8; 32], ApiError> {
    if let Some(facility) = seen.get(&session_id) {
        return Ok(*facility);
    }
    let session = sessions::find(&state.db, state.config.deployment_id, session_id)
        .await?
        .ok_or(ApiError::Internal)?;
    seen.insert(session_id, session.facility_id);
    Ok(session.facility_id)
}

/// (session id, batch index).
type BatchKey = ([u8; 32], u32);
/// A batch with its leaf hashes in order.
type LoadedBatch = (sessions::BatchRecord, Vec<[u8; 32]>);

/// Every anchored session presence proof of the asset with its Merkle path to the batch root.
async fn batched_observations(
    state: &AppState,
    asset_id: [u8; 32],
) -> Result<Vec<BatchedObservationProof>, ApiError> {
    let observations = sessions::anchored_for_asset(
        &state.db,
        state.config.deployment_id,
        asset_id,
        u32::try_from(domain_v2::MAX_HISTORY_EVENTS + 1).map_err(|_| ApiError::Internal)?,
    )
    .await?;
    if observations.len() > domain_v2::MAX_HISTORY_EVENTS {
        return Err(ApiError::Conflict(
            "presence history exceeds the package limit".into(),
        ));
    }
    let mut batches: HashMap<BatchKey, LoadedBatch> = HashMap::new();
    let mut sessions_seen: HashMap<[u8; 32], [u8; 32]> = HashMap::new();
    let mut proofs = Vec::with_capacity(observations.len());
    for observation in observations {
        let (batch_index, leaf_index) = observation
            .batch_index
            .zip(observation.leaf_index)
            .ok_or(ApiError::Internal)?;
        let key = (observation.session_id, batch_index);
        if let std::collections::hash_map::Entry::Vacant(slot) = batches.entry(key) {
            let batch = sessions::find_batch(&state.db, observation.session_id, batch_index)
                .await?
                .ok_or(ApiError::Internal)?;
            let leaves = sessions::batch_leaves(&state.db, observation.session_id, batch_index)
                .await?
                .iter()
                .map(|leaf| batch_leaf_hash(&leaf.envelope, &leaf.station_signature))
                .collect();
            slot.insert((batch, leaves));
        }
        let (batch, leaves) = &batches[&key];
        let proof =
            batch_merkle_proof(leaves, leaf_index as usize).map_err(|_| ApiError::Internal)?;
        proofs.push(BatchedObservationProof {
            event_hash: hex::encode(observation.event_hash),
            envelope_bytes_base64: BASE64.encode(observation.envelope),
            observed_rfid_hex: hex::encode(observation.observed_rfid),
            station_pubkey_hex: hex::encode(observation.station_pubkey),
            station_signature_hex: hex::encode(observation.station_signature),
            session_id: hex::encode(observation.session_id),
            facility_id: hex::encode(
                session_facility(state, &mut sessions_seen, observation.session_id).await?,
            ),
            batch_index,
            leaf_index,
            leaf_count: batch.leaf_count,
            merkle_root: hex::encode(batch.merkle_root),
            merkle_proof: proof.iter().map(hex::encode).collect(),
            tx_signature: batch.tx_signature.clone().ok_or(ApiError::Internal)?,
        });
    }
    Ok(proofs)
}

async fn canonical(state: &AppState, asset_id: &str) -> Result<CanonicalV2AssetState, ApiError> {
    let asset_id = parse_hex32("assetId", asset_id)?;
    state
        .rpc
        .v2_asset_state(state.config.deployment_id, asset_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("asset is not registered on Solana".into()))
}

pub(crate) fn asset_response(asset: &CanonicalV2AssetState) -> AssetResponse {
    AssetResponse {
        asset_id: hex::encode(asset.asset_id),
        asset_type: asset.asset_type,
        status: asset.status,
        custodian: hex::encode(asset.custodian.to_bytes()),
        state_version: asset.state_version,
        event_sequence: asset.event_sequence,
        last_event_hash: hex::encode(asset.last_event_hash),
        current_rfid_hash: (asset.current_rfid_hash != [0; 32])
            .then(|| hex::encode(asset.current_rfid_hash)),
        available_weight_grams: asset.available_weight_grams,
    }
}
