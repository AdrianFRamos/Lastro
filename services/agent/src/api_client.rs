use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};

use crate::{command::StationCommand, error::AgentError, spool::model::OutboxRow};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceStatus {
    EvidenceAccepted,
    Submitted,
    Finalized,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvidenceStatusDto {
    status: EvidenceStatus,
}

/// HTTP boundary used by the Agent. Authentication, timeout and strict DTO conversion live here.
pub struct ApiClient {
    client: Client,
    base_url: Url,
    bearer_token: String,
}

impl ApiClient {
    pub fn new(
        base_url: String,
        bearer_token: String,
        timeout: Duration,
    ) -> Result<Self, AgentError> {
        let base_url = Url::parse(&base_url)
            .map_err(|error| AgentError::Config(format!("invalid Agent API URL: {error}")))?;
        if !matches!(base_url.scheme(), "http" | "https") {
            return Err(AgentError::Config(
                "Agent API URL must use http or https".into(),
            ));
        }
        if bearer_token.len() < 32 {
            return Err(AgentError::Config(
                "Agent bearer token must contain at least 32 characters".into(),
            ));
        }
        if timeout.is_zero() {
            return Err(AgentError::Config(
                "Agent API timeout must be greater than zero".into(),
            ));
        }
        let client = Client::builder()
            .timeout(timeout)
            .user_agent("lastro-agent/0.1")
            .build()
            .map_err(|error| AgentError::Config(format!("cannot build HTTP client: {error}")))?;
        Ok(Self {
            client,
            base_url,
            bearer_token,
        })
    }

    /// Poll exactly one pending command for this Station. JSON null means no command.
    pub async fn poll_command(&self) -> Result<Option<StationCommand>, AgentError> {
        let url = self.endpoint("api/agent/commands")?;
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.bearer_token)
            .send()
            .await
            .map_err(api_transport)?;
        if response.status() != StatusCode::OK {
            return Err(status_error(response.status(), "poll Agent command"));
        }
        let value: Option<AgentCommandDto> = response.json().await.map_err(|error| {
            AgentError::ApiTerminal(format!("invalid Agent command JSON: {error}"))
        })?;
        value.map(StationCommand::try_from).transpose()
    }

    /// Read lifecycle state for evidence already accepted by the API.
    pub async fn evidence_status(
        &self,
        event_hash: &[u8; 32],
    ) -> Result<EvidenceStatus, AgentError> {
        let url = self.endpoint(&format!("api/agent/evidence/{}", hex::encode(event_hash)))?;
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.bearer_token)
            .send()
            .await
            .map_err(api_transport)?;
        if response.status() != StatusCode::OK {
            return Err(status_error(
                response.status(),
                "read Agent evidence status",
            ));
        }
        let value: EvidenceStatusDto = response.json().await.map_err(|error| {
            AgentError::ApiTerminal(format!("invalid Agent evidence status JSON: {error}"))
        })?;
        Ok(value.status)
    }

    /// Submit bytes already persisted in LOCAL state. 200 is an exact duplicate; 201 is a new row.
    pub async fn post_evidence(&self, row: &OutboxRow) -> Result<(), AgentError> {
        let url = self.endpoint("api/agent/evidence")?;
        let body = AgentEvidenceDto {
            capture_id: row.capture_id,
            event_bytes_base64: BASE64.encode(row.event_bytes),
            observed_rfid_hex: hex::encode(row.observed_rfid),
            station_pubkey_hex: hex::encode(row.station_pubkey),
            station_signature_hex: hex::encode(row.station_signature),
        };
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.bearer_token)
            .json(&body)
            .send()
            .await
            .map_err(api_transport)?;
        match response.status() {
            StatusCode::OK | StatusCode::CREATED => Ok(()),
            status => Err(status_error(status, "submit Agent evidence")),
        }
    }

    fn endpoint(&self, path: &str) -> Result<Url, AgentError> {
        self.base_url
            .join(path)
            .map_err(|error| AgentError::Config(format!("invalid Agent API endpoint: {error}")))
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ActionDto {
    Origin,
    Transfer,
    Reidentify,
}

impl ActionDto {
    const fn code(self) -> u8 {
        match self {
            Self::Origin => 1,
            Self::Transfer => 2,
            Self::Reidentify => 3,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AgentCommandDto {
    capture_id: uuid::Uuid,
    action: ActionDto,
    deployment_id: String,
    animal_id: String,
    event_sequence: u64,
    identity_revision: u32,
    previous_event_hash: String,
    expected_old_rfid_hash: String,
    from_custodian: String,
    to_custodian: String,
}

impl TryFrom<AgentCommandDto> for StationCommand {
    type Error = AgentError;

    fn try_from(value: AgentCommandDto) -> Result<Self, Self::Error> {
        let command = StationCommand {
            capture_id: value.capture_id,
            action: value.action.code(),
            deployment_id: decode_hex32("deploymentId", &value.deployment_id)?,
            animal_id: decode_hex32("animalId", &value.animal_id)?,
            event_sequence: value.event_sequence,
            identity_revision: value.identity_revision,
            previous_event_hash: decode_hex32("previousEventHash", &value.previous_event_hash)?,
            expected_old_rfid_hash: decode_hex32(
                "expectedOldRfidHash",
                &value.expected_old_rfid_hash,
            )?,
            from_custodian: decode_hex32("fromCustodian", &value.from_custodian)?,
            to_custodian: decode_hex32("toCustodian", &value.to_custodian)?,
        };
        command.validate()?;
        Ok(command)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentEvidenceDto {
    capture_id: uuid::Uuid,
    event_bytes_base64: String,
    observed_rfid_hex: String,
    station_pubkey_hex: String,
    station_signature_hex: String,
}

fn decode_hex32(name: &str, value: &str) -> Result<[u8; 32], AgentError> {
    if value.len() != 64 || value != value.to_ascii_lowercase() {
        return Err(AgentError::ApiTerminal(format!(
            "{name} must be 32-byte lowercase hex"
        )));
    }
    let bytes = hex::decode(value)
        .map_err(|_| AgentError::ApiTerminal(format!("{name} must be 32-byte lowercase hex")))?;
    bytes
        .try_into()
        .map_err(|_| AgentError::ApiTerminal(format!("{name} must be 32-byte lowercase hex")))
}

fn api_transport(error: reqwest::Error) -> AgentError {
    // Reqwest errors can retain the full request URL. Strip it before the error
    // crosses into durable retry diagnostics so URL credentials/query secrets
    // cannot be persisted or surfaced by callers.
    AgentError::Api(error.without_url().to_string())
}

fn status_error(status: StatusCode, operation: &str) -> AgentError {
    if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
        AgentError::Api(format!("{operation} returned HTTP {status}"))
    } else {
        AgentError::ApiTerminal(format!("{operation} returned HTTP {status}"))
    }
}
