//! Read-only canonical Solana RPC boundary used by the API.
//!
//! The backend is a projection/coordinator. Confirmation must query finalized RPC
//! state and prove the submitted transaction contains the exact Lastro envelope
//! before PostgreSQL current state changes.

use std::{collections::HashMap, time::Duration};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use reqwest::Client;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

const SOLANA_RPC_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

use crate::{
    error::ApiError,
    model::{InstructionDto, TransactionDataResponse, TransactionVersionDto},
    solana::v2_transaction_builder::{
        COMPUTE_BUDGET_PROGRAM_ID, parse_program_id, v2_asset_address, v2_event_anchor_address,
        v2_protocol_config_address, v2_rfid_binding_address, v2_station_address,
        v2_transformation_address,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingStatus {
    Active,
    Retired,
}

/// v2 RfidBinding: never closed, so a retired tag keeps pointing at its asset forever.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalV2RfidBinding {
    pub asset_id: [u8; 32],
    pub rfid_hash: [u8; 32],
    pub status: BindingStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalV2ProtocolConfig {
    pub authority: Pubkey,
    pub station_registry: Pubkey,
    pub max_event_age_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalV2Transformation {
    pub transformation_id: [u8; 32],
    pub facility_id: [u8; 32],
    pub input_root: [u8; 32],
    pub output_root: [u8; 32],
    pub status: u8,
    pub manifest_hash: [u8; 32],
    pub sequence: u64,
}

/// StationRecord (v2): the program rejects events outside `valid_from..=valid_until` or while
/// the Station is not ACTIVE.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalV2Station {
    pub station_id: [u8; 32],
    pub pubkey33: [u8; 33],
    pub status: u8,
    pub valid_from: i64,
    pub valid_until: i64,
}

pub const STATION_STATUS_ACTIVE: u8 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalV2EventAnchor {
    pub event_id: [u8; 32],
    pub deployment_id: [u8; 32],
    pub subject_id: [u8; 32],
    pub source_id: [u8; 32],
    pub event_type: u16,
    pub state_version: u64,
    pub observed_at: i64,
    pub expires_at: i64,
    pub expected_previous_hash: [u8; 32],
    pub payload_hash: [u8; 32],
    pub event_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalV2AssetState {
    pub asset_id: [u8; 32],
    pub asset_type: u8,
    pub status: u8,
    pub deployment_id: [u8; 32],
    pub custodian: Pubkey,
    pub parent_root: [u8; 32],
    pub lineage_root: [u8; 32],
    pub current_lot_id: [u8; 32],
    pub available_weight_grams: u64,
    pub event_sequence: u64,
    pub state_version: u64,
    pub last_event_hash: [u8; 32],
    /// Active RFID hash; all zero before IDENTIFIER_BOUND.
    pub current_rfid_hash: [u8; 32],
}

#[async_trait]
pub trait SolanaRpc: Send + Sync {
    async fn v2_protocol_config(
        &self,
        deployment_id: [u8; 32],
    ) -> Result<Option<CanonicalV2ProtocolConfig>, ApiError> {
        let _ = deployment_id;
        Err(ApiError::Unavailable(
            "v2 ProtocolConfig RPC is not implemented by this client".into(),
        ))
    }

    async fn v2_event_anchor(
        &self,
        deployment_id: [u8; 32],
        event_id: [u8; 32],
    ) -> Result<Option<CanonicalV2EventAnchor>, ApiError> {
        let _ = (deployment_id, event_id);
        Err(ApiError::Unavailable(
            "v2 EventAnchor RPC is not implemented by this client".into(),
        ))
    }

    async fn v2_asset_state(
        &self,
        deployment_id: [u8; 32],
        asset_id: [u8; 32],
    ) -> Result<Option<CanonicalV2AssetState>, ApiError> {
        let _ = (deployment_id, asset_id);
        Err(ApiError::Unavailable(
            "v2 AssetState RPC is not implemented by this client".into(),
        ))
    }

    async fn v2_rfid_binding(
        &self,
        deployment_id: [u8; 32],
        rfid_hash: [u8; 32],
    ) -> Result<Option<CanonicalV2RfidBinding>, ApiError> {
        let _ = (deployment_id, rfid_hash);
        Err(ApiError::Unavailable(
            "v2 RfidBinding RPC is not implemented by this client".into(),
        ))
    }

    async fn v2_transformation(
        &self,
        deployment_id: [u8; 32],
        transformation_id: [u8; 32],
    ) -> Result<Option<CanonicalV2Transformation>, ApiError> {
        let _ = (deployment_id, transformation_id);
        Err(ApiError::Unavailable(
            "v2 TransformationAnchor RPC is not implemented by this client".into(),
        ))
    }

    async fn v2_station(
        &self,
        deployment_id: [u8; 32],
        station_id: [u8; 32],
    ) -> Result<Option<CanonicalV2Station>, ApiError> {
        let _ = (deployment_id, station_id);
        Err(ApiError::Unavailable(
            "v2 StationRecord RPC is not implemented by this client".into(),
        ))
    }

    /// Signature of the finalized, successful transaction that created an EventAnchor. The
    /// anchor is written exactly once, so this recovers transactions a wallet broadcast but
    /// never reported to the API.
    async fn v2_event_anchor_signature(
        &self,
        deployment_id: [u8; 32],
        event_id: [u8; 32],
    ) -> Result<Option<String>, ApiError> {
        let _ = (deployment_id, event_id);
        Err(ApiError::Unavailable(
            "v2 EventAnchor signature RPC is not implemented by this client".into(),
        ))
    }

    /// Verifies the exact compiled Lastro transaction at confirmed commitment.
    async fn transaction_matches_confirmed(
        &self,
        signature: &str,
        expected: &TransactionDataResponse,
    ) -> Result<bool, ApiError>;

    /// Verifies the same exact transaction at finalized commitment.
    async fn transaction_matches(
        &self,
        signature: &str,
        expected: &TransactionDataResponse,
    ) -> Result<bool, ApiError>;
}

pub struct HttpSolanaRpc {
    pub url: String,
    pub lastro_program_id: String,
    client: Client,
}

impl HttpSolanaRpc {
    pub fn new(url: String, lastro_program_id: String) -> Self {
        Self {
            url,
            lastro_program_id,
            client: Client::new(),
        }
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, ApiError> {
        let response = self
            .client
            .post(&self.url)
            .timeout(SOLANA_RPC_REQUEST_TIMEOUT)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .await
            .map_err(|_| ApiError::Unavailable("Solana RPC request failed".into()))?;
        if !response.status().is_success() {
            return Err(ApiError::Unavailable(
                "Solana RPC returned an HTTP error".into(),
            ));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|_| ApiError::Unavailable("Solana RPC returned invalid JSON".into()))?;
        if body.get("error").is_some() {
            return Err(ApiError::Unavailable("Solana RPC returned an error".into()));
        }
        body.get("result")
            .cloned()
            .ok_or_else(|| ApiError::Unavailable("Solana RPC response is missing result".into()))
    }

    async fn transaction_matches_at_commitment(
        &self,
        signature: &str,
        expected: &TransactionDataResponse,
        commitment: &'static str,
    ) -> Result<bool, ApiError> {
        let result = self
            .call(
                "getTransaction",
                json!([signature, {"commitment":commitment,"encoding":"json","maxSupportedTransactionVersion":0}]),
            )
            .await?;
        if result.is_null() {
            return Ok(false);
        }
        transaction_matches_json(&result, signature, expected)
    }

    async fn account_data(&self, address: &Pubkey) -> Result<Option<Vec<u8>>, ApiError> {
        let result = self
            .call(
                "getAccountInfo",
                json!([address.to_string(), {"commitment":"finalized","encoding":"base64"}]),
            )
            .await?;
        let value = result.get("value").ok_or_else(|| {
            ApiError::Unavailable("Solana account response is missing value".into())
        })?;
        if value.is_null() {
            return Ok(None);
        }
        let owner = value
            .get("owner")
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::Unavailable("Solana account owner is missing".into()))?;
        if owner != self.lastro_program_id {
            return Err(ApiError::Conflict(
                "canonical account is not owned by the configured Lastro program".into(),
            ));
        }
        if value.get("executable").and_then(Value::as_bool) != Some(false) {
            return Err(ApiError::Conflict(
                "canonical Lastro state account must not be executable".into(),
            ));
        }
        let data = value.get("data").and_then(Value::as_array).ok_or_else(|| {
            ApiError::Unavailable("Solana account data is not base64 encoded".into())
        })?;
        if data.len() != 2 || data[1].as_str() != Some("base64") {
            return Err(ApiError::Unavailable(
                "Solana account data encoding is not base64".into(),
            ));
        }
        let encoded = data[0].as_str().ok_or_else(|| {
            ApiError::Unavailable("Solana account data payload is invalid".into())
        })?;
        let decoded = BASE64_STANDARD.decode(encoded).map_err(|_| {
            ApiError::Unavailable("Solana account data contains invalid base64".into())
        })?;
        if BASE64_STANDARD.encode(&decoded) != encoded {
            return Err(ApiError::Unavailable(
                "Solana account data base64 is not canonical".into(),
            ));
        }
        Ok(Some(decoded))
    }
}

#[async_trait]
impl SolanaRpc for HttpSolanaRpc {
    async fn v2_protocol_config(
        &self,
        deployment_id: [u8; 32],
    ) -> Result<Option<CanonicalV2ProtocolConfig>, ApiError> {
        let program_id = parse_program_id(&self.lastro_program_id)?;
        let (address, expected_bump) = v2_protocol_config_address(&program_id, &deployment_id);
        let Some(data) = self.account_data(&address).await? else {
            return Ok(None);
        };
        decode_v2_protocol_config(&data, deployment_id, expected_bump).map(Some)
    }

    async fn v2_event_anchor(
        &self,
        deployment_id: [u8; 32],
        event_id: [u8; 32],
    ) -> Result<Option<CanonicalV2EventAnchor>, ApiError> {
        let program_id = parse_program_id(&self.lastro_program_id)?;
        let (address, expected_bump) =
            v2_event_anchor_address(&program_id, &deployment_id, &event_id);
        let Some(data) = self.account_data(&address).await? else {
            return Ok(None);
        };
        decode_v2_event_anchor(&data, deployment_id, event_id, expected_bump).map(Some)
    }

    async fn v2_station(
        &self,
        deployment_id: [u8; 32],
        station_id: [u8; 32],
    ) -> Result<Option<CanonicalV2Station>, ApiError> {
        let program_id = parse_program_id(&self.lastro_program_id)?;
        let (address, expected_bump) = v2_station_address(&program_id, &deployment_id, &station_id);
        let Some(data) = self.account_data(&address).await? else {
            return Ok(None);
        };
        decode_v2_station(&data, station_id, expected_bump).map(Some)
    }

    async fn v2_event_anchor_signature(
        &self,
        deployment_id: [u8; 32],
        event_id: [u8; 32],
    ) -> Result<Option<String>, ApiError> {
        let program_id = parse_program_id(&self.lastro_program_id)?;
        let (address, _) = v2_event_anchor_address(&program_id, &deployment_id, &event_id);
        // Only the creating transaction references the anchor PDA; the list is newest first.
        let result = self
            .call(
                "getSignaturesForAddress",
                json!([address.to_string(), {"commitment":"finalized","limit":20}]),
            )
            .await?;
        let entries = result.as_array().ok_or_else(|| {
            ApiError::Unavailable("getSignaturesForAddress returned an invalid shape".into())
        })?;
        Ok(entries
            .iter()
            .rev()
            .find(|entry| entry.get("err").is_some_and(Value::is_null))
            .and_then(|entry| entry.get("signature").and_then(Value::as_str))
            .map(str::to_owned))
    }

    async fn v2_asset_state(
        &self,
        deployment_id: [u8; 32],
        asset_id: [u8; 32],
    ) -> Result<Option<CanonicalV2AssetState>, ApiError> {
        let program_id = parse_program_id(&self.lastro_program_id)?;
        let (address, expected_bump) = v2_asset_address(&program_id, &deployment_id, &asset_id);
        let Some(data) = self.account_data(&address).await? else {
            return Ok(None);
        };
        decode_v2_asset_state(&data, deployment_id, asset_id, expected_bump).map(Some)
    }

    async fn v2_rfid_binding(
        &self,
        deployment_id: [u8; 32],
        rfid_hash: [u8; 32],
    ) -> Result<Option<CanonicalV2RfidBinding>, ApiError> {
        let program_id = parse_program_id(&self.lastro_program_id)?;
        let (address, expected_bump) =
            v2_rfid_binding_address(&program_id, &deployment_id, &rfid_hash);
        let Some(data) = self.account_data(&address).await? else {
            return Ok(None);
        };
        decode_v2_rfid_binding(&data, rfid_hash, expected_bump).map(Some)
    }

    async fn v2_transformation(
        &self,
        deployment_id: [u8; 32],
        transformation_id: [u8; 32],
    ) -> Result<Option<CanonicalV2Transformation>, ApiError> {
        let program_id = parse_program_id(&self.lastro_program_id)?;
        let (address, expected_bump) =
            v2_transformation_address(&program_id, &deployment_id, &transformation_id);
        let Some(data) = self.account_data(&address).await? else {
            return Ok(None);
        };
        decode_v2_transformation(&data, transformation_id, expected_bump).map(Some)
    }

    async fn transaction_matches_confirmed(
        &self,
        signature: &str,
        expected: &TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        self.transaction_matches_at_commitment(signature, expected, "confirmed")
            .await
    }

    async fn transaction_matches(
        &self,
        signature: &str,
        expected: &TransactionDataResponse,
    ) -> Result<bool, ApiError> {
        let result = self
            .call(
                "getTransaction",
                json!([signature, {"commitment":"finalized","encoding":"json","maxSupportedTransactionVersion":0}]),
            )
            .await?;
        // Not finalized *yet* is transient: callers (and the reconciler) retry instead of
        // treating a pending transaction as a mismatch and quarantining it.
        if result.is_null() {
            return Err(ApiError::Unavailable(
                "Solana transaction is not finalized yet".into(),
            ));
        }
        transaction_matches_json(&result, signature, expected)
    }
}

fn anchor_discriminator(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("account:{name}").as_bytes());
    digest[..8]
        .try_into()
        .expect("SHA-256 output always contains eight bytes")
}

fn expect_layout<'a>(data: &'a [u8], account: &str, len: usize) -> Result<&'a [u8], ApiError> {
    if data.len() != len || data[..8] != anchor_discriminator(account) {
        return Err(ApiError::Conflict(format!(
            "canonical {account} account has an invalid layout"
        )));
    }
    Ok(data)
}

