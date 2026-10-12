//! SPEC v2.1 S3/S4a/S8: a handling session admits Station-signed presence proofs offline and
//! anchors them as one Merkle batch the evidence package can prove leaf by leaf.

mod common;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use lastro_api::{
    error::ApiError,
    model::TransactionDataResponse,
    routes,
    solana::rpc::{
        CanonicalV2AssetState, CanonicalV2Batch, CanonicalV2ProtocolConfig, CanonicalV2Session,
        CanonicalV2Station, SESSION_STATUS_OPEN, SolanaRpc,
    },
};
use lastro_protocol::{
    crypto::derive_station_id,
    rfid::hash_canonical_rfid,
    v2::{
        SessionAssetContext, batch_leaf_hash, batch_merkle_root, capture_envelope,
        verify_batch_proof,
    },
};
use serde_json::{Value, json};
use solana_pubkey::Pubkey;
use tower::ServiceExt;

use common::{
    AGENT_TOKEN, AUTHORITY, DEPLOYMENT_ID, STATION_PUBKEY, TestDb, app_state, station_sign,
};

const CUSTODIAN: [u8; 32] = [0xc5; 32];
const ASSET_A: [u8; 32] = [0xa1; 32];
const ASSET_B: [u8; 32] = [0xa2; 32];
const TAG_A: [u8; 8] = [0x80, 0, 0x13, 0, 0, 0, 0, 0x0a];
const TAG_B: [u8; 8] = [0x80, 0, 0x13, 0, 0, 0, 0, 0x0b];
const HEAD_A: [u8; 32] = [0xe1; 32];
const HEAD_B: [u8; 32] = [0xe2; 32];
const FARM: [u8; 32] = [0xfa; 32];

#[derive(Default)]
struct SessionRpc {
    session: Mutex<Option<CanonicalV2Session>>,
    batches: Mutex<HashMap<u32, CanonicalV2Batch>>,
}

#[async_trait]
impl SolanaRpc for SessionRpc {
    async fn v2_protocol_config(
        &self,
        _: [u8; 32],
    ) -> Result<Option<CanonicalV2ProtocolConfig>, ApiError> {
        Ok(Some(CanonicalV2ProtocolConfig {
            authority: Pubkey::new_from_array(AUTHORITY),
            station_registry: Pubkey::new_from_array([0xbb; 32]),
            max_event_age_seconds: 86_400,
        }))
    }

    async fn v2_asset_state(
        &self,
        _: [u8; 32],
        asset_id: [u8; 32],
    ) -> Result<Option<CanonicalV2AssetState>, ApiError> {
        let mut asset = common::untagged_asset(asset_id, CUSTODIAN);
        asset.state_version = 1;
        asset.event_sequence = 1;
        asset.last_event_hash = HEAD_A;
        asset.current_rfid_hash = hash_canonical_rfid(&TAG_A);
        Ok(Some(asset))
    }

    async fn v2_station(
        &self,
        _: [u8; 32],
        station_id: [u8; 32],
    ) -> Result<Option<CanonicalV2Station>, ApiError> {
        Ok(Some(CanonicalV2Station {
            station_id,
            pubkey33: STATION_PUBKEY,
            status: 1,
            valid_from: 0,
            valid_until: i64::MAX,
            facility_id: FARM,
        }))
    }

    async fn v2_session(
        &self,
        _: [u8; 32],
        _: [u8; 32],
    ) -> Result<Option<CanonicalV2Session>, ApiError> {
        Ok(self.session.lock().unwrap().clone())
    }

    async fn v2_batch(
        &self,
        _: [u8; 32],
        _: [u8; 32],
        batch_index: u32,
    ) -> Result<Option<CanonicalV2Batch>, ApiError> {
        Ok(self.batches.lock().unwrap().get(&batch_index).cloned())
    }

    async fn transaction_matches_confirmed(
        &self,
        _: &str,
        _: &TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        Ok(true)
    }

