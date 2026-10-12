//! SPEC v2.1 S1: the platform sponsor pays fee and rent only for transactions the API
//! prepared and the authorizing wallet signed, within its budgets.

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
use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use lastro_api::{
    config::AppConfig,
    error::ApiError,
    model::TransactionDataResponse,
    routes,
    solana::rpc::{CanonicalV2AssetState, CanonicalV2ProtocolConfig, SolanaRpc},
    sponsor::{Sponsor, SponsorLimits},
    state::AppState,
};
use serde_json::{Value, json};
use solana_pubkey::Pubkey;
use tower::ServiceExt;

use common::{AGENT_TOKEN, DEPLOYMENT_ID, PROGRAM_ID, STATION_PUBKEY, TestDb};

const SPONSOR_SEED: u8 = 0x51;
const AUTHORITY_SEED: u8 = 0x52;

fn signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn pubkey(seed: u8) -> Pubkey {
    Pubkey::new_from_array(signing_key(seed).verifying_key().to_bytes())
}

fn sponsor(limits: SponsorLimits) -> Sponsor {
    let mut bytes = [0u8; 64];
    bytes[..32].copy_from_slice(&[SPONSOR_SEED; 32]);
    bytes[32..].copy_from_slice(signing_key(SPONSOR_SEED).verifying_key().as_bytes());
    Sponsor::from_keypair_bytes(&bytes, limits).unwrap()
}

/// Canonical state with a real Ed25519 deployment authority and no registered assets; records
/// every broadcast.
struct SponsorRpc {
    authority: Pubkey,
    sent: Mutex<Vec<Vec<u8>>>,
}

#[async_trait]
impl SolanaRpc for SponsorRpc {
    async fn v2_protocol_config(
        &self,
        _deployment_id: [u8; 32],
    ) -> Result<Option<CanonicalV2ProtocolConfig>, ApiError> {
        Ok(Some(CanonicalV2ProtocolConfig {
            authority: self.authority,
            station_registry: Pubkey::new_from_array([0xbb; 32]),
            max_event_age_seconds: 86_400,
        }))
    }

    async fn v2_asset_state(
        &self,
        _deployment_id: [u8; 32],
        _asset_id: [u8; 32],
    ) -> Result<Option<CanonicalV2AssetState>, ApiError> {
        Ok(None)
    }

    async fn send_transaction(&self, wire: &[u8]) -> Result<String, ApiError> {
        self.sent.lock().unwrap().push(wire.to_vec());
        Ok(bs58::encode(&wire[1..65]).into_string())
    }

    async fn transaction_matches_confirmed(
        &self,
        _signature: &str,
        _expected: &TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        Ok(false)
    }

    async fn transaction_matches(
        &self,
        _signature: &str,
        _expected: &TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        Ok(false)
    }
}

fn app(db: &TestDb, rpc: Arc<SponsorRpc>, limits: SponsorLimits) -> Router {
    routes::router(AppState {
        db: db.pool.clone(),
        config: Arc::new(AppConfig {
            bind_addr: "127.0.0.1:8080".parse().unwrap(),
            database_url: "postgres://test-only".into(),
            agent_token: AGENT_TOKEN.into(),
            operator_token: None,
            solana_rpc_url: "http://127.0.0.1:8899".into(),
            deployment_id: DEPLOYMENT_ID,
            station_pubkey33: STATION_PUBKEY,
            lastro_program_id: PROGRAM_ID.into(),
            cors_allowed_origins: Vec::new(),
            priority_fee_micro_lamports: None,
            sponsor_keypair_path: None,
            sponsor_limits: limits,
            org_key_encryption_key: None,
            otp_log_delivery: false,
        }),
        rpc,
        sponsor: Some(Arc::new(sponsor(limits))),
        vault: None,
    })
}