/// RfidBinding (v2): discriminator, asset_id, rfid_hash, status, bump = 74 bytes.
fn decode_v2_rfid_binding(
    data: &[u8],
    expected_rfid_hash: [u8; 32],
    expected_bump: u8,
) -> Result<CanonicalV2RfidBinding, ApiError> {
    let data = expect_layout(data, "RfidBinding", 74)?;
    let rfid_hash: [u8; 32] = data[40..72].try_into().map_err(|_| ApiError::Internal)?;
    if rfid_hash != expected_rfid_hash || data[73] != expected_bump {
        return Err(ApiError::Conflict(
            "canonical RfidBinding does not match its PDA seeds".into(),
        ));
    }
    let status = match data[72] {
        1 => BindingStatus::Active,
        2 => BindingStatus::Retired,
        _ => {
            return Err(ApiError::Conflict(
                "canonical RfidBinding has an invalid status".into(),
            ));
        }
    };
    Ok(CanonicalV2RfidBinding {
        asset_id: data[8..40].try_into().map_err(|_| ApiError::Internal)?,
        rfid_hash,
        status,
    })
}

fn decode_v2_protocol_config(
    data: &[u8],
    deployment_id: [u8; 32],
    expected_bump: u8,
) -> Result<CanonicalV2ProtocolConfig, ApiError> {
    let data = expect_layout(data, "ProtocolConfigV2", 221)?;
    let stored_deployment: [u8; 32] = data[40..72].try_into().map_err(|_| ApiError::Internal)?;
    if stored_deployment != deployment_id || data[220] != expected_bump {
        return Err(ApiError::Conflict(
            "canonical v2 ProtocolConfig does not match its PDA seeds".into(),
        ));
    }
    Ok(CanonicalV2ProtocolConfig {
        authority: Pubkey::new_from_array(data[8..40].try_into().map_err(|_| ApiError::Internal)?),
        station_registry: Pubkey::new_from_array(
            data[74..106].try_into().map_err(|_| ApiError::Internal)?,
        ),
        max_event_age_seconds: u64::from_le_bytes(
            data[212..220].try_into().map_err(|_| ApiError::Internal)?,
        ),
    })
}

