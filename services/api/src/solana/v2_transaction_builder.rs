//! Prepare the exact two-instruction v2 observation transaction.
//!
//! The first instruction is Solana's Secp256r1 precompile. The second is the
//! Lastro v2 `record_observation` instruction. The wallet remains the only
//! transaction signer; this module never handles private keys.

use std::{collections::HashSet, str::FromStr};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use lastro_protocol::crypto::derive_station_id;
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

use crate::{
    error::ApiError,
    model::{AccountMetaDto, InstructionDto, TransactionDataResponse, TransactionVersionDto},
    repository::domain_v2::DomainAnchorRecord,
    solana::secp256r1::{
        SECP256R1_PROGRAM_ID, Secp256r1Descriptor, build_secp256r1_instruction_data_for_message,
    },
};

const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const INSTRUCTIONS_SYSVAR_ID: &str = "Sysvar1nstructions1111111111111111111111111";
const MAX_LEGACY_TRANSACTION_BYTES: usize = 1232;
const CONFIG_V2_SEED: &[u8] = b"config-v2";
const STATION_REGISTRY_SEED: &[u8] = b"station-registry";
const ASSET_SEED: &[u8] = b"asset";
const EVENT_SEED: &[u8] = b"event";
const RECORD_OBSERVATION_ARGUMENT_PREFIX_LEN: usize = 32 + 32 + 32 + 8;
const RECORD_OBSERVATION_DATA_LEN: usize = RECORD_OBSERVATION_ARGUMENT_PREFIX_LEN + 220;

pub const LASTRO_SECP256R1_MESSAGE_LEN: u16 = 220;

pub fn build_transaction_data(
    lastro_program_id: &str,
    record: &DomainAnchorRecord,
    authority: Pubkey,
    station_registry: Pubkey,
) -> Result<TransactionDataResponse, ApiError> {
    let program_id = parse_program_id(lastro_program_id)?;
    let station_id = derive_station_id(&record.station_pubkey33).map_err(|_| {
        ApiError::Conflict("v2 Station public key cannot derive a StationId".into())
    })?;
    if record.envelope.source_id != station_id {
        return Err(ApiError::Conflict(
            "v2 event sourceId does not match its Station public key".into(),
        ));
    }

    let lastro_data = build_record_observation_data(record, station_id);
    let event_offset = unique_event_offset(&lastro_data, &record.envelope_bytes)?;
    if event_offset as usize != RECORD_OBSERVATION_ARGUMENT_PREFIX_LEN {
        return Err(ApiError::Internal);
    }
    let secp_data = build_secp256r1_instruction_data_for_message(
        Secp256r1Descriptor {
            station_pubkey33: &record.station_pubkey33,
            station_signature64: &record.station_signature64,
            message_data_offset: event_offset,
        },
        LASTRO_SECP256R1_MESSAGE_LEN,
    )?;

    let (config, _) = v2_protocol_config_address(&program_id, &record.envelope.deployment_id);
    let (asset, _) = v2_asset_address(
        &program_id,
        &record.envelope.deployment_id,
        &record.envelope.subject_id,
    );
    let (event_anchor, _) = v2_event_anchor_address(
        &program_id,
        &record.envelope.deployment_id,
        &record.envelope.event_id,
    );
    let accounts = vec![
        meta(authority.to_string(), true, false),
        meta(config.to_string(), false, false),
        meta(station_registry.to_string(), false, false),
        meta(
            v2_station_address(&program_id, &record.envelope.deployment_id, &station_id)
                .0
                .to_string(),
            false,
            false,
        ),
        meta(asset.to_string(), false, true),
        meta(event_anchor.to_string(), false, true),
        meta(INSTRUCTIONS_SYSVAR_ID.into(), false, false),
        meta(SYSTEM_PROGRAM_ID.into(), false, false),
    ];
    let instructions = vec![
        InstructionDto {
            program_id: SECP256R1_PROGRAM_ID.to_owned(),
            accounts: Vec::new(),
            data_base64: BASE64_STANDARD.encode(secp_data),
        },
        InstructionDto {
            program_id: program_id.to_string(),
            accounts,
            data_base64: BASE64_STANDARD.encode(lastro_data),
        },
    ];
    let measured_serialized_bytes = measure_legacy_transaction(&instructions, &authority);
    if measured_serialized_bytes > MAX_LEGACY_TRANSACTION_BYTES {
        return Err(ApiError::Conflict(format!(
            "v2 observation transaction requires {measured_serialized_bytes} bytes, exceeding the 1232-byte Solana limit"
        )));
    }

    Ok(TransactionDataResponse {
        required_signer: authority.to_string(),
        lastro_program_id: program_id.to_string(),
        instructions,
        measured_serialized_bytes,
        transaction_version: TransactionVersionDto::Legacy,
    })
}

pub fn parse_program_id(value: &str) -> Result<Pubkey, ApiError> {
    Pubkey::from_str(value)
        .map_err(|_| ApiError::Config("LASTRO_PROGRAM_ID must be a valid Solana address".into()))
}

pub fn v2_protocol_config_address(program_id: &Pubkey, deployment_id: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG_V2_SEED, deployment_id], program_id)
}

