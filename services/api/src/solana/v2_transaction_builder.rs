//! Prepare exact v2 transactions for wallets.
//!
//! Station events use two instructions: Solana's Secp256r1 precompile, then the Lastro v2
//! instruction that anchors the envelope (`record_observation`, `bind_identifier` or
//! `replace_identifier`). Asset registration is a single `register_asset`. The wallet is the
//! only signer; this module never handles private keys.

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
pub const COMPUTE_BUDGET_PROGRAM_ID: &str = "ComputeBudget111111111111111111111111111111";
/// Lastro instructions run inside the default 200k budget; the explicit limit only prices it.
pub const PRIORITY_FEE_COMPUTE_UNIT_LIMIT: u32 = 200_000;
const RFID_BINDING_SEED: &[u8] = b"rfid";
const EVENT_TYPE_OBSERVATION: u16 = 2;
const EVENT_TYPE_IDENTIFIER_BOUND: u16 = 18;
const EVENT_TYPE_IDENTIFIER_REPLACED: u16 = 19;
const INSTRUCTIONS_SYSVAR_ID: &str = "Sysvar1nstructions1111111111111111111111111";
const MAX_LEGACY_TRANSACTION_BYTES: usize = 1232;
const CONFIG_V2_SEED: &[u8] = b"config-v2";
const STATION_REGISTRY_SEED: &[u8] = b"station-registry";
const ASSET_SEED: &[u8] = b"asset";
const EVENT_SEED: &[u8] = b"event";
const RECORD_OBSERVATION_ARGUMENT_PREFIX_LEN: usize = 32 + 32 + 32 + 8;
const RECORD_OBSERVATION_DATA_LEN: usize = RECORD_OBSERVATION_ARGUMENT_PREFIX_LEN + 220;

pub const LASTRO_SECP256R1_MESSAGE_LEN: u16 = 220;

/// RFID hashes carried after the envelope by identity instructions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentityArgs {
    pub old_rfid_hash: [u8; 32],
    pub new_rfid_hash: [u8; 32],
}

/// Station-event transaction. `signer` is the config authority for observations and the
/// asset's current custodian for identity events (the program enforces both).
pub fn build_transaction_data(
    lastro_program_id: &str,
    record: &DomainAnchorRecord,
    signer: Pubkey,
    station_registry: Pubkey,
    identity: Option<IdentityArgs>,
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
    let deployment = &record.envelope.deployment_id;
    let (instruction_name, trailing): (&str, Vec<[u8; 32]>) =
        match (record.envelope.event_type, identity) {
            (EVENT_TYPE_OBSERVATION, None) => ("record_observation", Vec::new()),
            (EVENT_TYPE_IDENTIFIER_BOUND, Some(args)) if args.old_rfid_hash == [0; 32] => {
                ("bind_identifier", vec![args.new_rfid_hash])
            }
            (EVENT_TYPE_IDENTIFIER_REPLACED, Some(args)) => (
                "replace_identifier",
                vec![args.old_rfid_hash, args.new_rfid_hash],
            ),
            _ => {
                return Err(ApiError::Conflict(
                    "v2 event type and identity arguments are inconsistent".into(),
                ));
            }
        };

    let mut lastro_data = Vec::with_capacity(RECORD_OBSERVATION_DATA_LEN + 64);
    lastro_data.extend_from_slice(&discriminator(instruction_name));
    lastro_data.extend_from_slice(&record.envelope.subject_id);
    lastro_data.extend_from_slice(&record.envelope.event_id);
    lastro_data.extend_from_slice(&station_id);
    lastro_data.extend_from_slice(&record.envelope_bytes);
    for hash in &trailing {
        lastro_data.extend_from_slice(hash);
    }
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

    let (config, _) = v2_protocol_config_address(&program_id, deployment);
    let (asset, _) = v2_asset_address(&program_id, deployment, &record.envelope.subject_id);
    let (event_anchor, _) =
        v2_event_anchor_address(&program_id, deployment, &record.envelope.event_id);
    let mut accounts = vec![
        meta(
            signer.to_string(),
            true,
            instruction_name != "record_observation",
        ),
        meta(config.to_string(), false, false),
        meta(station_registry.to_string(), false, false),
        meta(
            v2_station_address(&program_id, deployment, &station_id)
                .0
                .to_string(),
            false,
            false,
        ),
        meta(asset.to_string(), false, true),
        meta(event_anchor.to_string(), false, true),
    ];
    for hash in &trailing {
        accounts.push(meta(
            v2_rfid_binding_address(&program_id, deployment, hash)
                .0
                .to_string(),
            false,
            true,
        ));
    }
    accounts.push(meta(INSTRUCTIONS_SYSVAR_ID.into(), false, false));
    accounts.push(meta(SYSTEM_PROGRAM_ID.into(), false, false));

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
    finish(program_id, signer, instructions, "v2 Station event")
}