/// StationRecord (v2): discriminator, station_id, key_id, pubkey33, status, valid_from,
/// valid_until, firmware_hash, bump = 155 bytes.
fn decode_v2_station(
    data: &[u8],
    station_id: [u8; 32],
    expected_bump: u8,
) -> Result<CanonicalV2Station, ApiError> {
    let data = expect_layout(data, "StationRecord", 155)?;
    let stored: [u8; 32] = data[8..40].try_into().map_err(|_| ApiError::Internal)?;
    if stored != station_id || data[154] != expected_bump {
        return Err(ApiError::Conflict(
            "canonical v2 StationRecord does not match its PDA seeds".into(),
        ));
    }
    Ok(CanonicalV2Station {
        station_id: stored,
        pubkey33: data[72..105].try_into().map_err(|_| ApiError::Internal)?,
        status: data[105],
        valid_from: i64::from_le_bytes(data[106..114].try_into().map_err(|_| ApiError::Internal)?),
        valid_until: i64::from_le_bytes(data[114..122].try_into().map_err(|_| ApiError::Internal)?),
    })
}

fn decode_v2_event_anchor(
    data: &[u8],
    deployment_id: [u8; 32],
    event_id: [u8; 32],
    expected_bump: u8,
) -> Result<CanonicalV2EventAnchor, ApiError> {
    let data = expect_layout(data, "EventAnchor", 259)?;
    let stored_event_id: [u8; 32] = data[8..40].try_into().map_err(|_| ApiError::Internal)?;
    let stored_deployment: [u8; 32] = data[40..72].try_into().map_err(|_| ApiError::Internal)?;
    if stored_event_id != event_id
        || stored_deployment != deployment_id
        || data[258] != expected_bump
    {
        return Err(ApiError::Conflict(
            "canonical v2 EventAnchor does not match its PDA seeds".into(),
        ));
    }
    Ok(CanonicalV2EventAnchor {
        event_id: stored_event_id,
        deployment_id: stored_deployment,
        subject_id: data[72..104].try_into().map_err(|_| ApiError::Internal)?,
        source_id: data[104..136].try_into().map_err(|_| ApiError::Internal)?,
        event_type: u16::from_le_bytes(data[136..138].try_into().map_err(|_| ApiError::Internal)?),
        state_version: u64::from_le_bytes(
            data[138..146].try_into().map_err(|_| ApiError::Internal)?,
        ),
        observed_at: i64::from_le_bytes(data[146..154].try_into().map_err(|_| ApiError::Internal)?),
        expires_at: i64::from_le_bytes(data[154..162].try_into().map_err(|_| ApiError::Internal)?),
        expected_previous_hash: data[162..194].try_into().map_err(|_| ApiError::Internal)?,
        payload_hash: data[194..226].try_into().map_err(|_| ApiError::Internal)?,
        event_hash: data[226..258].try_into().map_err(|_| ApiError::Internal)?,
    })
}

