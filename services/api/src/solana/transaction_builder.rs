//! Prepare the exact two-instruction Lastro transaction envelope for a custodian wallet.
//!
//! The API never signs as custodian. It returns inspectable instruction descriptors;
//! the browser must independently validate them before asking the connected wallet to
//! sign. The Secp256r1 descriptor references the raw 276-byte StationEvent embedded in
//! instruction 1, never its 32-byte hash.

use std::{collections::HashSet, str::FromStr};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use lastro_protocol::{Action, StationEvent};
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

use crate::{
    error::ApiError,
    model::{AccountMetaDto, InstructionDto, TransactionDataResponse, TransactionVersionDto},
    repository::events::EventRecord,
    solana::secp256r1::{
        SECP256R1_PROGRAM_ID, Secp256r1Descriptor, build_secp256r1_instruction_data,
    },
};

const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const INSTRUCTIONS_SYSVAR_ID: &str = "Sysvar1nstructions1111111111111111111111111";
const MAX_LEGACY_TRANSACTION_BYTES: usize = 1232;
const CONFIG_SEED: &[u8] = b"config";
const ANIMAL_SEED: &[u8] = b"animal";
const RFID_SEED: &[u8] = b"rfid";

pub fn build_transaction_data(
    lastro_program_id: &str,
    record: &EventRecord,
) -> Result<TransactionDataResponse, ApiError> {
    let program_id = parse_program_id(lastro_program_id)?;
    let event = &record.event;
    let required_signer = required_signer(event);
    let lastro_data = build_lastro_instruction_data(event, &record.event_bytes);
    let event_offset = unique_event_offset(&lastro_data, &record.event_bytes)?;
    let secp_data = build_secp256r1_instruction_data(Secp256r1Descriptor {
        station_pubkey33: &record.station_pubkey,
        station_signature64: &record.station_signature,
        message_data_offset: event_offset,
    })?;

    let accounts = instruction_accounts(&program_id, event)?;
    let instructions = vec![
        InstructionDto {
            program_id: SECP256R1_PROGRAM_ID.to_owned(),
            accounts: Vec::new(),
            data_base64: BASE64_STANDARD.encode(&secp_data),
        },
        InstructionDto {
            program_id: program_id.to_string(),
            accounts,
            data_base64: BASE64_STANDARD.encode(&lastro_data),
        },
    ];

    let measured_serialized_bytes = measure_legacy_transaction(&instructions, &required_signer);
    if measured_serialized_bytes > MAX_LEGACY_TRANSACTION_BYTES {
        return Err(ApiError::Conflict(format!(
            "Lastro transaction requires {measured_serialized_bytes} bytes, exceeding the 1232-byte Solana limit"
        )));
    }

    Ok(TransactionDataResponse {
        required_signer: required_signer.to_string(),
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

pub fn protocol_config_address(program_id: &Pubkey, deployment_id: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG_SEED, deployment_id], program_id)
}

pub fn animal_state_address(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    animal_id: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[ANIMAL_SEED, deployment_id, animal_id], program_id)
}

pub fn rfid_binding_address(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    rfid_hash: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[RFID_SEED, deployment_id, rfid_hash], program_id)
}

pub fn lastro_instruction_discriminator(action: Action) -> [u8; 8] {
    let name = match action {
        Action::Origin => "origin",
        Action::Transfer => "transfer",
        Action::Reidentify => "reidentify",
    };
    let digest = Sha256::digest(format!("global:{name}").as_bytes());
    digest[..8]
        .try_into()
        .expect("SHA-256 output always contains eight bytes")
}

fn build_lastro_instruction_data(action_event: &StationEvent, event_bytes: &[u8; 276]) -> Vec<u8> {
    let mut data = Vec::with_capacity(8 + event_bytes.len());
    data.extend_from_slice(&lastro_instruction_discriminator(action_event.action));
    data.extend_from_slice(event_bytes);
    data
}

fn unique_event_offset(data: &[u8], event: &[u8; 276]) -> Result<u16, ApiError> {
    let mut offsets = data
        .windows(event.len())
        .enumerate()
        .filter_map(|(offset, candidate)| (candidate == event).then_some(offset));
    let offset = offsets.next().ok_or_else(|| ApiError::Internal)?;
    if offsets.next().is_some() {
        return Err(ApiError::Internal);
    }
    u16::try_from(offset).map_err(|_| ApiError::Internal)
}

