//! SPEC v2.1 S2: an organization signs in without a browser wallet; its custodian key stays
//! sealed and signs only the transactions its people's roles allow.

mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use ed25519_dalek::{Signature, SigningKey, Verifier as _, VerifyingKey};
use lastro_api::{
    config::AppConfig,
    error::ApiError,
    model::TransactionDataResponse,
    orgs::LocalKeyVault,
    repository, routes,
    solana::rpc::{
        CanonicalV2AssetState, CanonicalV2ProtocolConfig, CanonicalV2Station, SolanaRpc,
    },
    sponsor::{LegacyTransaction, Sponsor, SponsorLimits},
    state::AppState,
};
use serde_json::{Value, json};
use solana_pubkey::Pubkey;
use tower::ServiceExt;
use uuid::Uuid;

use common::{AGENT_TOKEN, DEPLOYMENT_ID, PROGRAM_ID, STATION_PUBKEY, TestDb};

const OPERATOR: &str = "operator-token-0123456789abcdef0123";
const MASTER_KEY: [u8; 32] = [
    0x3c, 0x91, 0x0e, 0x5a, 0xb7, 0x22, 0x68, 0xf4, 0x19, 0xd3, 0x40, 0x8e, 0x7b, 0x05, 0xc6, 0x2f,
    0x93, 0x5d, 0xe1, 0x0a, 0x76, 0xb8, 0x34, 0xcf, 0x61, 0x1e, 0xa9, 0x57, 0x0d, 0xf2, 0x84, 0x3b,
];
const SPONSOR_SEED: u8 = 0x51;
const AUTHORITY_SEED: u8 = 0x52;
const FARM: [u8; 32] = [0xfa; 32];

fn pubkey(seed: u8) -> Pubkey {
    Pubkey::new_from_array(
        SigningKey::from_bytes(&[seed; 32])
            .verifying_key()
            .to_bytes(),
    )
}

/// A registered Station installed at FARM; records every broadcast.
#[derive(Default)]
struct OrgRpc {
    sent: Mutex<Vec<Vec<u8>>>,
}

#[async_trait]
impl SolanaRpc for OrgRpc {
    async fn v2_protocol_config(
        &self,
        _: [u8; 32],
    ) -> Result<Option<CanonicalV2ProtocolConfig>, ApiError> {
        Ok(Some(CanonicalV2ProtocolConfig {
            authority: pubkey(AUTHORITY_SEED),
            station_registry: Pubkey::new_from_array([0xbb; 32]),
            max_event_age_seconds: 86_400,
        }))
    }