fn decode_v2_asset_state(
    data: &[u8],
    deployment_id: [u8; 32],
    asset_id: [u8; 32],
    expected_bump: u8,
) -> Result<CanonicalV2AssetState, ApiError> {
    // 341 bytes: ... reserved_by (266..298), reserved_until, flags (306..308),
    // current_rfid_hash (308..340), bump (340).
    let data = expect_layout(data, "AssetState", 341)?;
    let stored_asset_id: [u8; 32] = data[8..40].try_into().map_err(|_| ApiError::Internal)?;
    let stored_deployment: [u8; 32] = data[42..74].try_into().map_err(|_| ApiError::Internal)?;
    if stored_asset_id != asset_id
        || stored_deployment != deployment_id
        || data[340] != expected_bump
    {
        return Err(ApiError::Conflict(
            "canonical v2 AssetState does not match its PDA seeds".into(),
        ));
    }
    Ok(CanonicalV2AssetState {
        asset_id: stored_asset_id,
        asset_type: data[40],
        status: data[41],
        deployment_id: stored_deployment,
        custodian: Pubkey::new_from_array(
            data[74..106].try_into().map_err(|_| ApiError::Internal)?,
        ),
        parent_root: data[106..138].try_into().map_err(|_| ApiError::Internal)?,
        lineage_root: data[138..170].try_into().map_err(|_| ApiError::Internal)?,
        current_lot_id: data[170..202].try_into().map_err(|_| ApiError::Internal)?,
        available_weight_grams: u64::from_le_bytes(
            data[202..210].try_into().map_err(|_| ApiError::Internal)?,
        ),
        event_sequence: u64::from_le_bytes(
            data[218..226].try_into().map_err(|_| ApiError::Internal)?,
        ),
        state_version: u64::from_le_bytes(
            data[226..234].try_into().map_err(|_| ApiError::Internal)?,
        ),
        last_event_hash: data[234..266].try_into().map_err(|_| ApiError::Internal)?,
        current_rfid_hash: data[308..340].try_into().map_err(|_| ApiError::Internal)?,
    })
}