async fn post(app: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn registration(asset: u8) -> Value {
    json!({
        "assetId": hex::encode([asset; 32]),
        "custodian": hex::encode([0x77; 32]),
        "assetType": 1,
        "availableWeightGrams": 400_000,
    })
}

fn purpose(asset: u8) -> Value {
    let mut value = registration(asset);
    value["kind"] = json!("REGISTER_ASSET");
    value
}

/// Compiles a prepared descriptor into a legacy wire transaction exactly as a wallet does
/// (fee payer first; writable signers, read-only signers, writable, read-only), signs it with
/// `signers` and leaves every other signature slot (the sponsor's) empty.
fn compile_and_sign(prepared: &Value, signers: &[&SigningKey]) -> Vec<u8> {
    let fee_payer = prepared["feePayer"].as_str().unwrap().to_owned();
    let mut roles: Vec<(String, bool, bool)> = vec![(fee_payer.clone(), true, true)];
    let mut upsert = |address: &str, signer: bool, writable: bool| {
        if let Some(role) = roles.iter_mut().find(|role| role.0 == address) {
            role.1 |= signer;
            role.2 |= writable;
        } else {
            roles.push((address.to_owned(), signer, writable));
        }
    };
    let instructions = prepared["instructions"].as_array().unwrap();
    for instruction in instructions {
        for account in instruction["accounts"].as_array().unwrap() {
            upsert(
                account["address"].as_str().unwrap(),
                account["isSigner"].as_bool().unwrap(),
                account["isWritable"].as_bool().unwrap(),
            );
        }
        upsert(instruction["programId"].as_str().unwrap(), false, false);
    }
    let payer = roles.remove(0);
    let class = |role: &(String, bool, bool)| match (role.1, role.2) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    };
    roles.sort_by_key(class);
    roles.insert(0, payer);
    let keys: Vec<String> = roles.iter().map(|role| role.0.clone()).collect();
    let index: HashMap<&str, u8> = keys
        .iter()
        .enumerate()
        .map(|(at, key)| (key.as_str(), u8::try_from(at).unwrap()))
        .collect();
    let required = roles.iter().filter(|role| role.1).count();
    let readonly_signed = roles.iter().filter(|role| class(role) == 1).count();
    let readonly_unsigned = roles.iter().filter(|role| class(role) == 3).count();

    let mut message = vec![
        u8::try_from(required).unwrap(),
        u8::try_from(readonly_signed).unwrap(),
        u8::try_from(readonly_unsigned).unwrap(),
        u8::try_from(keys.len()).unwrap(),
    ];
    for key in &keys {
        message.extend_from_slice(bs58::decode(key).into_vec().unwrap().as_slice());
    }
    message.extend_from_slice(&[0x42; 32]);
    message.push(u8::try_from(instructions.len()).unwrap());
    for instruction in instructions {
        message.push(index[instruction["programId"].as_str().unwrap()]);
        let accounts = instruction["accounts"].as_array().unwrap();
        message.push(u8::try_from(accounts.len()).unwrap());
        for account in accounts {
            message.push(index[account["address"].as_str().unwrap()]);
        }
        let data = BASE64
            .decode(instruction["dataBase64"].as_str().unwrap())
            .unwrap();
        push_shortvec(&mut message, data.len());
        message.extend_from_slice(&data);
    }
    let mut wire = vec![u8::try_from(required).unwrap()];
    for key in keys.iter().take(required) {
        let signer = signers
            .iter()
            .find(|signer| bs58::encode(signer.verifying_key().as_bytes()).into_string() == *key);
        let signature = signer.map_or([0u8; 64], |signer| signer.sign(&message).to_bytes());
        wire.extend_from_slice(&signature);
    }
    wire.extend_from_slice(&message);
    wire
}

