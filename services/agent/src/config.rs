use std::{env, path::PathBuf, time::Duration};

use crate::error::AgentError;

/// Runtime configuration loaded once at process start.
#[derive(Clone)]
pub struct AgentConfig {
    pub api_base_url: String,
    pub agent_token: String,
    pub station_serial_path: PathBuf,
    pub station_baud: u32,
    pub station_pubkey33: [u8; 33],
    pub sqlite_url: String,
    pub poll_interval: Duration,
    pub request_timeout: Duration,
    pub station_response_timeout: Duration,
}

impl AgentConfig {
    pub fn from_env() -> Result<Self, AgentError> {
        Self::from_lookup(|name| env::var(name).ok())
    }

    pub fn from_lookup<F>(mut lookup: F) -> Result<Self, AgentError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        fn required<F>(lookup: &mut F, name: &'static str) -> Result<String, AgentError>
        where
            F: FnMut(&str) -> Option<String>,
        {
            let value =
                lookup(name).ok_or_else(|| AgentError::Config(format!("missing {name}")))?;
            if value.trim().is_empty() {
                return Err(AgentError::Config(format!("{name} must not be empty")));
            }
            Ok(value)
        }

        fn positive_u64(name: &'static str, value: &str) -> Result<u64, AgentError> {
            let parsed: u64 = value
                .parse()
                .map_err(|_| AgentError::Config(format!("{name} must be a positive integer")))?;
            if parsed == 0 {
                return Err(AgentError::Config(format!(
                    "{name} must be greater than zero"
                )));
            }
            Ok(parsed)
        }

        let api_base_url = required(&mut lookup, "LASTRO_AGENT_API_URL")?;
        let parsed_url = reqwest::Url::parse(&api_base_url).map_err(|_| {
            AgentError::Config("LASTRO_AGENT_API_URL must be an absolute http(s) URL".into())
        })?;
        if !matches!(parsed_url.scheme(), "http" | "https") {
            return Err(AgentError::Config(
                "LASTRO_AGENT_API_URL must use http or https".into(),
            ));
        }

        let agent_token = required(&mut lookup, "LASTRO_AGENT_TOKEN")?;
        if agent_token.len() < 32 {
            return Err(AgentError::Config(
                "LASTRO_AGENT_TOKEN must contain at least 32 characters".into(),
            ));
        }

        let station_serial_path = PathBuf::from(required(&mut lookup, "LASTRO_AGENT_SERIAL_PORT")?);
        let station_baud_raw = required(&mut lookup, "LASTRO_AGENT_SERIAL_BAUD")?;
        let station_baud: u32 = station_baud_raw.parse().map_err(|_| {
            AgentError::Config("LASTRO_AGENT_SERIAL_BAUD must be a positive u32".into())
        })?;
        if station_baud == 0 {
            return Err(AgentError::Config(
                "LASTRO_AGENT_SERIAL_BAUD must be greater than zero".into(),
            ));
        }

        let pubkey_hex = required(&mut lookup, "LASTRO_STATION_PUBKEY_HEX")?;
        if pubkey_hex.len() != 66 || pubkey_hex != pubkey_hex.to_ascii_lowercase() {
            return Err(AgentError::Config(
                "LASTRO_STATION_PUBKEY_HEX must be a 33-byte compressed P-256 public key".into(),
            ));
        }
        let pubkey = hex::decode(&pubkey_hex).map_err(|_| {
            AgentError::Config("LASTRO_STATION_PUBKEY_HEX must be lowercase hex".into())
        })?;
        let station_pubkey33: [u8; 33] = pubkey.try_into().map_err(|_| {
            AgentError::Config(
                "LASTRO_STATION_PUBKEY_HEX must be a 33-byte compressed P-256 public key".into(),
            )
        })?;
        lastro_protocol::crypto::derive_station_id(&station_pubkey33).map_err(|_| {
            AgentError::Config("LASTRO_STATION_PUBKEY_HEX is not a valid P-256 public key".into())
        })?;

        let sqlite_url = required(&mut lookup, "LASTRO_AGENT_SQLITE_URL")?;
        if !sqlite_url.starts_with("sqlite:") {
            return Err(AgentError::Config(
                "LASTRO_AGENT_SQLITE_URL must use the sqlite: scheme".into(),
            ));
        }

        let poll_ms = positive_u64(
            "LASTRO_AGENT_POLL_INTERVAL_MS",
            &required(&mut lookup, "LASTRO_AGENT_POLL_INTERVAL_MS")?,
        )?;
        let timeout_ms = positive_u64(
            "LASTRO_AGENT_REQUEST_TIMEOUT_MS",
            &required(&mut lookup, "LASTRO_AGENT_REQUEST_TIMEOUT_MS")?,
        )?;
        let station_response_timeout_ms = positive_u64(
            "LASTRO_AGENT_STATION_RESPONSE_TIMEOUT_MS",
            &required(&mut lookup, "LASTRO_AGENT_STATION_RESPONSE_TIMEOUT_MS")?,
        )?;

        Ok(Self {
            api_base_url,
            agent_token,
            station_serial_path,
            station_baud,
            station_pubkey33,
            sqlite_url,
            poll_interval: Duration::from_millis(poll_ms),
            request_timeout: Duration::from_millis(timeout_ms),
            station_response_timeout: Duration::from_millis(station_response_timeout_ms),
        })
    }
}