/// TransformationAnchor is 274 bytes: discriminator, ids, roots, five u32 counters, eight u64
/// weights, tolerance, status (224), manifest hash (225..257), sequence, expires_at, bump (273).
fn decode_v2_transformation(
    data: &[u8],
    transformation_id: [u8; 32],
    expected_bump: u8,
) -> Result<CanonicalV2Transformation, ApiError> {
    let data = expect_layout(data, "TransformationAnchor", 274)?;
    let stored_id: [u8; 32] = data[8..40].try_into().map_err(|_| ApiError::Internal)?;
    if stored_id != transformation_id || data[273] != expected_bump {
        return Err(ApiError::Conflict(
            "canonical v2 TransformationAnchor does not match its PDA seeds".into(),
        ));
    }
    Ok(CanonicalV2Transformation {
        transformation_id: stored_id,
        facility_id: data[40..72].try_into().map_err(|_| ApiError::Internal)?,
        input_root: data[74..106].try_into().map_err(|_| ApiError::Internal)?,
        output_root: data[106..138].try_into().map_err(|_| ApiError::Internal)?,
        status: data[224],
        manifest_hash: data[225..257].try_into().map_err(|_| ApiError::Internal)?,
        sequence: u64::from_le_bytes(data[257..265].try_into().map_err(|_| ApiError::Internal)?),
    })
}