pub fn v2_station_registry_address(program_id: &Pubkey, deployment_id: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[STATION_REGISTRY_SEED, deployment_id], program_id)
}

pub fn v2_station_address(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    station_id: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"station-v2", deployment_id, station_id], program_id)
}

pub fn v2_asset_address(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    asset_id: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[ASSET_SEED, deployment_id, asset_id], program_id)
}

pub fn v2_event_anchor_address(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    event_id: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[EVENT_SEED, deployment_id, event_id], program_id)
}

pub fn record_observation_discriminator() -> [u8; 8] {
    let digest = Sha256::digest(b"global:record_observation");
    digest[..8]
        .try_into()
        .expect("SHA-256 output always contains eight bytes")
}

fn build_record_observation_data(record: &DomainAnchorRecord, station_id: [u8; 32]) -> Vec<u8> {
    let mut data = Vec::with_capacity(RECORD_OBSERVATION_DATA_LEN);
    data.extend_from_slice(&record_observation_discriminator());
    data.extend_from_slice(&record.envelope.subject_id);
    data.extend_from_slice(&record.envelope.event_id);
    data.extend_from_slice(&station_id);
    data.extend_from_slice(&record.envelope_bytes);
    debug_assert_eq!(data.len(), RECORD_OBSERVATION_DATA_LEN);
    data
}

fn unique_event_offset(data: &[u8], event: &[u8; 220]) -> Result<u16, ApiError> {
    let mut offsets = data
        .windows(event.len())
        .enumerate()
        .filter_map(|(offset, candidate)| (candidate == event).then_some(offset));
    let offset = offsets.next().ok_or(ApiError::Internal)?;
    if offsets.next().is_some() {
        return Err(ApiError::Internal);
    }
    u16::try_from(offset).map_err(|_| ApiError::Internal)
}

fn meta(address: String, is_signer: bool, is_writable: bool) -> AccountMetaDto {
    AccountMetaDto {
        address,
        is_signer,
        is_writable,
    }
}

fn measure_legacy_transaction(instructions: &[InstructionDto], fee_payer: &Pubkey) -> usize {
    let mut keys = HashSet::new();
    keys.insert(fee_payer.to_string());
    for instruction in instructions {
        keys.insert(instruction.program_id.clone());
        for account in &instruction.accounts {
            keys.insert(account.address.clone());
        }
    }
    let signatures = 1 + 64;
    let message_header = 3;
    let account_keys = shortvec_len(keys.len()) + 32 * keys.len();
    let recent_blockhash = 32;
    let instruction_count = shortvec_len(instructions.len());
    let compiled_instructions = instructions
        .iter()
        .map(|instruction| {
            let data_len = BASE64_STANDARD
                .decode(&instruction.data_base64)
                .expect("builder only measures canonical base64")
                .len();
            1 + shortvec_len(instruction.accounts.len())
                + instruction.accounts.len()
                + shortvec_len(data_len)
                + data_len
        })
        .sum::<usize>();
    signatures
        + message_header
        + account_keys
        + recent_blockhash
        + instruction_count
        + compiled_instructions
}

fn shortvec_len(mut value: usize) -> usize {
    let mut length = 1;
    while value >= 0x80 {
        value >>= 7;
        length += 1;
    }
    length
}

#[cfg(test)]
mod tests {
    use super::*;
    use lastro_protocol::v2::{DomainEventEnvelope, EventType};

    fn valid_station_pubkey() -> [u8; 33] {
        hex::decode("036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296")
            .unwrap()
            .try_into()
            .unwrap()
    }

    fn record() -> DomainAnchorRecord {
        let envelope = DomainEventEnvelope::new(
            EventType::ObservationRecorded,
            [1; 32],
            [2; 32],
            [3; 32],
            1,
            [0; 32],
            [4; 32],
            derive_station_id(&valid_station_pubkey()).unwrap(),
            10,
            20,
        )
        .unwrap();
        DomainAnchorRecord {
            envelope,
            envelope_bytes: envelope.encode().unwrap(),
            station_pubkey33: valid_station_pubkey(),
            station_signature64: [0x44; 64],
            event_hash: envelope.event_hash().unwrap(),
            status: "EVIDENCE_ACCEPTED".into(),
            tx_signature: None,
        }
    }

    #[test]
    fn builder_binds_the_220_byte_event_at_the_anchor_argument_offset() {
        let value = build_transaction_data(
            "11111111111111111111111111111111",
            &record(),
            Pubkey::new_from_array([5; 32]),
            Pubkey::new_from_array([6; 32]),
        )
        .unwrap();
        assert_eq!(value.instructions.len(), 2);
        let data = BASE64_STANDARD
            .decode(&value.instructions[1].data_base64)
            .unwrap();
        assert_eq!(data.len(), RECORD_OBSERVATION_DATA_LEN);
        assert_eq!(
            &data[RECORD_OBSERVATION_ARGUMENT_PREFIX_LEN..],
            &record().envelope_bytes
        );
    }

    #[test]
    fn duplicate_event_bytes_in_prefix_are_rejected_by_unique_offset() {
        let event = [0x5a; 220];
        let mut data = event.to_vec();
        data.extend_from_slice(&event);
        assert!(unique_event_offset(&data, &event).is_err());
    }
}
