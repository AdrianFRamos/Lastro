//! API startup configuration contracts.

use std::collections::HashMap;

use lastro_api::{config::AppConfig, error::ApiError};

const STATION_PUBKEY_HEX: &str =
    "036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296";
const PROGRAM_ID: &str = "Vote111111111111111111111111111111111111111";
const AGENT_TOKEN: &str = "0123456789abcdef0123456789abcdef";

fn valid_values() -> HashMap<&'static str, String> {
    HashMap::from([
        ("LASTRO_API_BIND", "127.0.0.1:8080".into()),
        (
            "LASTRO_DATABASE_URL",
            "postgres://lastro:lastro@127.0.0.1:5432/lastro".into(),
        ),
        ("LASTRO_AGENT_TOKEN", AGENT_TOKEN.into()),
        (
            "LASTRO_SOLANA_RPC_URL",
            "https://api.devnet.solana.com".into(),
        ),
        ("LASTRO_DEPLOYMENT_ID_HEX", "d0".repeat(32)),
        ("LASTRO_STATION_PUBKEY_HEX", STATION_PUBKEY_HEX.into()),
        ("LASTRO_PROGRAM_ID", PROGRAM_ID.into()),
    ])
}

fn parse(values: &HashMap<&'static str, String>) -> Result<AppConfig, ApiError> {
    AppConfig::from_lookup(|name| values.get(name).cloned())
}

fn config_message(result: Result<AppConfig, ApiError>) -> String {
    match result {
        Err(ApiError::Config(message)) => message,
        Err(other) => panic!("expected configuration error, got {other:?}"),
        Ok(_) => panic!("expected configuration error, got valid configuration"),
    }
}

#[test]
fn startup_rejects_every_missing_required_variable_individually() {
    // PURPOSE: no deployment input may silently fall back to another environment.
    let required = [
        "LASTRO_API_BIND",
        "LASTRO_DATABASE_URL",
        "LASTRO_AGENT_TOKEN",
        "LASTRO_SOLANA_RPC_URL",
        "LASTRO_DEPLOYMENT_ID_HEX",
        "LASTRO_STATION_PUBKEY_HEX",
        "LASTRO_PROGRAM_ID",
    ];

    for key in required {
        let mut values = valid_values();
        values.remove(key);
        let message = config_message(parse(&values));
        assert!(message.contains(key), "error did not name {key}: {message}");
    }

    // ASSERT: every required deployment/security input is explicit.
    // FAILURE MEANS: the API can boot against an unintended deployment or trust boundary.
}

#[test]
fn startup_rejects_invalid_fixed_length_crypto_identifiers() {
    // PURPOSE: freeze deployment and Station identity shapes before any clients are created.
    let invalid_deployments = ["00".repeat(31), "00".repeat(33), "GG".repeat(32)];
    for value in invalid_deployments {
        let mut values = valid_values();
        values.insert("LASTRO_DEPLOYMENT_ID_HEX", value);
        assert!(parse(&values).is_err());
    }

    let invalid_station_keys = [
        "02".to_string() + &"00".repeat(31),
        "02".to_string() + &"00".repeat(33),
        "04".to_string() + &"00".repeat(32),
        "02".to_string() + &"ff".repeat(32),
        STATION_PUBKEY_HEX.to_ascii_uppercase(),
    ];
    for value in invalid_station_keys {
        let mut values = valid_values();
        values.insert("LASTRO_STATION_PUBKEY_HEX", value);
        assert!(parse(&values).is_err());
    }

    let mut invalid_program = valid_values();
    invalid_program.insert("LASTRO_PROGRAM_ID", "not-a-solana-address".into());
    assert!(parse(&invalid_program).is_err());

    // ASSERT: malformed sizes, encodings, curve points, and Solana addresses all fail closed.
    // FAILURE MEANS: runtime requests can operate with ambiguous cryptographic configuration.
}

#[test]
fn startup_rejects_invalid_network_and_database_endpoints() {
    // PURPOSE: startup must not accept endpoints for unsupported transports.
    for database_url in ["http://database.example", "sqlite://lastro.db"] {
        let mut values = valid_values();
        values.insert("LASTRO_DATABASE_URL", database_url.into());
        assert!(parse(&values).is_err());
    }

    for rpc_url in ["api.devnet.solana.com", "ftp://rpc.example"] {
        let mut values = valid_values();
        values.insert("LASTRO_SOLANA_RPC_URL", rpc_url.into());
        assert!(parse(&values).is_err());
    }

    let mut invalid_bind = valid_values();
    invalid_bind.insert("LASTRO_API_BIND", "localhost:not-a-port".into());
    assert!(parse(&invalid_bind).is_err());

    // ASSERT: only explicit PostgreSQL, HTTP(S) RPC, and socket-address inputs are accepted.
    // FAILURE MEANS: startup can defer obvious deployment mistakes into runtime failures.
}

#[test]
fn startup_rejects_short_demo_agent_token_without_leaking_it() {
    // PURPOSE: transport authentication must not silently accept a trivial token or expose it in errors.
    let secret = "too-short-secret";
    let mut values = valid_values();
    values.insert("LASTRO_AGENT_TOKEN", secret.into());

    let message = config_message(parse(&values));
    assert!(!message.contains(secret));
    assert!(message.contains("LASTRO_AGENT_TOKEN"));

    // ASSERT: short tokens fail and the rejected secret is not reflected in the error.
    // FAILURE MEANS: the Agent authentication boundary can weaken or leak credentials.
}

#[test]
fn startup_accepts_one_complete_valid_environment_without_rewriting_values() {
    // PURPOSE: prove the parser preserves the configured deployment exactly.
    let values = valid_values();
    let config = parse(&values).expect("valid configuration");

    assert_eq!(config.bind_addr.to_string(), "127.0.0.1:8080");
    assert_eq!(
        config.database_url,
        "postgres://lastro:lastro@127.0.0.1:5432/lastro"
    );
    assert_eq!(config.agent_token, AGENT_TOKEN);
    assert_eq!(config.solana_rpc_url, "https://api.devnet.solana.com");
    assert_eq!(config.deployment_id, [0xd0; 32]);
    assert_eq!(hex::encode(config.station_pubkey33), STATION_PUBKEY_HEX);
    assert_eq!(config.lastro_program_id, PROGRAM_ID);

    // ASSERT: typed bytes and endpoint/address strings equal the declared inputs exactly.
    // FAILURE MEANS: configuration normalization can point the API at a different deployment.
}