fn transaction_matches_json(
    result: &Value,
    requested_signature: &str,
    expected: &TransactionDataResponse,
) -> Result<bool, ApiError> {
    if expected.transaction_version != TransactionVersionDto::Legacy {
        return Ok(false);
    }
    if result
        .get("version")
        .is_some_and(|version| !version.is_null() && version.as_str() != Some("legacy"))
    {
        return Ok(false);
    }
    let meta = result
        .get("meta")
        .and_then(Value::as_object)
        .ok_or_else(|| ApiError::Unavailable("Solana transaction is missing metadata".into()))?;
    if meta.get("err").is_none_or(|err| !err.is_null()) {
        return Ok(false);
    }

    let transaction = result
        .get("transaction")
        .and_then(Value::as_object)
        .ok_or_else(|| ApiError::Unavailable("Solana transaction payload is invalid".into()))?;
    let signatures = transaction
        .get("signatures")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::Unavailable("Solana transaction signatures are invalid".into()))?;
    if signatures.first().and_then(Value::as_str) != Some(requested_signature) {
        return Ok(false);
    }
    let message = transaction
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| ApiError::Unavailable("Solana transaction message is invalid".into()))?;
    if message
        .get("addressTableLookups")
        .is_some_and(|value| value.as_array().is_none_or(|lookups| !lookups.is_empty()))
    {
        return Ok(false);
    }
    let account_keys = message
        .get("accountKeys")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ApiError::Unavailable("Solana transaction account keys are invalid".into())
        })?;
    let account_keys: Vec<&str> = account_keys
        .iter()
        .map(|value| value.as_str())
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            ApiError::Unavailable("Solana transaction account keys must be strings".into())
        })?;
    let header = message
        .get("header")
        .and_then(Value::as_object)
        .ok_or_else(|| ApiError::Unavailable("Solana transaction header is invalid".into()))?;
    let required_signatures = usize_field(header.get("numRequiredSignatures"))?;
    if signatures.len() != required_signatures {
        return Ok(false);
    }
    let readonly_signed = usize_field(header.get("numReadonlySignedAccounts"))?;
    let readonly_unsigned = usize_field(header.get("numReadonlyUnsignedAccounts"))?;
    if required_signatures != 1
        || readonly_signed > required_signatures
        || account_keys.first().copied() != Some(expected.required_signer.as_str())
    {
        return Ok(false);
    }
    if readonly_unsigned > account_keys.len().saturating_sub(required_signatures) {
        return Ok(false);
    }

    let instructions = message
        .get("instructions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ApiError::Unavailable("Solana transaction instructions are invalid".into())
        })?;
    // Protocol instructions must match exactly. Up to two ComputeBudget instructions may
    // follow them (priority fees); the programs reject anything else after index 1, and
    // they change no Lastro state, so their values need not match the prepared ones.
    let protocol: Vec<&InstructionDto> = expected
        .instructions
        .iter()
        .filter(|instruction| instruction.program_id != COMPUTE_BUDGET_PROGRAM_ID)
        .collect();
    let trailing = instructions.len().saturating_sub(protocol.len());
    if instructions.len() < protocol.len() || trailing > 2 {
        return Ok(false);
    }

    let mut expected_roles: HashMap<&str, (bool, bool)> = HashMap::new();
    expected_roles.insert(expected.required_signer.as_str(), (true, true));
    if trailing > 0 {
        expected_roles.insert(COMPUTE_BUDGET_PROGRAM_ID, (false, false));
    }
    for instruction in &protocol {
        expected_roles
            .entry(instruction.program_id.as_str())
            .or_insert((false, false));
        for account in &instruction.accounts {
            let role = expected_roles
                .entry(account.address.as_str())
                .or_insert((false, false));
            role.0 |= account.is_signer;
            role.1 |= account.is_writable;
        }
    }
    if expected_roles.len() != account_keys.len() {
        return Ok(false);
    }
    for (index, address) in account_keys.iter().enumerate() {
        let signer = index < required_signatures;
        let writable = if signer {
            index < required_signatures.saturating_sub(readonly_signed)
        } else {
            index < account_keys.len().saturating_sub(readonly_unsigned)
        };
        if expected_roles.get(*address).copied() != Some((signer, writable)) {
            return Ok(false);
        }
    }

    for actual in &instructions[protocol.len()..] {
        let actual = actual.as_object().ok_or_else(|| {
            ApiError::Unavailable("Solana compiled instruction is invalid".into())
        })?;
        let program_index = usize_field(actual.get("programIdIndex"))?;
        let no_accounts = actual
            .get("accounts")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty);
        if account_keys.get(program_index).copied() != Some(COMPUTE_BUDGET_PROGRAM_ID)
            || !no_accounts
        {
            return Ok(false);
        }
    }
    for (actual, expected_ix) in instructions.iter().zip(protocol) {
        let actual = actual.as_object().ok_or_else(|| {
            ApiError::Unavailable("Solana compiled instruction is invalid".into())
        })?;
        let program_index = usize_field(actual.get("programIdIndex"))?;
        if account_keys.get(program_index).copied() != Some(expected_ix.program_id.as_str()) {
            return Ok(false);
        }
        let actual_accounts = actual
            .get("accounts")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ApiError::Unavailable("Solana compiled instruction accounts are invalid".into())
            })?;
        if actual_accounts.len() != expected_ix.accounts.len() {
            return Ok(false);
        }
        for (actual_index, expected_account) in actual_accounts.iter().zip(&expected_ix.accounts) {
            let index = usize_field(Some(actual_index))?;
            if account_keys.get(index).copied() != Some(expected_account.address.as_str()) {
                return Ok(false);
            }
        }
        let actual_data = actual.get("data").and_then(Value::as_str).ok_or_else(|| {
            ApiError::Unavailable("Solana compiled instruction data is invalid".into())
        })?;
        let actual_data = bs58::decode(actual_data).into_vec().map_err(|_| {
            ApiError::Unavailable("Solana compiled instruction data is invalid base58".into())
        })?;
        let expected_data = BASE64_STANDARD
            .decode(&expected_ix.data_base64)
            .map_err(|_| ApiError::Internal)?;
        if actual_data != expected_data {
            return Ok(false);
        }
    }
    Ok(true)
}