fn required_signer(event: &StationEvent) -> Pubkey {
    let bytes = match event.action {
        Action::Origin => event.to_custodian,
        Action::Transfer | Action::Reidentify => event.from_custodian,
    };
    Pubkey::new_from_array(bytes)
}

fn instruction_accounts(
    program_id: &Pubkey,
    event: &StationEvent,
) -> Result<Vec<AccountMetaDto>, ApiError> {
    let signer = required_signer(event).to_string();
    let (config, _) = protocol_config_address(program_id, &event.deployment_id);
    let (animal, _) = animal_state_address(program_id, &event.deployment_id, &event.animal_id);
    let sysvar = AccountMetaDto {
        address: INSTRUCTIONS_SYSVAR_ID.into(),
        is_signer: false,
        is_writable: false,
    };
    let system = AccountMetaDto {
        address: SYSTEM_PROGRAM_ID.into(),
        is_signer: false,
        is_writable: false,
    };

    let accounts = match event.action {
        Action::Origin => {
            let (binding, _) =
                rfid_binding_address(program_id, &event.deployment_id, &event.new_rfid_hash);
            vec![
                meta(signer, true, true),
                meta(config.to_string(), false, false),
                meta(animal.to_string(), false, true),
                meta(binding.to_string(), false, true),
                sysvar,
                system,
            ]
        }
        Action::Transfer => {
            let (binding, _) =
                rfid_binding_address(program_id, &event.deployment_id, &event.old_rfid_hash);
            vec![
                meta(signer, true, false),
                meta(config.to_string(), false, false),
                meta(animal.to_string(), false, true),
                meta(binding.to_string(), false, false),
                sysvar,
            ]
        }
        Action::Reidentify => {
            let (old_binding, _) =
                rfid_binding_address(program_id, &event.deployment_id, &event.old_rfid_hash);
            let (new_binding, _) =
                rfid_binding_address(program_id, &event.deployment_id, &event.new_rfid_hash);
            vec![
                meta(signer, true, true),
                meta(config.to_string(), false, false),
                meta(animal.to_string(), false, true),
                meta(old_binding.to_string(), false, true),
                meta(new_binding.to_string(), false, true),
                sysvar,
                system,
            ]
        }
    };
    Ok(accounts)
}

fn meta(address: String, is_signer: bool, is_writable: bool) -> AccountMetaDto {
    AccountMetaDto {
        address,
        is_signer,
        is_writable,
    }
}

/// Measure the exact serialized length of the legacy transaction represented by the response.
/// The required signer is also the fee payer, so the transaction has exactly one signature.
fn measure_legacy_transaction(instructions: &[InstructionDto], fee_payer: &Pubkey) -> usize {
    let mut keys = HashSet::new();
    keys.insert(fee_payer.to_string());
    for instruction in instructions {
        keys.insert(instruction.program_id.clone());
        for account in &instruction.accounts {
            keys.insert(account.address.clone());
        }
    }

    let signature_count = 1usize;
    let signatures = shortvec_len(signature_count) + 64 * signature_count;
    let message_header = 3usize;
    let account_keys = shortvec_len(keys.len()) + 32 * keys.len();
    let recent_blockhash = 32usize;
    let instruction_count = shortvec_len(instructions.len());
    let compiled_instructions = instructions
        .iter()
        .map(|instruction| {
            let data_len = BASE64_STANDARD
                .decode(&instruction.data_base64)
                .expect("builder only measures its own canonical base64")
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

    #[test]
    fn anchor_discriminators_are_action_specific() {
        assert_ne!(
            lastro_instruction_discriminator(Action::Origin),
            lastro_instruction_discriminator(Action::Transfer)
        );
        assert_ne!(
            lastro_instruction_discriminator(Action::Transfer),
            lastro_instruction_discriminator(Action::Reidentify)
        );
    }

    #[test]
    fn raw_event_offset_is_derived_from_serialized_instruction() {
        let event = [0x5a; 276];
        let mut data = vec![0u8; 11];
        data.extend_from_slice(&event);
        assert_eq!(unique_event_offset(&data, &event).unwrap(), 11);
    }

    #[test]
    fn duplicate_event_slice_is_rejected() {
        let event = [0x5a; 276];
        let mut data = Vec::new();
        data.extend_from_slice(&event);
        data.extend_from_slice(&event);
        assert!(unique_event_offset(&data, &event).is_err());
    }
}
