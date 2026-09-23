//! Read-only canonical Solana RPC boundary used by the API.
//!
//! The backend is a projection/coordinator. Confirmation must query finalized RPC
//! state and prove the submitted transaction contains the exact Lastro envelope
//! before PostgreSQL current state changes.

use std::{collections::HashMap, time::Duration};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use lastro_protocol::{
    crypto::derive_station_id,
    ids::{AnimalId, DeploymentId, EventHash, RfidHash},
};
use reqwest::Client;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

const SOLANA_RPC_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

use crate::{
    error::ApiError,
    model::{TransactionDataResponse, TransactionVersionDto},
    solana::transaction_builder::{
        animal_state_address, parse_program_id, protocol_config_address, rfid_binding_address,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalAnimalState {
    pub animal_id: AnimalId,
    pub current_rfid_hash: RfidHash,
    pub current_custodian: [u8; 32],
    pub identity_revision: u32,
    pub event_sequence: u64,
    pub last_event_hash: EventHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingStatus {
    Active,
    Retired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalRfidBinding {
    pub animal_id: AnimalId,
    pub rfid_hash: RfidHash,
    pub status: BindingStatus,
}

#[async_trait]
pub trait SolanaRpc: Send + Sync {
    async fn protocol_station_pubkey(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<[u8; 33], ApiError>;
    async fn animal_state(
        &self,
        deployment_id: DeploymentId,
        animal_id: AnimalId,
    ) -> Result<Option<CanonicalAnimalState>, ApiError>;
    async fn rfid_binding(
        &self,
        deployment_id: DeploymentId,
        rfid_hash: RfidHash,
    ) -> Result<Option<CanonicalRfidBinding>, ApiError>;
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
    async fn protocol_station_pubkey(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<[u8; 33], ApiError> {
        let program_id = parse_program_id(&self.lastro_program_id)?;
        let (address, expected_bump) = protocol_config_address(&program_id, &deployment_id);
        let data = self.account_data(&address).await?.ok_or_else(|| {
            ApiError::NotFound("canonical ProtocolConfig account not found".into())
        })?;
        decode_protocol_config(&data, deployment_id, expected_bump)
    }

    async fn animal_state(
        &self,
        deployment_id: DeploymentId,
        animal_id: AnimalId,
    ) -> Result<Option<CanonicalAnimalState>, ApiError> {
        let program_id = parse_program_id(&self.lastro_program_id)?;
        let (address, expected_bump) =
            animal_state_address(&program_id, &deployment_id, &animal_id);
        let Some(data) = self.account_data(&address).await? else {
            return Ok(None);
        };
        decode_animal_state(&data, animal_id, expected_bump).map(Some)
    }

    async fn rfid_binding(
        &self,
        deployment_id: DeploymentId,
        rfid_hash: RfidHash,
    ) -> Result<Option<CanonicalRfidBinding>, ApiError> {
        let program_id = parse_program_id(&self.lastro_program_id)?;
        let (address, expected_bump) =
            rfid_binding_address(&program_id, &deployment_id, &rfid_hash);
        let Some(data) = self.account_data(&address).await? else {
            return Ok(None);
        };
        decode_rfid_binding(&data, rfid_hash, expected_bump).map(Some)
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
        self.transaction_matches_at_commitment(signature, expected, "finalized")
            .await
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

fn decode_protocol_config(
    data: &[u8],
    deployment_id: DeploymentId,
    expected_bump: u8,
) -> Result<[u8; 33], ApiError> {
    let data = expect_layout(data, "ProtocolConfig", 106)?;
    let stored_deployment: [u8; 32] = data[40..72].try_into().map_err(|_| ApiError::Internal)?;
    if stored_deployment != deployment_id || data[105] != expected_bump {
        return Err(ApiError::Conflict(
            "canonical ProtocolConfig does not match its PDA seeds".into(),
        ));
    }
    let station_key: [u8; 33] = data[72..105].try_into().map_err(|_| ApiError::Internal)?;
    derive_station_id(&station_key).map_err(|_| {
        ApiError::Conflict("canonical ProtocolConfig contains an invalid Station public key".into())
    })?;
    Ok(station_key)
}

fn decode_animal_state(
    data: &[u8],
    expected_animal_id: AnimalId,
    expected_bump: u8,
) -> Result<CanonicalAnimalState, ApiError> {
    let data = expect_layout(data, "AnimalState", 149)?;
    let animal_id: [u8; 32] = data[8..40].try_into().map_err(|_| ApiError::Internal)?;
    if animal_id != expected_animal_id || data[148] != expected_bump {
        return Err(ApiError::Conflict(
            "canonical AnimalState does not match its PDA seeds".into(),
        ));
    }
    Ok(CanonicalAnimalState {
        animal_id,
        current_rfid_hash: data[40..72].try_into().map_err(|_| ApiError::Internal)?,
        current_custodian: data[72..104].try_into().map_err(|_| ApiError::Internal)?,
        identity_revision: u32::from_le_bytes(
            data[104..108].try_into().map_err(|_| ApiError::Internal)?,
        ),
        event_sequence: u64::from_le_bytes(
            data[108..116].try_into().map_err(|_| ApiError::Internal)?,
        ),
        last_event_hash: data[116..148].try_into().map_err(|_| ApiError::Internal)?,
    })
}

fn decode_rfid_binding(
    data: &[u8],
    expected_rfid_hash: RfidHash,
    expected_bump: u8,
) -> Result<CanonicalRfidBinding, ApiError> {
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
                "canonical RfidBinding contains an invalid status".into(),
            ));
        }
    };
    Ok(CanonicalRfidBinding {
        animal_id: data[8..40].try_into().map_err(|_| ApiError::Internal)?,
        rfid_hash,
        status,
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

    let mut expected_roles: HashMap<&str, (bool, bool)> = HashMap::new();
    expected_roles.insert(expected.required_signer.as_str(), (true, true));
    for instruction in &expected.instructions {
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

    let instructions = message
        .get("instructions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ApiError::Unavailable("Solana transaction instructions are invalid".into())
        })?;
    if instructions.len() != expected.instructions.len() {
        return Ok(false);
    }
    for (actual, expected_ix) in instructions.iter().zip(&expected.instructions) {
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
    fn decodes_exact_account_layouts_and_rejects_seed_mismatch() {
        let deployment = [0x11; 32];
        let station =
            hex::decode("036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296")
                .unwrap();
        let mut config_body = vec![0x22; 32];
        config_body.extend_from_slice(&deployment);
        config_body.extend_from_slice(&station);
        config_body.push(7);
        let config = with_discriminator("ProtocolConfig", config_body);
        assert_eq!(
            decode_protocol_config(&config, deployment, 7)
                .unwrap()
                .as_slice(),
            station.as_slice()
        );
        assert!(decode_protocol_config(&config, [0x12; 32], 7).is_err());

        let animal_id = [0x33; 32];
        let mut animal_body = animal_id.to_vec();
        animal_body.extend_from_slice(&[0x44; 32]);
        animal_body.extend_from_slice(&[0x55; 32]);
        animal_body.extend_from_slice(&9u32.to_le_bytes());
        animal_body.extend_from_slice(&10u64.to_le_bytes());
        animal_body.extend_from_slice(&[0x66; 32]);
        animal_body.push(8);
        let animal = decode_animal_state(
            &with_discriminator("AnimalState", animal_body),
            animal_id,
            8,
        )
        .unwrap();
        assert_eq!(animal.identity_revision, 9);
        assert_eq!(animal.event_sequence, 10);

        let rfid = [0x77; 32];
        let mut binding_body = animal_id.to_vec();
        binding_body.extend_from_slice(&rfid);
        binding_body.push(1);
        binding_body.push(6);
        let binding =
            decode_rfid_binding(&with_discriminator("RfidBinding", binding_body), rfid, 6).unwrap();
        assert_eq!(binding.status, BindingStatus::Active);
    }

    #[test]
    fn account_layout_length_and_binding_status_are_strict() {
        assert!(decode_animal_state(&[0u8; 148], [0u8; 32], 0).is_err());
        let rfid = [0x77; 32];
        let mut body = vec![0x33; 32];
        body.extend_from_slice(&rfid);
        body.push(9);
        body.push(1);
        assert!(decode_rfid_binding(&with_discriminator("RfidBinding", body), rfid, 1).is_err());
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
