//! Public queries for transformation commitments and asset lineage.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use lastro_protocol::v2::{
    LineageLeaf, LineageRole, MassBalance, TransformationManifest, merkle_proof, merkle_root,
};
use solana_pubkey::Pubkey;

use crate::{
    error::ApiError,
    model::{
        LineageEdgeResponse, LineageLeafInput, LineageLeafProofResponse,
        RegisterTransformationRequest, SyncTransformationRequest,
        TransformationRegistrationResponse, TransformationResponse, parse_hex32,
    },
    repository::transformations::{self, LineageEdgeRecord, NewLineageEdge, TransformationRecord},
    routes::{agent::authorize, validate_transaction_signature},
    solana::v2_transaction_builder::{build_finalize_transformation, parse_program_id},
    state::AppState,
};

/// Keeps the request inside the 64 KiB operational body limit and proofs shallow.
const MAX_LEAVES_PER_SIDE: usize = 200;

/// Registers a facility transformation manifest. The API derives both Merkle roots,
/// counts and mass totals from the submitted leaves, so a client cannot commit roots that
/// disagree with the lineage it reports; it returns the proofs the facility wallet needs
/// for `reserve_transformation_input` and `create_transformation_output`.
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RegisterTransformationRequest>,
) -> Result<(StatusCode, Json<TransformationRegistrationResponse>), ApiError> {
    let token =
        state.config.operator_token.as_deref().ok_or_else(|| {
            ApiError::Unavailable("operational API token is not configured".into())
        })?;
    authorize(&headers, token)?;

    let transformation_id = parse_hex32("transformationId", &body.transformation_id)?;
    let facility_id = parse_hex32("facilityId", &body.facility_id)?;
    let inputs = parse_leaves(&body.inputs, &[LineageRole::Input])?;
    let outputs = parse_leaves(
        &body.outputs,
        &[LineageRole::Output, LineageRole::Byproduct],
    )?;
    let sum = |leaves: &[LineageLeaf], role: LineageRole| {
        leaves
            .iter()
            .filter(|leaf| leaf.role == role)
            .try_fold(0u64, |total, leaf| total.checked_add(leaf.weight_grams))
            .ok_or_else(|| ApiError::Validation("lineage weight overflows u64".into()))
    };
    let manifest = TransformationManifest {
        transformation_id,
        facility_id,
        transformation_type: body.transformation_type,
        input_root: merkle_root(&inputs).map_err(invalid)?,
        output_root: merkle_root(&outputs).map_err(invalid)?,
        input_count: u32::try_from(inputs.len()).map_err(|_| ApiError::Internal)?,
        output_count: u32::try_from(outputs.len()).map_err(|_| ApiError::Internal)?,
        mass: MassBalance {
            input_weight_grams: sum(&inputs, LineageRole::Input)?,
            output_weight_grams: sum(&outputs, LineageRole::Output)?,
            byproduct_weight_grams: sum(&outputs, LineageRole::Byproduct)?,
            loss_weight_grams: body.loss_weight_grams,
            tolerance_basis_points: body.tolerance_basis_points,
        },
        manifest_nonce: body.manifest_nonce,
        expires_at: body.expires_at,
    };
    let manifest_bytes = manifest.encode().map_err(invalid)?;
    let manifest_hash = manifest.manifest_hash().map_err(invalid)?;

    let mut edges = Vec::with_capacity(inputs.len() + outputs.len());
    for leaf in &inputs {
        edges.push(edge(leaf, leaf.asset_id, transformation_id));
    }
    for leaf in &outputs {
        edges.push(edge(leaf, transformation_id, leaf.asset_id));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;
    let record = transformations::insert_transformation_tx(
        &mut tx,
        state.config.deployment_id,
        &manifest,
        &manifest_bytes,
        manifest_hash,
        &edges,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|_| ApiError::Unavailable("postgres transaction failed".into()))?;

    Ok((
        StatusCode::CREATED,
        Json(TransformationRegistrationResponse {
            transformation: to_response(record),
            input_proofs: proofs(&inputs)?,
            output_proofs: proofs(&outputs)?,
        }),
    ))
}

/// Mirrors the finalized on-chain TransformationAnchor into the projection. A FINALIZED
/// status is only accepted with the facility owner's finalized `finalize_transformation`.
pub async fn sync(
    State(state): State<AppState>,
    Path(transformation_id): Path<String>,
    Json(body): Json<SyncTransformationRequest>,
) -> Result<Json<TransformationResponse>, ApiError> {
    let transformation_id = parse_hex32("transformationId", &transformation_id)?;
    let deployment_id = state.config.deployment_id;
    let record = transformations::find_transformation(&state.db, deployment_id, transformation_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("v2 transformation not found".into()))?;
    let canonical = state
        .rpc
        .v2_transformation(deployment_id, transformation_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("canonical v2 TransformationAnchor not found".into()))?;
    if canonical.manifest_hash != record.manifest_hash
        || canonical.facility_id != record.facility_id
        || canonical.input_root != record.input_root
        || canonical.output_root != record.output_root
    {
        return Err(ApiError::Conflict(
            "on-chain transformation does not match the registered manifest".into(),
        ));
    }
    let status = match canonical.status {
        1 => "OPEN",
        2 => "FINALIZING",
        3 => "FINALIZED",
        4 => "ABORTED",
        5 => "EXPIRED",
        _ => {
            return Err(ApiError::Conflict(
                "unknown on-chain transformation status".into(),
            ));
        }
    };
    if record.status == status {
        return Ok(Json(to_response(record)));
    }

    let mut signature = None;
    if status == "FINALIZED" {
        let tx_signature = body.tx_signature.as_deref().ok_or_else(|| {
            ApiError::Validation("txSignature of finalize_transformation is required".into())
        })?;
        validate_transaction_signature(tx_signature)?;
        let owner =
            transformations::facility_owner_wallet(&state.db, deployment_id, record.facility_id)
                .await?
                .ok_or_else(|| ApiError::Conflict("facility owner is unknown".into()))?;
        let expected = build_finalize_transformation(
            &parse_program_id(&state.config.lastro_program_id)?,
            &deployment_id,
            &record.facility_id,
            &transformation_id,
            Pubkey::new_from_array(owner),
        );
        if !state
            .rpc
            .transaction_matches(tx_signature, &expected)
            .await?
        {
            return Err(ApiError::Conflict(
                "transaction is not the facility owner's finalized finalize_transformation".into(),
            ));
        }
        signature = Some(tx_signature);
    }
    let updated = transformations::update_status(
        &state.db,
        deployment_id,
        transformation_id,
        status,
        canonical.sequence,
        signature,
    )
    .await?;
    Ok(Json(to_response(updated)))
}

fn parse_leaves(
    values: &[LineageLeafInput],
    allowed: &[LineageRole],
) -> Result<Vec<LineageLeaf>, ApiError> {
    if values.is_empty() || values.len() > MAX_LEAVES_PER_SIDE {
        return Err(ApiError::Validation(format!(
            "each side must contain between 1 and {MAX_LEAVES_PER_SIDE} lineage leaves"
        )));
    }
    let mut leaves = values
        .iter()
        .map(|value| {
            let role = LineageRole::try_from(value.role)
                .ok()
                .filter(|role| allowed.contains(role))
                .ok_or_else(|| {
                    ApiError::Validation("lineage leaf role is not allowed here".into())
                })?;
            Ok(LineageLeaf {
                asset_id: parse_hex32("assetId", &value.asset_id)?,
                role,
                position: value.position,
                quantity: value.quantity,
                weight_grams: value.weight_grams,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    // Proof indices follow position order, exactly like merkle_root.
    leaves.sort_by_key(|leaf| leaf.position);
    Ok(leaves)
}

fn proofs(leaves: &[LineageLeaf]) -> Result<Vec<LineageLeafProofResponse>, ApiError> {
    leaves
        .iter()
        .enumerate()
        .map(|(index, leaf)| {
            Ok(LineageLeafProofResponse {
                asset_id: hex::encode(leaf.asset_id),
                role: leaf.role as u8,
                position: leaf.position,
                quantity: leaf.quantity,
                weight_grams: leaf.weight_grams,
                leaf_index: u32::try_from(index).map_err(|_| ApiError::Internal)?,
                proof: merkle_proof(leaves, index)
                    .map_err(invalid)?
                    .into_iter()
                    .map(hex::encode)
                    .collect(),
            })
        })
        .collect()
}

fn edge(leaf: &LineageLeaf, parent: [u8; 32], child: [u8; 32]) -> NewLineageEdge {
    NewLineageEdge {
        parent_asset_id: parent,
        child_asset_id: child,
        role: leaf.role as u8,
        position: leaf.position,
        quantity: leaf.quantity,
        weight_grams: leaf.weight_grams,
    }
}

fn invalid(error: lastro_protocol::error::ProtocolError) -> ApiError {
    ApiError::Validation(format!("invalid transformation manifest: {error}"))
}

pub async fn get(
    State(state): State<AppState>,
    Path(transformation_id): Path<String>,
) -> Result<Json<TransformationResponse>, ApiError> {
    let transformation_id = parse_hex32("transformationId", &transformation_id)?;
    let record = transformations::find_transformation(
        &state.db,
        state.config.deployment_id,
        transformation_id,
    )
    .await?
    .ok_or_else(|| ApiError::NotFound("v2 transformation not found".into()))?;
    Ok(Json(to_response(record)))
}

pub async fn lineage(
    State(state): State<AppState>,
    Path(asset_id): Path<String>,
) -> Result<Json<Vec<LineageEdgeResponse>>, ApiError> {
    let asset_id = parse_hex32("assetId", &asset_id)?;
    let edges = transformations::list_lineage(&state.db, state.config.deployment_id, asset_id)
        .await?
        .into_iter()
        .map(to_edge_response)
        .collect();
    Ok(Json(edges))
}

fn to_response(record: TransformationRecord) -> TransformationResponse {
    TransformationResponse {
        transformation_id: hex::encode(record.transformation_id),
        deployment_id: hex::encode(record.deployment_id),
        facility_id: hex::encode(record.facility_id),
        transformation_type: record.transformation_type,
        input_root: hex::encode(record.input_root),
        output_root: hex::encode(record.output_root),
        input_count: record.input_count,
        output_count: record.output_count,
        input_weight_grams: record.input_weight_grams,
        output_weight_grams: record.output_weight_grams,
        byproduct_weight_grams: record.byproduct_weight_grams,
        loss_weight_grams: record.loss_weight_grams,
        tolerance_basis_points: record.tolerance_basis_points,
        manifest_nonce: record.manifest_nonce,
        manifest_hash: hex::encode(record.manifest_hash),
        manifest_bytes_base64: transformations::manifest_base64(&record),
        status: record.status,
        sequence: record.sequence,
        expires_at: record.expires_at,
        tx_signature: record.tx_signature,
    }
}

fn to_edge_response(record: LineageEdgeRecord) -> LineageEdgeResponse {
    LineageEdgeResponse {
        transformation_id: hex::encode(record.transformation_id),
        parent_asset_id: hex::encode(record.parent_asset_id),
        child_asset_id: hex::encode(record.child_asset_id),
        role: record.role,
        position: record.position,
        quantity: record.quantity,
        weight_grams: record.weight_grams,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lastro_protocol::v2::verify_merkle_proof;

    fn leaf(id: u8, role: u8, position: u32, weight: u64) -> LineageLeafInput {
        LineageLeafInput {
            asset_id: hex::encode([id; 32]),
            role,
            position,
            quantity: 1,
            weight_grams: weight,
        }
    }

    #[test]
    fn returned_proofs_verify_against_the_derived_roots() {
        // PURPOSE: facility wallets receive proofs the on-chain verifier will accept.
        // ASSERT: each proof verifies at its position-sorted index; wrong roles are rejected.
        // FAILURE MEANS: registered transformations could never reserve inputs on-chain.
        let outputs = parse_leaves(
            &[leaf(3, 3, 9, 40), leaf(1, 2, 2, 500), leaf(2, 2, 5, 300)],
            &[LineageRole::Output, LineageRole::Byproduct],
        )
        .unwrap();
        let root = merkle_root(&outputs).unwrap();
        for item in proofs(&outputs).unwrap() {
            let asset_id: [u8; 32] = hex::decode(&item.asset_id).unwrap().try_into().unwrap();
            let hash = LineageLeaf {
                asset_id,
                role: LineageRole::try_from(item.role).unwrap(),
                position: item.position,
                quantity: item.quantity,
                weight_grams: item.weight_grams,
            }
            .hash()
            .unwrap();
            let path: Vec<[u8; 32]> = item
                .proof
                .iter()
                .map(|node| hex::decode(node).unwrap().try_into().unwrap())
                .collect();
            assert!(verify_merkle_proof(hash, item.leaf_index, &path, root));
        }
        assert!(parse_leaves(&[leaf(1, 2, 0, 10)], &[LineageRole::Input]).is_err());
        assert!(parse_leaves(&[], &[LineageRole::Input]).is_err());
    }
}
