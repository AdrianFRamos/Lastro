use std::{
    collections::HashMap,
    env,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use async_trait::async_trait;
use lastro_api::{
    config::AppConfig,
    error::ApiError,
    solana::rpc::{CanonicalAnimalState, CanonicalRfidBinding, SolanaRpc},
    state::AppState,
};
use lastro_protocol::ids::{AnimalId, DeploymentId, RfidHash};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

pub const DEPLOYMENT_ID: [u8; 32] = [0xd0; 32];
pub const STATION_PUBKEY: [u8; 33] = [
    0x03, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63,
    0xa4, 0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39,
    0x45, 0xd8, 0x98, 0xc2, 0x96,
];
pub const PROGRAM_ID: &str = "Vote111111111111111111111111111111111111111";
pub const AGENT_TOKEN: &str = "0123456789abcdef0123456789abcdef";

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
                    sqlx::query(sqlx::AssertSqlSafe(format!(r#"SET search_path TO "{}""#, schema.as_str())))
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

        Self { pool, admin, schema }
    }

    pub async fn cleanup(self) {
        self.pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!(r#"DROP SCHEMA "{}" CASCADE"#, self.schema)))
            .execute(&self.admin)
            .await
            .expect("drop isolated test schema");
        self.admin.close().await;
    }
}

#[derive(Default)]
pub struct TestRpc {
    station_pubkey: Mutex<Option<[u8; 33]>>,
    animals: Mutex<HashMap<AnimalId, CanonicalAnimalState>>,
    bindings: Mutex<HashMap<RfidHash, CanonicalRfidBinding>>,
    confirmed_transaction_match: Mutex<bool>,
    transaction_match: Mutex<bool>,
    calls: AtomicUsize,
}

impl TestRpc {
    pub fn with_station_pubkey(station_pubkey: [u8; 33]) -> Self {
        Self {
            station_pubkey: Mutex::new(Some(station_pubkey)),
            confirmed_transaction_match: Mutex::new(true),
            transaction_match: Mutex::new(true),
            ..Self::default()
        }
    }

    pub fn set_animal(&self, state: CanonicalAnimalState) {
        self.animals.lock().unwrap().insert(state.animal_id, state);
    }

    pub fn set_binding(&self, binding: CanonicalRfidBinding) {
        self.bindings.lock().unwrap().insert(binding.rfid_hash, binding);
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
    async fn protocol_station_pubkey(&self, _: DeploymentId) -> Result<[u8; 33], ApiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.station_pubkey
            .lock()
            .unwrap()
            .ok_or_else(|| ApiError::NotFound("canonical ProtocolConfig account not found".into()))
    }

    async fn animal_state(
        &self,
        _: DeploymentId,
        animal_id: AnimalId,
    ) -> Result<Option<CanonicalAnimalState>, ApiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.animals.lock().unwrap().get(&animal_id).cloned())
    }

    async fn rfid_binding(
        &self,
        _: DeploymentId,
        rfid_hash: RfidHash,
    ) -> Result<Option<CanonicalRfidBinding>, ApiError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.bindings.lock().unwrap().get(&rfid_hash).cloned())
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
            solana_rpc_url: "http://127.0.0.1:8899".into(),
            deployment_id: DEPLOYMENT_ID,
            station_pubkey33: STATION_PUBKEY,
            lastro_program_id: PROGRAM_ID.into(),
        }),
        rpc,
    }
}

pub fn sign_station_event_with_test_scalar(
    event: &lastro_protocol::StationEvent,
    scalar: u8,
) -> ([u8; 276], [u8; 33], [u8; 64]) {
    use p256::ecdsa::{signature::Signer, Signature, SigningKey};

    assert!(scalar != 0, "test signing scalar must be non-zero");
    let mut secret = [0u8; 32];
    secret[31] = scalar;
    let signing_key = SigningKey::from_slice(&secret).expect("valid test-only P-256 scalar");
    let event_bytes = event.encode();
    let mut signature: Signature = signing_key.sign(&event_bytes);
    if let Some(low_s) = signature.normalize_s() {
        signature = low_s;
    }
    let public_key: [u8; 33] = signing_key
        .verifying_key()
        .to_encoded_point(true)
        .as_bytes()
        .try_into()
        .expect("compressed P-256 key is 33 bytes");
    let signature: [u8; 64] = signature.to_bytes().into();
    (event_bytes, public_key, signature)
}
