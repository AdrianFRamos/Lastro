#![allow(dead_code)]

use std::{
    collections::HashMap,
    env,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use lastro_api::{
    config::AppConfig,
    error::ApiError,
    solana::rpc::{
        CanonicalV2AssetState, CanonicalV2EventAnchor, CanonicalV2ProtocolConfig,
        CanonicalV2Station, STATION_STATUS_ACTIVE, SolanaRpc,
    },
    state::AppState,
};
use solana_pubkey::Pubkey;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

pub const DEPLOYMENT_ID: [u8; 32] = [0xd0; 32];
/// Compressed P-256 public key of the test-only scalar 1 (same as the v2 vectors).
pub const STATION_PUBKEY: [u8; 33] = [
    0x03, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40,
    0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98, 0xc2,
    0x96,
];
pub const PROGRAM_ID: &str = "Vote111111111111111111111111111111111111111";
pub const AGENT_TOKEN: &str = "0123456789abcdef0123456789abcdef";
pub const AUTHORITY: [u8; 32] = [0xaa; 32];

pub struct TestDb {
    pub pool: PgPool,
    admin: PgPool,
    schema: String,
}

impl TestDb {
    pub async fn new() -> Self {
        let database_url = env::var("LASTRO_DATABASE_URL")
            .expect("LASTRO_DATABASE_URL must point to the isolated PostgreSQL test service");
        let admin = PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .expect("connect to PostgreSQL test service");

        let schema = format!("lastro_test_{}", Uuid::new_v4().simple());
        sqlx::query(sqlx::AssertSqlSafe(format!(r#"CREATE SCHEMA "{schema}""#)))
            .execute(&admin)
            .await
            .expect("create isolated test schema");

        let schema_for_connections = Arc::new(schema.clone());
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .after_connect(move |connection, _metadata| {
                let schema = Arc::clone(&schema_for_connections);
                Box::pin(async move {
                    sqlx::query(sqlx::AssertSqlSafe(format!(
                        r#"SET search_path TO "{}""#,
                        schema.as_str()
                    )))
                    .execute(connection)
                    .await?;
                    Ok(())
                })
            })
            .connect(&database_url)
            .await
            .expect("connect isolated PostgreSQL test pool");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("apply API migrations to isolated schema");

        Self {
            pool,
            admin,
            schema,
        }
    }

    pub async fn cleanup(self) {
        self.pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"DROP SCHEMA "{}" CASCADE"#,
            self.schema
        )))
        .execute(&self.admin)
        .await
        .expect("drop isolated test schema");
        self.admin.close().await;
    }
}

/// In-memory stand-in for finalized Solana state.
#[derive(Default)]
pub struct TestRpc {
    config: Mutex<Option<CanonicalV2ProtocolConfig>>,
    assets: Mutex<HashMap<[u8; 32], CanonicalV2AssetState>>,
    anchors: Mutex<HashMap<[u8; 32], CanonicalV2EventAnchor>>,
    confirmed_transaction_match: Mutex<bool>,
    transaction_match: Mutex<bool>,
    calls: AtomicUsize,
}

impl TestRpc {
    /// A deployment whose v2 ProtocolConfig exists; transactions verify by default.
    pub fn configured() -> Self {
        Self {
            config: Mutex::new(Some(CanonicalV2ProtocolConfig {
                authority: Pubkey::new_from_array(AUTHORITY),
                station_registry: Pubkey::new_from_array([0xbb; 32]),
                max_event_age_seconds: 86_400,
            })),
            confirmed_transaction_match: Mutex::new(true),
            transaction_match: Mutex::new(true),
            ..Self::default()
        }
    }

    pub fn set_asset(&self, asset: CanonicalV2AssetState) {
        self.assets.lock().unwrap().insert(asset.asset_id, asset);
    }

    pub fn set_anchor(&self, anchor: CanonicalV2EventAnchor) {
        self.anchors.lock().unwrap().insert(anchor.event_id, anchor);
    }

    pub fn set_confirmed_transaction_match(&self, matches: bool) {
        *self.confirmed_transaction_match.lock().unwrap() = matches;
    }

    pub fn set_transaction_match(&self, matches: bool) {
        *self.transaction_match.lock().unwrap() = matches;
    }

    pub fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl SolanaRpc for TestRpc {
    async fn v2_protocol_config(
        &self,
        _: [u8; 32],
    ) -> Result<Option<CanonicalV2ProtocolConfig>, ApiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.config.lock().unwrap().clone())
    }

    async fn v2_asset_state(
        &self,
        _: [u8; 32],
        asset_id: [u8; 32],
    ) -> Result<Option<CanonicalV2AssetState>, ApiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.assets.lock().unwrap().get(&asset_id).cloned())
    }

    async fn v2_event_anchor(
        &self,
        _: [u8; 32],
        event_id: [u8; 32],
    ) -> Result<Option<CanonicalV2EventAnchor>, ApiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.anchors.lock().unwrap().get(&event_id).cloned())
    }

    async fn v2_station(
        &self,
        _: [u8; 32],
        station_id: [u8; 32],
    ) -> Result<Option<CanonicalV2Station>, ApiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Some(CanonicalV2Station {
            station_id,
            pubkey33: STATION_PUBKEY,
            status: STATION_STATUS_ACTIVE,
            valid_from: 0,
            valid_until: i64::MAX,
        }))
    }

    async fn v2_event_anchor_signature(
        &self,
        _: [u8; 32],
        _: [u8; 32],
    ) -> Result<Option<String>, ApiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }

    async fn transaction_matches_confirmed(
        &self,
        _: &str,
        _: &lastro_api::model::TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(*self.confirmed_transaction_match.lock().unwrap())
    }

    async fn transaction_matches(
        &self,
        _: &str,
        _: &lastro_api::model::TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(*self.transaction_match.lock().unwrap())
    }
}

pub fn app_state(pool: PgPool, rpc: Arc<dyn SolanaRpc>) -> AppState {
    AppState {
        db: pool,
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
        }),
        rpc,
    }
}

/// An untagged, active animal asset at `state_version` with the given custodian.
pub fn untagged_asset(asset_id: [u8; 32], custodian: [u8; 32]) -> CanonicalV2AssetState {
    CanonicalV2AssetState {
        asset_id,
        asset_type: 1,
        status: 1,
        deployment_id: DEPLOYMENT_ID,
        custodian: Pubkey::new_from_array(custodian),
        parent_root: [0; 32],
        lineage_root: asset_id,
        current_lot_id: [0; 32],
        available_weight_grams: 500_000,
        event_sequence: 0,
        state_version: 0,
        last_event_hash: [0; 32],
        current_rfid_hash: [0; 32],
    }
}

/// Sign `message` like the Station does (test-only scalar 1, low-S compact r||s).
pub fn station_sign(message: &[u8]) -> [u8; 64] {
    use p256::ecdsa::{Signature, SigningKey, signature::Signer};
    let mut secret = [0u8; 32];
    secret[31] = 1;
    let key = SigningKey::from_slice(&secret).expect("valid test-only P-256 scalar");
    let signature: Signature = key.sign(message);
    signature
        .normalize_s()
        .unwrap_or(signature)
        .to_bytes()
        .into()
}

/// Deterministic ed25519 wallet for challenge signing in tests.
pub fn wallet(seed: u8) -> ed25519_dalek::SigningKey {
    ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
}