    async fn v2_asset_state(
        &self,
        _: [u8; 32],
        _: [u8; 32],
    ) -> Result<Option<CanonicalV2AssetState>, ApiError> {
        Ok(None)
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

    async fn latest_blockhash(&self) -> Result<[u8; 32], ApiError> {
        Ok([0x42; 32])
    }

    async fn send_transaction(&self, wire: &[u8]) -> Result<String, ApiError> {
        self.sent.lock().unwrap().push(wire.to_vec());
        Ok(bs58::encode(&wire[1..65]).into_string())
    }

    async fn transaction_matches_confirmed(
        &self,
        _: &str,
        _: &TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        Ok(false)
    }

    async fn transaction_matches(
        &self,
        _: &str,
        _: &TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        Ok(false)
    }
}

fn app(db: &TestDb, rpc: Arc<OrgRpc>, sponsored: bool) -> Router {
    let limits = SponsorLimits::default();
    let sponsor = sponsored.then(|| {
        let mut bytes = [0u8; 64];
        bytes[..32].copy_from_slice(&[SPONSOR_SEED; 32]);
        bytes[32..].copy_from_slice(pubkey(SPONSOR_SEED).as_ref());
        Arc::new(Sponsor::from_keypair_bytes(&bytes, limits).unwrap())
    });
    routes::router(AppState {
        db: db.pool.clone(),
        config: Arc::new(AppConfig {
            bind_addr: "127.0.0.1:8080".parse().unwrap(),
            database_url: "postgres://test-only".into(),
            agent_token: AGENT_TOKEN.into(),
            operator_token: Some(OPERATOR.into()),
            solana_rpc_url: "http://127.0.0.1:8899".into(),
            deployment_id: DEPLOYMENT_ID,
            station_pubkey33: STATION_PUBKEY,
            lastro_program_id: PROGRAM_ID.into(),
            cors_allowed_origins: Vec::new(),
            priority_fee_micro_lamports: None,
            sponsor_keypair_path: None,
            sponsor_limits: limits,
            org_key_encryption_key: Some(MASTER_KEY),
            otp_log_delivery: true,
        }),
        rpc,
        sponsor,
        vault: Some(Arc::new(LocalKeyVault::new(&MASTER_KEY))),
    })
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
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// Stands in for the e-mail the person would receive: stores a known code for them.
async fn deliver_code(db: &TestDb, user_id: Uuid, code: &str) {
    let vault = LocalKeyVault::new(&MASTER_KEY);
    assert!(
        repository::orgs::issue_code(&db.pool, user_id, vault.code_hash(user_id, code))
            .await
            .unwrap()
    );
}

async fn sign_in(app: &Router, db: &TestDb, email: &str, user_id: Uuid) -> String {
    let (status, _) = call(
        app,
        "POST",
        "/api/v2/auth/otp/request",
        Some(json!({ "email": email })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    deliver_code(db, user_id, "246810").await;
    let (status, session) = call(
        app,
        "POST",
        "/api/v2/auth/otp/verify",
        Some(json!({ "email": email, "code": "246810" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    session["token"].as_str().unwrap().to_owned()
}

fn verify(signer: &Pubkey, message: &[u8], signature: &[u8; 64]) -> bool {
    VerifyingKey::from_bytes(&signer.to_bytes())
        .unwrap()
        .verify(message, &Signature::from_bytes(signature))
        .is_ok()
}

#[tokio::test]
async fn an_organization_runs_a_session_without_a_wallet_and_roles_bound_its_key() {
    // PURPOSE: S2 — a corral operator signs in with an e-mailed code and opens a handling
    // session that the organization wallet signs and the sponsor pays, while roles keep every
    // other purpose and the key itself out of reach.
    // ARRANGE: sponsor and vault enabled; an organization with its administrator.
    let db = TestDb::new().await;
    let rpc = Arc::new(OrgRpc::default());
    let app = app(&db, rpc.clone(), true);
    let (status, _) = call(
        &app,
        "POST",
        "/api/v2/organizations",
        Some(json!({ "name": "Fazenda Boa Vista", "adminEmail": "Dona@BoaVista.com.br" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "operator token required");
    let (status, org) = call(
        &app,
        "POST",
        "/api/v2/organizations",
        Some(json!({ "name": "Fazenda Boa Vista", "adminEmail": "Dona@BoaVista.com.br" })),
        Some(OPERATOR),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{org}");
    let org_id: Uuid = org["orgId"].as_str().unwrap().parse().unwrap();
    let wallet: Pubkey = org["wallet"].as_str().unwrap().parse().unwrap();
    let admin_id: Uuid = org["adminUserId"].as_str().unwrap().parse().unwrap();

    // ACTION: the administrator signs in (e-mail normalized) and adds a corral operator.
    let admin = sign_in(&app, &db, "dona@boavista.com.br", admin_id).await;
    let (status, me) = call(&app, "GET", "/api/v2/me", None, Some(&admin)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["role"], "ORG_ADMIN");
    assert_eq!(me["wallet"], wallet.to_string());
    let users = format!("/api/v2/organizations/{org_id}/users");
    let (status, operator) = call(
        &app,
        "POST",
        &users,
        Some(json!({ "email": "peao@boavista.com.br", "role": "OPERADOR_CURRAL" })),
        Some(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{operator}");
    let operator_id: Uuid = operator["userId"].as_str().unwrap().parse().unwrap();
    let operator = sign_in(&app, &db, "peao@boavista.com.br", operator_id).await;

    // ASSERT: the operator cannot administer the organization or take the key.
    let (status, _) = call(
        &app,
        "POST",
        &users,
        Some(json!({ "email": "outro@boavista.com.br", "role": "ORG_ADMIN" })),
        Some(&operator),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let export = format!("/api/v2/organizations/{org_id}/export");
    let (status, _) = call(
        &app,
        "POST",
        &export,
        Some(json!({ "confirmWallet": wallet.to_string(), "code": "123456" })),
        Some(&operator),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // ASSERT (A1): nobody prepares a sponsored session anonymously, for a throwaway key, or for
    // another organization's wallet.
    let session_body = |custodian: [u8; 32]| json!({ "custodian": hex::encode(custodian), "maxEvents": 50, "durationSeconds": 3600 });
    let (status, _) = call(
        &app,
        "POST",
        "/api/v2/sessions",
        Some(session_body(wallet.to_bytes())),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(
        &app,
        "POST",
        "/api/v2/sessions",
        Some(session_body([0x77; 32])),
        Some(OPERATOR),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "unknown custodian");
    let (status, _) = call(
        &app,
        "POST",
        "/api/v2/sessions",
        Some(session_body([0x77; 32])),
        Some(&operator),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "not the user's organization wallet"
    );

    // ACTION: a session for the organization wallet, prepared by its corral operator.
    let (status, prepared) = call(
        &app,
        "POST",
        "/api/v2/sessions",
        Some(session_body(wallet.to_bytes())),
        Some(&operator),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{prepared}");
    let session_id = prepared["session"]["sessionId"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, signed) = call(
        &app,
        "POST",
        "/api/v2/org/sign",
        Some(json!({ "purpose": { "kind": "SESSION_OPEN", "sessionId": session_id } })),
        Some(&operator),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{signed}");

    // ASSERT: one broadcast, sponsor in slot 0 and the organization wallet in slot 1, both
    // valid over the exact message; the signature is in the organization's audit trail.
    let sent = rpc.sent.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    let transaction = LegacyTransaction::decode(&sent[0]).unwrap();
    assert_eq!(transaction.account_keys[0], pubkey(SPONSOR_SEED));
    assert_eq!(transaction.account_keys[1], wallet);
    assert!(verify(
        &pubkey(SPONSOR_SEED),
        &transaction.message,
        &transaction.signatures[0]
    ));
    assert!(verify(
        &wallet,
        &transaction.message,
        &transaction.signatures[1]
    ));
    assert_eq!(signed["feePayer"], pubkey(SPONSOR_SEED).to_string());
    let logged: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM v2_org_signatures WHERE org_id=$1 AND user_id=$2 AND purpose='SESSION_OPEN'",
    )
    .bind(org_id)
    .bind(operator_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(logged, 1);

    // ASSERT: custody stays an administrator decision; a stranger's token signs nothing.
    let (status, _) = call(
        &app,
        "POST",
        "/api/v2/org/sign",
        Some(json!({ "purpose": { "kind": "CUSTODY_PROPOSE", "transferId": Uuid::new_v4() } })),
        Some(&operator),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = call(
        &app,
        "POST",
        "/api/v2/org/sign",
        Some(json!({ "purpose": { "kind": "SESSION_REVOKE", "sessionId": session_id } })),
        Some(&"ab".repeat(32)),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        rpc.sent.lock().unwrap().len(),
        1,
        "nothing else was broadcast"
    );

    // ACTION/ASSERT: the admin disables the operator, whose token dies at once.
    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/v2/organizations/{org_id}/users/{operator_id}/disable"),
        None,
        Some(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&app, "GET", "/api/v2/me", None, Some(&operator)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // ACTION/ASSERT (A3): the export needs the wallet repeated and a fresh one-time code, works
    // once, is audited, and ends managed signing.
    let (status, _) = call(
        &app,
        "POST",
        &export,
        Some(json!({ "confirmWallet": pubkey(1).to_string(), "code": "000000" })),
        Some(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &app,
        "POST",
        &export,
        Some(json!({ "confirmWallet": wallet.to_string(), "code": "000000" })),
        Some(&admin),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a stolen token alone cannot export"
    );
    deliver_code(&db, admin_id, "802468").await;
    let (status, exported) = call(
        &app,
        "POST",
        &export,
        Some(json!({ "confirmWallet": wallet.to_string(), "code": "802468" })),
        Some(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    deliver_code(&db, admin_id, "802469").await;
    let (status, _) = call(
        &app,
        "POST",
        &export,
        Some(json!({ "confirmWallet": wallet.to_string(), "code": "802469" })),
        Some(&admin),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "the key leaves custody only once"
    );
    let (status, _) = call(
        &app,
        "POST",
        "/api/v2/org/sign",
        Some(json!({ "purpose": { "kind": "SESSION_REVOKE", "sessionId": session_id } })),
        Some(&admin),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "no managed signing after export"
    );
    let keypair: Vec<u8> = serde_json::from_value(exported["keypair"].clone()).unwrap();
    let key = SigningKey::from_keypair_bytes(&keypair.try_into().unwrap()).unwrap();
    assert_eq!(key.verifying_key().to_bytes(), wallet.to_bytes());
    let (exported_at, sealed): (Option<time::OffsetDateTime>, Vec<u8>) =
        sqlx::query_as("SELECT exported_at, sealed_secret FROM v2_organizations WHERE org_id=$1")
            .bind(org_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(exported_at.is_some());
    assert!(
        !sealed
            .windows(32)
            .any(|window| window == key.to_bytes().as_slice()),
        "the database never holds the plaintext key"
    );
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM v2_audit_log WHERE action IN ('ORGANIZATION_CREATED','ORGANIZATION_USER_ADDED','ORGANIZATION_USER_DISABLED','ORGANIZATION_KEY_EXPORTED')",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(audited, 4);
    let (status, _) = call(&app, "POST", "/api/v2/auth/logout", None, Some(&admin)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&app, "GET", "/api/v2/me", None, Some(&admin)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "logout revokes the token");
    let swapped = sqlx::query("UPDATE v2_organizations SET wallet=$2 WHERE org_id=$1")
        .bind(org_id)
        .bind([7u8; 32].to_vec())
        .execute(&db.pool)
        .await;
    assert!(swapped.is_err(), "an organization's wallet is immutable");
    db.cleanup().await;
    // FAILURE MEANS: a role could sign outside its purposes, the key could leak or be swapped,
    // or a person without a wallet could not operate the corral.
}

#[tokio::test]
async fn one_time_codes_are_bounded_and_do_not_reveal_accounts() {
    // PURPOSE: S2 — login codes resist guessing and the request route leaks nothing.
    // ARRANGE: one organization, self-paid deployment.
    let db = TestDb::new().await;
    let app = app(&db, Arc::new(OrgRpc::default()), false);
    let (_, org) = call(
        &app,
        "POST",
        "/api/v2/organizations",
        Some(json!({ "name": "Frigorifico Sul", "adminEmail": "ops@sul.com.br" })),
        Some(OPERATOR),
    )
    .await;
    let admin_id: Uuid = org["adminUserId"].as_str().unwrap().parse().unwrap();

    // ASSERT: unknown e-mails get the same answer; malformed ones are rejected.
    for email in ["ops@sul.com.br", "nobody@sul.com.br"] {
        let (status, _) = call(
            &app,
            "POST",
            "/api/v2/auth/otp/request",
            Some(json!({ "email": email })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
    }
    let (status, _) = call(
        &app,
        "POST",
        "/api/v2/auth/otp/request",
        Some(json!({ "email": "not-an-email" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // ACTION: five wrong guesses against a known code.
    deliver_code(&db, admin_id, "135790").await;
    let verify = |code: &'static str| {
        let app = app.clone();
        async move {
            call(
                &app,
                "POST",
                "/api/v2/auth/otp/verify",
                Some(json!({ "email": "ops@sul.com.br", "code": code })),
                None,
            )
            .await
            .0
        }
    };
    for _ in 0..5 {
        assert_eq!(verify("000000").await, StatusCode::UNAUTHORIZED);
    }
    // ASSERT: the code is dead after the attempt budget, even when finally right.
    assert_eq!(verify("135790").await, StatusCode::UNAUTHORIZED);

    // ASSERT: a fresh code works exactly once, then requests are rate limited.
    deliver_code(&db, admin_id, "975310").await;
    assert_eq!(verify("975310").await, StatusCode::OK);
    assert_eq!(verify("975310").await, StatusCode::UNAUTHORIZED);
    let vault = LocalKeyVault::new(&MASTER_KEY);
    let mut issued = true;
    for _ in 0..5 {
        issued = repository::orgs::issue_code(&db.pool, admin_id, vault.code_hash(admin_id, "1"))
            .await
            .unwrap();
    }
    assert!(!issued, "at most five codes per user per window");
    let (status, _) = call(&app, "GET", "/api/v2/me", None, Some(&"0".repeat(64))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    db.cleanup().await;
    // FAILURE MEANS: a six-digit code could be brute-forced or accounts enumerated.
}

#[tokio::test]
async fn the_sponsor_only_recognizes_wallets_the_platform_knows() {
    // PURPOSE: A1 — the sponsor pays only for active parties and organization wallets, so a
    // throwaway key cannot drain the budget whatever route prepared its transaction.
    let db = TestDb::new().await;
    let app = app(&db, Arc::new(OrgRpc::default()), true);
    let (_, org) = call(
        &app,
        "POST",
        "/api/v2/organizations",
        Some(json!({ "name": "Fazenda Norte", "adminEmail": "adm@norte.com.br" })),
        Some(OPERATOR),
    )
    .await;
    let wallet: Pubkey = org["wallet"].as_str().unwrap().parse().unwrap();
    for (party, wallet, status) in [
        ([1u8; 32], [0x21u8; 32], "ACTIVE"),
        ([2; 32], [0x22; 32], "SUSPENDED"),
    ] {
        sqlx::query("INSERT INTO v2_parties(party_id,deployment_id,legal_name,wallet,role,status) VALUES($1,$2,'Parte',$3,1,$4)")
            .bind(party.to_vec())
            .bind(DEPLOYMENT_ID.to_vec())
            .bind(wallet.to_vec())
            .bind(status)
            .execute(&db.pool)
            .await
            .unwrap();
    }
    let known = |candidate: [u8; 32]| {
        let pool = db.pool.clone();
        async move {
            repository::sponsor::is_known_signer(&pool, DEPLOYMENT_ID, candidate)
                .await
                .unwrap()
        }
    };
    assert!(known(wallet.to_bytes()).await, "organization wallet");
    assert!(known([0x21; 32]).await, "active party");
    assert!(!known([0x22; 32]).await, "suspended party");
    assert!(!known([0x99; 32]).await, "throwaway key");
    db.cleanup().await;
    // FAILURE MEANS: any throwaway key could make the platform pay fees and rent.
}