/// `register_asset`, signed by the deployment authority.
pub fn build_register_asset(
    lastro_program_id: &str,
    deployment_id: &[u8; 32],
    authority: Pubkey,
    asset_id: [u8; 32],
    asset_type: u8,
    custodian: Pubkey,
    available_weight_grams: u64,
) -> Result<TransactionDataResponse, ApiError> {
    let program_id = parse_program_id(lastro_program_id)?;
    let mut data = discriminator("register_asset").to_vec();
    data.extend_from_slice(&asset_id);
    data.push(asset_type);
    data.extend_from_slice(custodian.as_ref());
    data.extend_from_slice(&[0; 32]); // parent_root: an animal has no parents
    data.extend_from_slice(&asset_id); // lineage_root: the asset itself is its lineage origin
    data.extend_from_slice(&available_weight_grams.to_le_bytes());
    let instructions = vec![InstructionDto {
        program_id: program_id.to_string(),
        accounts: vec![
            meta(authority.to_string(), true, true),
            meta(
                v2_protocol_config_address(&program_id, deployment_id)
                    .0
                    .to_string(),
                false,
                false,
            ),
            meta(
                v2_asset_address(&program_id, deployment_id, &asset_id)
                    .0
                    .to_string(),
                false,
                true,
            ),
            meta(SYSTEM_PROGRAM_ID.into(), false, false),
        ],
        data_base64: BASE64_STANDARD.encode(data),
    }];
    finish(program_id, authority, instructions, "v2 asset registration")
}

fn finish(
    program_id: Pubkey,
    signer: Pubkey,
    instructions: Vec<InstructionDto>,
    label: &str,
) -> Result<TransactionDataResponse, ApiError> {
    let measured_serialized_bytes = measure_legacy_transaction(&instructions, &signer);
    if measured_serialized_bytes > MAX_LEGACY_TRANSACTION_BYTES {
        return Err(ApiError::Conflict(format!(
            "{label} transaction requires {measured_serialized_bytes} bytes, exceeding the 1232-byte Solana limit"
        )));
    }
    Ok(TransactionDataResponse {
        required_signer: signer.to_string(),
        lastro_program_id: program_id.to_string(),
        instructions,
        measured_serialized_bytes,
        transaction_version: TransactionVersionDto::Legacy,
    })
}

/// Append `set_compute_unit_limit` + `set_compute_unit_price` after the protocol
/// instructions. The programs accept only ComputeBudget after index 1, so the Secp256r1
/// descriptor indices never move.
pub fn with_priority_fee(
    mut data: TransactionDataResponse,
    micro_lamports_per_unit: Option<u64>,
) -> Result<TransactionDataResponse, ApiError> {
    let Some(price) = micro_lamports_per_unit else {
        return Ok(data);
    };
    let mut limit = vec![2u8];
    limit.extend_from_slice(&PRIORITY_FEE_COMPUTE_UNIT_LIMIT.to_le_bytes());
    let mut unit_price = vec![3u8];
    unit_price.extend_from_slice(&price.to_le_bytes());
    for bytes in [limit, unit_price] {
        data.instructions.push(InstructionDto {
            program_id: COMPUTE_BUDGET_PROGRAM_ID.to_owned(),
            accounts: Vec::new(),
            data_base64: BASE64_STANDARD.encode(bytes),
        });
    }
    let signer = Pubkey::from_str(&data.required_signer).map_err(|_| ApiError::Internal)?;
    data.measured_serialized_bytes = measure_legacy_transaction(&data.instructions, &signer);
    if data.measured_serialized_bytes > MAX_LEGACY_TRANSACTION_BYTES {
        return Err(ApiError::Conflict(
            "priority fee instructions exceed the 1232-byte Solana limit".into(),
        ));
    }
    Ok(data)
}