fn usize_field(value: Option<&Value>) -> Result<usize, ApiError> {
    let value = value.and_then(Value::as_u64).ok_or_else(|| {
        ApiError::Unavailable("Solana transaction contains an invalid numeric field".into())
    })?;
    usize::try_from(value)
        .map_err(|_| ApiError::Unavailable("Solana transaction numeric field is too large".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_discriminator(name: &str, mut body: Vec<u8>) -> Vec<u8> {
        let mut data = anchor_discriminator(name).to_vec();
        data.append(&mut body);
        data
    }

    #[test]
    fn decodes_station_record_validity_window() {
        let mut body = vec![7u8; 32]; // station_id
        body.extend_from_slice(&[1; 32]); // key_id
        body.extend_from_slice(&[2; 33]); // pubkey33
        body.push(STATION_STATUS_ACTIVE);
        body.extend_from_slice(&10i64.to_le_bytes());
        body.extend_from_slice(&20i64.to_le_bytes());
        body.extend_from_slice(&[0; 32]); // firmware hash
        body.push(250); // bump
        let data = with_discriminator("StationRecord", body);
        let station = decode_v2_station(&data, [7; 32], 250).unwrap();
        assert_eq!(
            (station.status, station.valid_from, station.valid_until),
            (1, 10, 20)
        );
        assert_eq!(station.pubkey33, [2; 33]);
        assert!(decode_v2_station(&data, [8; 32], 250).is_err());
        assert!(decode_v2_station(&data[..154], [7; 32], 250).is_err());
    }

    #[test]
    fn decodes_exact_v2_account_layouts_and_rejects_seed_mismatch() {
        // PURPOSE: canonical v2 accounts are decoded only at their exact on-chain layout.
        // ASSERT: AssetState (341 B) exposes custodian/version/RFID; RfidBinding status is strict.
        // FAILURE MEANS: the projection could trust a foreign or truncated account.
        let asset_id = [0x11; 32];
        let deployment = [0xd0; 32];
        let custodian = [0x22; 32];
        let mut body = asset_id.to_vec();
        body.push(1); // asset_type
        body.push(1); // status
        body.extend_from_slice(&deployment);
        body.extend_from_slice(&custodian);
        body.extend_from_slice(&[0; 32 * 3]); // parent, lineage, lot
        for value in [500u64, 0, 3, 7] {
            body.extend_from_slice(&value.to_le_bytes()); // available, reserved, sequence, version
        }
        body.extend_from_slice(&[0x33; 32]); // last_event_hash
        body.extend_from_slice(&[0; 32]); // reserved_by
        body.extend_from_slice(&0i64.to_le_bytes());
        body.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&[0x44; 32]); // current_rfid_hash
        body.push(9); // bump
        let data = with_discriminator("AssetState", body);
        assert_eq!(data.len(), 341);
        let asset = decode_v2_asset_state(&data, deployment, asset_id, 9).unwrap();
        assert_eq!(asset.custodian.to_bytes(), custodian);
        assert_eq!(asset.state_version, 7);
        assert_eq!(asset.current_rfid_hash, [0x44; 32]);
        assert!(decode_v2_asset_state(&data, deployment, asset_id, 8).is_err());
        assert!(decode_v2_asset_state(&data[..340], deployment, asset_id, 9).is_err());

        let rfid = [0x77; 32];
        let mut binding = asset_id.to_vec();
        binding.extend_from_slice(&rfid);
        binding.push(2);
        binding.push(6);
        let decoded =
            decode_v2_rfid_binding(&with_discriminator("RfidBinding", binding.clone()), rfid, 6)
                .unwrap();
        assert_eq!(decoded.status, BindingStatus::Retired);
        assert_eq!(decoded.asset_id, asset_id);
        binding[64] = 9;
        assert!(
            decode_v2_rfid_binding(&with_discriminator("RfidBinding", binding), rfid, 6).is_err()
        );
    }

    fn expected_transaction() -> TransactionDataResponse {
        TransactionDataResponse {
            required_signer: "Signer111111111111111111111111111111111".into(),
            lastro_program_id: "Lastro111111111111111111111111111111111".into(),
            instructions: vec![
                crate::model::InstructionDto {
                    program_id: "Secp1111111111111111111111111111111111".into(),
                    accounts: vec![],
                    data_base64: BASE64_STANDARD.encode([1u8, 2]),
                },
                crate::model::InstructionDto {
                    program_id: "Lastro111111111111111111111111111111111".into(),
                    accounts: vec![
                        crate::model::AccountMetaDto {
                            address: "Signer111111111111111111111111111111111".into(),
                            is_signer: true,
                            is_writable: true,
                        },
                        crate::model::AccountMetaDto {
                            address: "Writable111111111111111111111111111111".into(),
                            is_signer: false,
                            is_writable: true,
                        },
                        crate::model::AccountMetaDto {
                            address: "Readonly111111111111111111111111111111".into(),
                            is_signer: false,
                            is_writable: false,
                        },
                    ],
                    data_base64: BASE64_STANDARD.encode([3u8, 4]),
                },
            ],
            measured_serialized_bytes: 500,
            transaction_version: TransactionVersionDto::Legacy,
        }
    }

    fn finalized_transaction_json(signature: &str) -> Value {
        json!({
            "slot": 42,
            "version": "legacy",
            "meta": { "err": null },
            "transaction": {
                "signatures": [signature],
                "message": {
                    "header": {
                        "numRequiredSignatures": 1,
                        "numReadonlySignedAccounts": 0,
                        "numReadonlyUnsignedAccounts": 3
                    },
                    "accountKeys": [
                        "Signer111111111111111111111111111111111",
                        "Writable111111111111111111111111111111",
                        "Secp1111111111111111111111111111111111",
                        "Lastro111111111111111111111111111111111",
                        "Readonly111111111111111111111111111111"
                    ],
                    "recentBlockhash": "ignored-by-matcher",
                    "instructions": [
                        {
                            "programIdIndex": 2,
                            "accounts": [],
                            "data": bs58::encode([1u8, 2]).into_string()
                        },
                        {
                            "programIdIndex": 3,
                            "accounts": [0, 1, 4],
                            "data": bs58::encode([3u8, 4]).into_string()
                        }
                    ]
                }
            }
        })
    }

    #[test]
    fn finalized_transaction_must_match_exact_compiled_envelope() {
        let signature = "fixture-signature";
        let expected = expected_transaction();
        let valid = finalized_transaction_json(signature);
        assert!(transaction_matches_json(&valid, signature, &expected).unwrap());

        let mut changed_data = valid.clone();
        changed_data["transaction"]["message"]["instructions"][1]["data"] =
            Value::String(bs58::encode([3u8, 5]).into_string());
        assert!(!transaction_matches_json(&changed_data, signature, &expected).unwrap());

        let mut changed_role = valid.clone();
        changed_role["transaction"]["message"]["header"]["numReadonlyUnsignedAccounts"] = json!(4);
        assert!(!transaction_matches_json(&changed_role, signature, &expected).unwrap());

        let mut extra_instruction = valid.clone();
        extra_instruction["transaction"]["message"]["instructions"]
            .as_array_mut()
            .unwrap()
            .push(json!({"programIdIndex": 2, "accounts": [], "data": ""}));
        assert!(!transaction_matches_json(&extra_instruction, signature, &expected).unwrap());
    }

    #[test]
    fn finalized_transaction_tolerates_only_trailing_compute_budget() {
        // PURPOSE: priority fees may be paid without weakening the exact protocol envelope match.
        // ASSERT: up to two account-less ComputeBudget instructions after the protocol pass;
        // accounts on them, a third one, or any other trailing program fail.
        // FAILURE MEANS: prices would break confirmation, or extra instructions could slip in.
        let signature = "fixture-signature";
        let expected = expected_transaction();
        let mut priced = finalized_transaction_json(signature);
        priced["transaction"]["message"]["header"]["numReadonlyUnsignedAccounts"] = json!(4);
        priced["transaction"]["message"]["accountKeys"]
            .as_array_mut()
            .unwrap()
            .push(json!(COMPUTE_BUDGET_PROGRAM_ID));
        for data in [[2u8, 0, 0, 0, 0], [3u8, 1, 0, 0, 0]] {
            priced["transaction"]["message"]["instructions"]
                .as_array_mut()
                .unwrap()
                .push(json!({"programIdIndex": 5, "accounts": [], "data": bs58::encode(data).into_string()}));
        }
        assert!(transaction_matches_json(&priced, signature, &expected).unwrap());

        let mut with_accounts = priced.clone();
        with_accounts["transaction"]["message"]["instructions"][2]["accounts"] = json!([1]);
        assert!(!transaction_matches_json(&with_accounts, signature, &expected).unwrap());

        let mut three = priced.clone();
        three["transaction"]["message"]["instructions"]
            .as_array_mut()
            .unwrap()
            .push(json!({"programIdIndex": 5, "accounts": [], "data": ""}));
        assert!(!transaction_matches_json(&three, signature, &expected).unwrap());
    }

    #[test]
    fn finalized_transaction_rejects_wrong_program_id() {
        // PURPOSE: Bind confirmation to the configured Lastro program, not merely matching instruction bytes.
        // ASSERT: Replacing the Lastro program account with another address makes the finalized envelope mismatch.
        // FAILURE MEANS: A transaction executed by an unrelated program could authorize a Lastro projection update.
        let signature = "fixture-signature";
        let expected = expected_transaction();
        let mut wrong_program = finalized_transaction_json(signature);
        wrong_program["transaction"]["message"]["accountKeys"][3] =
            Value::String("Other1111111111111111111111111111111111".into());
        assert!(!transaction_matches_json(&wrong_program, signature, &expected).unwrap());
    }

    #[test]
    fn finalized_transaction_rejects_substituted_account_address() {
        let signature = "fixture-signature";
        let expected = expected_transaction();
        let mut wrong_account = finalized_transaction_json(signature);
        wrong_account["transaction"]["message"]["accountKeys"][1] =
            Value::String("OtherWritable111111111111111111111111111".into());
        assert!(!transaction_matches_json(&wrong_account, signature, &expected).unwrap());
    }

    #[test]
    fn finalized_transaction_rejects_failed_execution() {
        let signature = "fixture-signature";
        let expected = expected_transaction();
        let mut failed = finalized_transaction_json(signature);
        failed["meta"]["err"] = json!({"InstructionError": [1, {"Custom": 6000}]});
        assert!(!transaction_matches_json(&failed, signature, &expected).unwrap());
    }

    #[test]
    fn finalized_transaction_treats_fee_payer_as_globally_writable_even_when_instruction_meta_is_readonly()
     {
        // PURPOSE: Match Solana message privileges rather than confusing them with one instruction's account meta.
        // ASSERT: A TRANSFER-style read-only custodian signer still matches when the same address is the writable fee payer.
        // FAILURE MEANS: Every valid TRANSFER finalized transaction would be rejected during API confirmation.
        let signature = "fixture-signature";
        let mut expected = expected_transaction();
        expected.instructions[1].accounts[0].is_writable = false;
        let valid = finalized_transaction_json(signature);
        assert!(transaction_matches_json(&valid, signature, &expected).unwrap());
    }

    #[test]
    fn finalized_transaction_requires_exact_requested_signature_and_signature_count() {
        let expected = expected_transaction();
        let valid = finalized_transaction_json("fixture-signature");
        assert!(!transaction_matches_json(&valid, "different-signature", &expected).unwrap());

        let mut extra_signature = valid.clone();
        extra_signature["transaction"]["signatures"] =
            json!(["fixture-signature", "unexpected-second-signature"]);
        assert!(
            !transaction_matches_json(&extra_signature, "fixture-signature", &expected).unwrap()
        );
    }
}