    async fn transaction_matches(
        &self,
        _: &str,
        _: &TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        Ok(true)
    }
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    bearer: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = match body {
        Some(value) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(value.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

const OPERATOR: &str = "operator-token-0123456789abcdef0123";

/// The test app with an operator token, and the custodian registered as an ACTIVE party (the
/// operator may only prepare sessions for a custodian the platform knows).
async fn session_app(db: &TestDb, rpc: Arc<SessionRpc>) -> Router {
    sqlx::query("INSERT INTO v2_parties(party_id,deployment_id,legal_name,wallet,role,status) VALUES($1,$2,'Fazenda Boa Vista',$3,1,'ACTIVE')")
        .bind([0x0au8; 32].to_vec())
        .bind(DEPLOYMENT_ID.to_vec())
        .bind(CUSTODIAN.to_vec())
        .execute(&db.pool)
        .await
        .unwrap();
    let mut state = app_state(db.pool.clone(), rpc);
    let mut config = (*state.config).clone();
    config.operator_token = Some(OPERATOR.into());
    state.config = Arc::new(config);
    routes::router(state)
}

async fn seed_asset(db: &TestDb, asset_id: [u8; 32], tag: [u8; 8], head: [u8; 32]) {
    sqlx::query(
        "INSERT INTO v2_assets(asset_id,deployment_id,asset_type,status,custodian,parent_root,lineage_root,current_lot_id,available_weight_grams,event_sequence,state_version,last_event_hash,current_rfid_hash) VALUES($1,$2,1,1,$3,$4,$1,$4,0,1,1,$5,$6)",
    )
    .bind(asset_id.to_vec())
    .bind(DEPLOYMENT_ID.to_vec())
    .bind(CUSTODIAN.to_vec())
    .bind([0u8; 32].to_vec())
    .bind(head.to_vec())
    .bind(hash_canonical_rfid(&tag).to_vec())
    .execute(&db.pool)
    .await
    .unwrap();
}

/// The Station-signed presence proof the Agent would deliver for `asset`.
fn observation(
    session_id: [u8; 32],
    asset: [u8; 32],
    tag: [u8; 8],
    head: [u8; 32],
    observed_at: i64,
) -> Value {
    let context = SessionAssetContext {
        deployment_id: DEPLOYMENT_ID,
        session_id,
        asset_id: asset,
        state_version: 1,
        previous_event_hash: head,
        rfid_hash: hash_canonical_rfid(&tag),
    };
    let command = context.command(observed_at, observed_at + 3_600).unwrap();
    let envelope = capture_envelope(&command, &tag, derive_station_id(&STATION_PUBKEY).unwrap())
        .unwrap()
        .encode()
        .unwrap();
    json!({
        "envelopeBase64": BASE64.encode(envelope),
        "observedRfidHex": hex::encode(tag),
        "stationPubkeyHex": hex::encode(STATION_PUBKEY),
        "stationSignatureHex": hex::encode(station_sign(&envelope)),
    })
}

fn event_hash_of(proof: &Value) -> [u8; 32] {
    let envelope = BASE64
        .decode(proof["envelopeBase64"].as_str().unwrap())
        .unwrap();
    lastro_protocol::v2::DomainEventEnvelope::decode(&envelope)
        .unwrap()
        .event_hash()
        .unwrap()
}

fn hex32(value: &Value) -> [u8; 32] {
    hex::decode(value.as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap()
}

#[tokio::test]
async fn a_session_admits_offline_proofs_and_anchors_them_as_one_provable_batch() {
    // PURPOSE: S3/S4a/S8 end to end in the API — one opening, offline delivery, one batch,
    // and a package whose every leaf proves against the on-chain root.
    // ARRANGE: two tagged assets held by the custodian, a mock chain that echoes the terms.
    let db = TestDb::new().await;
    seed_asset(&db, ASSET_A, TAG_A, HEAD_A).await;
    seed_asset(&db, ASSET_B, TAG_B, HEAD_B).await;
    let rpc = Arc::new(SessionRpc::default());
    let app = session_app(&db, rpc.clone()).await;

    // ACTION 1: prepare and confirm the opening (self-paid: no sponsor configured). Preparing
    // needs the operator (or the custodian's organization); anonymous callers are refused.
    let request =
        json!({"custodian": hex::encode(CUSTODIAN), "maxEvents": 10, "durationSeconds": 3600});
    let (status, _) = call(
        &app,
        "POST",
        "/api/v2/sessions",
        Some(request.clone()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, prepared) = call(
        &app,
        "POST",
        "/api/v2/sessions",
        Some(request),
        Some(OPERATOR),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{prepared}");
    let session = &prepared["session"];
    let session_id = hex32(&session["sessionId"]);
    let valid_from = session["validFrom"].as_i64().unwrap();
    let expires_at = session["expiresAt"].as_i64().unwrap();
    assert_eq!(
        prepared["transaction"]["requiredSigner"],
        Pubkey::new_from_array(CUSTODIAN).to_string()
    );
    let canonical = |events_used: u32, batch_count: u32| CanonicalV2Session {
        session_id,
        custodian: Pubkey::new_from_array(CUSTODIAN),
        station_id: derive_station_id(&STATION_PUBKEY).unwrap(),
        allowed_actions: 1,
        max_events: 10,
        events_used,
        batch_count,
        valid_from,
        expires_at,
        revoked_at: 0,
        status: SESSION_STATUS_OPEN,
        rent_payer: Pubkey::new_from_array(CUSTODIAN),
        facility_id: FARM,
    };
    *rpc.session.lock().unwrap() = Some(canonical(0, 0));
    let base = format!("/api/v2/sessions/{}", hex::encode(session_id));
    let signature = bs58::encode([7u8; 64]).into_string();
    let (status, opened) = call(
        &app,
        "POST",
        &format!("{base}/confirm"),
        Some(json!({"txSignature": signature})),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    assert_eq!(opened["status"], "OPEN");

    // ACTION 2: the Agent reads the frozen manifest (agent token only).
    let (status, _) = call(&app, "GET", &format!("{base}/manifest"), None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, manifest) = call(
        &app,
        "GET",
        &format!("{base}/manifest"),
        None,
        Some(AGENT_TOKEN),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(manifest["assets"].as_array().unwrap().len(), 2);

    // ACTION 3: deliver both proofs; a repeat is idempotent; forgeries are refused.
    let proof_a = observation(session_id, ASSET_A, TAG_A, HEAD_A, valid_from);
    let observations = format!("{base}/observations");
    let (status, admitted) = call(
        &app,
        "POST",
        &observations,
        Some(proof_a.clone()),
        Some(AGENT_TOKEN),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{admitted}");
    let (status, _) = call(
        &app,
        "POST",
        &observations,
        Some(proof_a.clone()),
        Some(AGENT_TOKEN),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let mut forged = proof_a.clone();
    forged["stationSignatureHex"] = json!(hex::encode([0x11u8; 64]));
    let (status, _) = call(&app, "POST", &observations, Some(forged), Some(AGENT_TOKEN)).await;
    assert!(status.is_client_error());
    let early = observation(session_id, ASSET_B, TAG_B, HEAD_B, valid_from - 1);
    let (status, _) = call(&app, "POST", &observations, Some(early), Some(AGENT_TOKEN)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let stale_head = observation(session_id, ASSET_B, TAG_B, HEAD_A, valid_from);
    let (status, _) = call(
        &app,
        "POST",
        &observations,
        Some(stale_head),
        Some(AGENT_TOKEN),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let wrong_tag = observation(session_id, ASSET_B, TAG_A, HEAD_B, valid_from);
    let (status, _) = call(
        &app,
        "POST",
        &observations,
        Some(wrong_tag),
        Some(AGENT_TOKEN),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let proof_b = observation(session_id, ASSET_B, TAG_B, HEAD_B, valid_from);
    let (status, _) = call(
        &app,
        "POST",
        &observations,
        Some(proof_b.clone()),
        Some(AGENT_TOKEN),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    // ACTION 4: one batch with both leaves; preparing again returns the same batch.
    let (status, _) = call(&app, "POST", &format!("{base}/batches"), None, None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "anonymous callers cannot freeze a batch"
    );
    let (status, batch) = call(
        &app,
        "POST",
        &format!("{base}/batches"),
        None,
        Some(AGENT_TOKEN),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{batch}");
    assert_eq!(batch["batch"]["leafCount"], 2);
    let (_, again) = call(
        &app,
        "POST",
        &format!("{base}/batches"),
        None,
        Some(AGENT_TOKEN),
    )
    .await;
    assert_eq!(again["batch"]["merkleRoot"], batch["batch"]["merkleRoot"]);
    let root = hex32(&batch["batch"]["merkleRoot"]);
    let leaf = |proof: &Value| {
        let envelope: [u8; 220] = BASE64
            .decode(proof["envelopeBase64"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        let signature: [u8; 64] = hex::decode(proof["stationSignatureHex"].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap();
        batch_leaf_hash(&envelope, &signature)
    };
    // Same observed_at: leaves are ordered by event hash.
    let mut ordered = [
        (hex32(&admitted["eventHash"]), leaf(&proof_a)),
        (event_hash_of(&proof_b), leaf(&proof_b)),
    ];
    ordered.sort();
    assert_eq!(
        batch_merkle_root(&[ordered[0].1, ordered[1].1]).unwrap(),
        root
    );

    // ACTION 5: the chain admits the batch; confirmation checks the BatchAnchor terms.
    rpc.batches.lock().unwrap().insert(
        0,
        CanonicalV2Batch {
            session_id,
            batch_index: 0,
            custodian: Pubkey::new_from_array(CUSTODIAN),
            station_id: derive_station_id(&STATION_PUBKEY).unwrap(),
            merkle_root: root,
            leaf_count: 2,
            first_observed_at: valid_from,
            last_observed_at: valid_from,
            anchored_at: valid_from + 60,
            facility_id: FARM,
        },
    );
    let batch_signature = bs58::encode([8u8; 64]).into_string();
    let (status, anchored) = call(
        &app,
        "POST",
        &format!("{base}/batches/0/confirm"),
        Some(json!({"txSignature": batch_signature})),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{anchored}");
    assert_eq!(anchored["status"], "ANCHORED");
    *rpc.session.lock().unwrap() = Some(canonical(2, 1));
    let (status, _) = call(
        &app,
        "POST",
        &format!("{base}/batches"),
        None,
        Some(AGENT_TOKEN),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "nothing left to batch");

    // ASSERT: the evidence package proves asset A's leaf against the anchored root.
    let (status, package) = call(
        &app,
        "GET",
        &format!("/api/v2/assets/{}/evidence-package", hex::encode(ASSET_A)),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{package}");
    assert_eq!(prepared["session"]["facilityId"], hex::encode(FARM));
    let facilities = package["facilities"].as_array().unwrap();
    assert_eq!(facilities.len(), 1);
    assert_eq!(facilities[0]["facilityId"], hex::encode(FARM));
    assert!(
        facilities[0]["boundaryBase64"].is_null(),
        "no boundary registered"
    );
    let proofs = package["batchedObservations"].as_array().unwrap();
    assert_eq!(proofs[0]["facilityId"], hex::encode(FARM));
    assert_eq!(proofs.len(), 1);
    let entry = &proofs[0];
    assert_eq!(entry["txSignature"], batch_signature);
    let path: Vec<[u8; 32]> = entry["merkleProof"]
        .as_array()
        .unwrap()
        .iter()
        .map(hex32)
        .collect();
    let envelope: [u8; 220] = BASE64
        .decode(entry["envelopeBytesBase64"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let signature: [u8; 64] = hex::decode(entry["stationSignatureHex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    assert!(verify_batch_proof(
        batch_leaf_hash(&envelope, &signature),
        u32::try_from(entry["leafIndex"].as_u64().unwrap()).unwrap(),
        2,
        &path,
        root,
    ));
    db.cleanup().await;
    // FAILURE MEANS: offline presence proofs could not be batched, or a package could not prove them.
}