fn push_shortvec(out: &mut Vec<u8>, mut value: usize) {
    loop {
        let byte = u8::try_from(value & 0x7f).unwrap();
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

#[tokio::test]
async fn the_sponsor_pays_only_for_prepared_signed_transactions_within_budget() {
    // PURPOSE: S1 — a zero-balance authority registers an asset while the sponsor pays, and
    // every policy limit holds.
    // ARRANGE: sponsor enabled; per-signer budget fits exactly two registrations.
    let db = TestDb::new().await;
    let rpc = Arc::new(SponsorRpc {
        authority: pubkey(AUTHORITY_SEED),
        sent: Mutex::new(Vec::new()),
    });
    let one_registration: u64 = 2 * 5_000 + (341 + 128) * 6_960;
    let limits = SponsorLimits {
        daily_lamports_per_signer: 2 * one_registration,
        ..SponsorLimits::default()
    };
    let app = app(&db, rpc.clone(), limits);
    let authority = signing_key(AUTHORITY_SEED);

    // ACTION 1: the prepared registration names the sponsor as fee payer.
    let (status, prepared) = post(&app, "/api/v2/assets/transaction-data", registration(1)).await;
    assert_eq!(status, StatusCode::OK, "{prepared}");
    assert_eq!(prepared["feePayer"], pubkey(SPONSOR_SEED).to_string());
    assert_eq!(
        prepared["requiredSigner"],
        pubkey(AUTHORITY_SEED).to_string()
    );
    let wire = compile_and_sign(&prepared, &[&authority]);
    assert_eq!(
        wire.len(),
        usize::try_from(prepared["measuredSerializedBytes"].as_u64().unwrap()).unwrap()
    );

    // ACTION 2: submit; the sponsor co-signs and broadcasts.
    let body = json!({"purpose": purpose(1), "transactionBase64": BASE64.encode(&wire)});
    let (status, sponsored) = post(&app, "/api/v2/sponsor/submit", body.clone()).await;
    assert_eq!(status, StatusCode::OK, "{sponsored}");
    assert_eq!(sponsored["reservedLamports"], one_registration);
    let broadcast = rpc.sent.lock().unwrap()[0].clone();
    let message = &broadcast[1 + 128..];
    VerifyingKey::from_bytes(&pubkey(SPONSOR_SEED).to_bytes())
        .unwrap()
        .verify(
            message,
            &Signature::from_bytes(&broadcast[1..65].try_into().unwrap()),
        )
        .unwrap();
    assert_eq!(
        &broadcast[65..129],
        &wire[65..129],
        "user signature untouched"
    );
    assert_eq!(
        sponsored["txSignature"],
        bs58::encode(&broadcast[1..65]).into_string()
    );

    // ASSERT: the same transaction cannot be sponsored twice.
    let (status, _) = post(&app, "/api/v2/sponsor/submit", body).await;
    assert_eq!(status, StatusCode::CONFLICT);

    // ASSERT: a transaction that differs from the prepared one (another custodian) is refused.
    let mut forged = prepared.clone();
    let lastro = forged["instructions"][0]["dataBase64"].as_str().unwrap();
    let mut data = BASE64.decode(lastro).unwrap();
    data[8 + 32 + 1] ^= 0xff;
    forged["instructions"][0]["dataBase64"] = json!(BASE64.encode(data));
    let forged_wire = compile_and_sign(&forged, &[&authority]);
    let (status, _) = post(
        &app,
        "/api/v2/sponsor/submit",
        json!({"purpose": purpose(1), "transactionBase64": BASE64.encode(forged_wire)}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // ASSERT: a transaction the authority did not sign is refused.
    let (_, prepared_two) = post(&app, "/api/v2/assets/transaction-data", registration(2)).await;
    let unsigned = compile_and_sign(&prepared_two, &[]);
    let (status, _) = post(
        &app,
        "/api/v2/sponsor/submit",
        json!({"purpose": purpose(2), "transactionBase64": BASE64.encode(unsigned)}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // ASSERT: the second registration fits the budget, the third exhausts it.
    let signed_two = compile_and_sign(&prepared_two, &[&authority]);
    let (status, _) = post(
        &app,
        "/api/v2/sponsor/submit",
        json!({"purpose": purpose(2), "transactionBase64": BASE64.encode(signed_two)}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, prepared_three) = post(&app, "/api/v2/assets/transaction-data", registration(3)).await;
    let signed_three = compile_and_sign(&prepared_three, &[&authority]);
    let (status, refused) = post(
        &app,
        "/api/v2/sponsor/submit",
        json!({"purpose": purpose(3), "transactionBase64": BASE64.encode(signed_three)}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{refused}");
    assert_eq!(rpc.sent.lock().unwrap().len(), 2);
    let (spent,): (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(lamports),0)::BIGINT FROM v2_sponsor_ledger")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(u64::try_from(spent).unwrap(), 2 * one_registration);
    db.cleanup().await;
    // FAILURE MEANS: the platform could pay for arbitrary, unsigned or unbounded transactions.
}

#[tokio::test]
async fn sponsorship_is_refused_when_the_sponsor_is_the_deployment_authority() {
    // PURPOSE: I7 — the paying key must never also hold protocol authority.
    // ASSERT: preparation falls back to self-paid and submission is refused, nothing broadcast.
    // FAILURE MEANS: a misconfigured sponsor would co-sign as the deployment authority.
    let db = TestDb::new().await;
    let rpc = Arc::new(SponsorRpc {
        authority: pubkey(SPONSOR_SEED),
        sent: Mutex::new(Vec::new()),
    });
    let app = app(&db, rpc.clone(), SponsorLimits::default());
    let (status, prepared) = post(&app, "/api/v2/assets/transaction-data", registration(1)).await;
    assert_eq!(status, StatusCode::OK);
    // The authority is the sponsor, so nothing is sponsored: it pays for itself.
    assert_eq!(prepared["feePayer"], prepared["requiredSigner"]);
    let wire = compile_and_sign(&prepared, &[&signing_key(SPONSOR_SEED)]);
    let (status, _) = post(
        &app,
        "/api/v2/sponsor/submit",
        json!({"purpose": purpose(1), "transactionBase64": BASE64.encode(wire)}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(rpc.sent.lock().unwrap().is_empty());
    db.cleanup().await;
}