fn discriminator(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("global:{name}").as_bytes());
    digest[..8]
        .try_into()
        .expect("SHA-256 output always contains eight bytes")
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

pub fn v2_rfid_binding_address(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    rfid_hash: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[RFID_BINDING_SEED, deployment_id, rfid_hash], program_id)
}

pub fn v2_event_anchor_address(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    event_id: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[EVENT_SEED, deployment_id, event_id], program_id)
}

pub fn v2_facility_address(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    facility_id: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"facility", deployment_id, facility_id], program_id)
}

pub fn v2_transformation_address(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    transformation_id: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"transformation", deployment_id, transformation_id],
        program_id,
    )
}

/// `finalize_transformation`, signed by the facility owner (the program's operator).
pub fn build_finalize_transformation(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    facility_id: &[u8; 32],
    transformation_id: &[u8; 32],
    operator: Pubkey,
) -> TransactionDataResponse {
    let digest = Sha256::digest(b"global:finalize_transformation");
    let accounts = vec![
        meta(operator.to_string(), true, false),
        meta(
            v2_protocol_config_address(program_id, deployment_id)
                .0
                .to_string(),
            false,
            false,
        ),
        meta(
            v2_facility_address(program_id, deployment_id, facility_id)
                .0
                .to_string(),
            false,
            false,
        ),
        meta(
            v2_transformation_address(program_id, deployment_id, transformation_id)
                .0
                .to_string(),
            false,
            true,
        ),
    ];
    let instructions = vec![InstructionDto {
        program_id: program_id.to_string(),
        accounts,
        data_base64: BASE64_STANDARD.encode(&digest[..8]),
    }];
    let measured_serialized_bytes = measure_legacy_transaction(&instructions, &operator);
    TransactionDataResponse {
        required_signer: operator.to_string(),
        lastro_program_id: program_id.to_string(),
        instructions,
        measured_serialized_bytes,
        transaction_version: TransactionVersionDto::Legacy,
    }
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
            capture_id: None,
            observed_rfid: None,
        }
    }

    #[test]
    fn builder_binds_the_220_byte_event_at_the_anchor_argument_offset() {
        let value = build_transaction_data(
            "11111111111111111111111111111111",
            &record(),
            Pubkey::new_from_array([5; 32]),
            Pubkey::new_from_array([6; 32]),
            None,
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
    fn identity_events_carry_rfid_hashes_after_the_envelope_and_need_matching_args() {
        // PURPOSE: bind/replace transactions match the on-chain argument and account layout.
        // ASSERT: replace appends old+new hashes after the envelope, adds both RFID PDAs, and
        // makes the custodian a writable signer; mismatched args are rejected.
        // FAILURE MEANS: the wallet would sign a transaction the program rejects.
        let mut value = record();
        let mut envelope = value.envelope;
        envelope.event_type = EventType::IdentifierReplaced as u16;
        value.envelope = envelope;
        value.envelope_bytes = envelope.encode().unwrap();
        let custodian = Pubkey::new_from_array([7; 32]);
        let args = IdentityArgs {
            old_rfid_hash: [8; 32],
            new_rfid_hash: [9; 32],
        };
        let tx = build_transaction_data(
            "11111111111111111111111111111111",
            &value,
            custodian,
            Pubkey::new_from_array([6; 32]),
            Some(args),
        )
        .unwrap();
        let data = BASE64_STANDARD
            .decode(&tx.instructions[1].data_base64)
            .unwrap();
        assert_eq!(data.len(), RECORD_OBSERVATION_DATA_LEN + 64);
        assert_eq!(&data[..8], &discriminator("replace_identifier"));
        assert_eq!(&data[data.len() - 64..data.len() - 32], &[8; 32]);
        assert_eq!(tx.instructions[1].accounts.len(), 10);
        assert!(
            tx.instructions[1].accounts[0].is_signer && tx.instructions[1].accounts[0].is_writable
        );
        assert!(tx.measured_serialized_bytes <= 1232);
        assert!(
            build_transaction_data(
                "11111111111111111111111111111111",
                &value,
                custodian,
                Pubkey::new_from_array([6; 32]),
                None,
            )
            .is_err()
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
