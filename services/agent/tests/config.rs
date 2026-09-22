//! Agent startup configuration contracts.

use std::{collections::HashMap, time::Duration};

use lastro_agent::{config::AgentConfig, error::AgentError};

const STATION_PUBKEY_HEX: &str =
    "036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296";
const AGENT_TOKEN: &str = "0123456789abcdef0123456789abcdef";

fn valid_values() -> HashMap<&'static str, String> {
    HashMap::from([
        ("LASTRO_AGENT_API_URL", "http://127.0.0.1:8080/".into()),
        ("LASTRO_AGENT_TOKEN", AGENT_TOKEN.into()),
        ("LASTRO_AGENT_SERIAL_PORT", "/dev/ttyUSB0".into()),
        ("LASTRO_AGENT_SERIAL_BAUD", "115200".into()),
        ("LASTRO_STATION_PUBKEY_HEX", STATION_PUBKEY_HEX.into()),
        ("LASTRO_AGENT_SQLITE_URL", "sqlite://agent.db?mode=rwc".into()),
        ("LASTRO_AGENT_POLL_INTERVAL_MS", "500".into()),
        ("LASTRO_AGENT_REQUEST_TIMEOUT_MS", "5000".into()),
    ])
}

fn parse(values: &HashMap<&'static str, String>) -> Result<AgentConfig, AgentError> {
    AgentConfig::from_lookup(|name| values.get(name).cloned())
}

fn config_message(result: Result<AgentConfig, AgentError>) -> String {
    match result {
        Err(AgentError::Config(message)) => message,
        Err(other) => panic!("expected configuration error, got {other:?}"),
        Ok(_) => panic!("expected configuration error, got valid configuration"),
    }
}

#[test]
fn agent_requires_every_transport_identity_database_and_timing_value() {
    // PURPOSE: the bridge must never guess its API, Station, database, or retry configuration.
    let required = [
        "LASTRO_AGENT_API_URL",
        "LASTRO_AGENT_TOKEN",
        "LASTRO_AGENT_SERIAL_PORT",
        "LASTRO_AGENT_SERIAL_BAUD",
        "LASTRO_STATION_PUBKEY_HEX",
        "LASTRO_AGENT_SQLITE_URL",
        "LASTRO_AGENT_POLL_INTERVAL_MS",
        "LASTRO_AGENT_REQUEST_TIMEOUT_MS",
    ];

    for key in required {
        let mut values = valid_values();
        values.remove(key);
        let message = config_message(parse(&values));
        assert!(message.contains(key), "error did not name {key}: {message}");
    }

    // ASSERT: every runtime trust/transport input is explicit.
    // FAILURE MEANS: the Agent can bind to a wrong Station, API, or local database silently.
}

#[test]
fn agent_rejects_invalid_transport_identity_database_and_timing_values() {
    // PURPOSE: reject values that make serial, retry, identity, or HTTP behavior ambiguous.
    let cases = [
        ("LASTRO_AGENT_API_URL", "ftp://api.example"),
        ("LASTRO_AGENT_API_URL", "api.example"),
        ("LASTRO_AGENT_TOKEN", "short"),
        ("LASTRO_AGENT_SERIAL_PORT", ""),
        ("LASTRO_AGENT_SERIAL_BAUD", "0"),
        ("LASTRO_AGENT_SERIAL_BAUD", "4294967296"),
        ("LASTRO_AGENT_SQLITE_URL", "postgres://localhost/lastro"),
        ("LASTRO_AGENT_POLL_INTERVAL_MS", "0"),
        ("LASTRO_AGENT_REQUEST_TIMEOUT_MS", "0"),
    ];

    for (key, value) in cases {
        let mut values = valid_values();
        values.insert(key, value.into());
        assert!(parse(&values).is_err(), "{key}={value:?} unexpectedly passed");
    }

    for station_key in [
        "02".to_string() + &"00".repeat(31),
        "04".to_string() + &"00".repeat(32),
        "02".to_string() + &"ff".repeat(32),
        STATION_PUBKEY_HEX.to_ascii_uppercase(),
    ] {
        let mut values = valid_values();
        values.insert("LASTRO_STATION_PUBKEY_HEX", station_key);
        assert!(parse(&values).is_err());
    }

    // ASSERT: invalid boundaries fail before serial/API workers start.
    // FAILURE MEANS: runtime can enter busy loops, immediate timeouts, or trust the wrong Station.
}

#[test]
fn agent_rejects_short_token_without_leaking_it() {
    // PURPOSE: authentication errors must not echo a rejected secret.
    let secret = "short-agent-secret";
    let mut values = valid_values();
    values.insert("LASTRO_AGENT_TOKEN", secret.into());

    let message = config_message(parse(&values));
    assert!(!message.contains(secret));
    assert!(message.contains("LASTRO_AGENT_TOKEN"));

    // ASSERT: the token is rejected by policy but never included in the error text.
    // FAILURE MEANS: startup can leak credentials through logs or diagnostics.
}

#[test]
fn agent_valid_configuration_preserves_serial_and_retry_values_exactly() {
    // PURPOSE: prove operational transport and timing values are explicit and reproducible.
    let values = valid_values();
    let config = parse(&values).expect("valid Agent configuration");

    assert_eq!(config.api_base_url, "http://127.0.0.1:8080/");
    assert_eq!(config.agent_token, AGENT_TOKEN);
    assert_eq!(config.station_serial_path.to_string_lossy(), "/dev/ttyUSB0");
    assert_eq!(config.station_baud, 115200);
    assert_eq!(hex::encode(config.station_pubkey33), STATION_PUBKEY_HEX);
    assert_eq!(config.sqlite_url, "sqlite://agent.db?mode=rwc");
    assert_eq!(config.poll_interval, Duration::from_millis(500));
    assert_eq!(config.request_timeout, Duration::from_millis(5000));

    // ASSERT: typed fields equal the declared inputs with no hidden defaults or unit changes.
    // FAILURE MEANS: deployed bridge behavior can differ from documented configuration.
}
