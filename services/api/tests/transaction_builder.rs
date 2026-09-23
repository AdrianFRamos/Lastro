//! Exact Solana transaction-envelope contracts for the Lastro API.

use std::{fs, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use lastro_api::{
    repository::events::EventRecord,
    solana::{
        secp256r1::{
            LASTRO_INSTRUCTION_INDEX, LASTRO_SECP_INSTRUCTION_DATA_LEN,
            LASTRO_SECP_PRECOMPILE_INSTRUCTION_INDEX, LASTRO_SECP_PUBKEY_OFFSET,
            LASTRO_SECP_SIGNATURE_OFFSET, LASTRO_SIGNED_MESSAGE_LEN, SECP256R1_PROGRAM_ID,
            Secp256r1Descriptor, build_secp256r1_instruction_data,
        },
        transaction_builder::{
            build_transaction_data, lastro_instruction_discriminator, parse_program_id,
            rfid_binding_address,
        },
    },
};
use lastro_protocol::{Action, StationEvent};
use serde_json::Value;
use uuid::Uuid;

const PROGRAM_ID: &str = "Vote111111111111111111111111111111111111111";
const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const INSTRUCTIONS_SYSVAR_ID: &str = "Sysvar1nstructions1111111111111111111111111";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn vectors() -> Value {
    serde_json::from_slice(&fs::read(root().join("test-vectors/vectors.json")).unwrap()).unwrap()
}

fn decode_hex<const N: usize>(value: &str) -> [u8; N] {
    hex::decode(value).unwrap().try_into().unwrap()
}

fn record(name: &str) -> EventRecord {
    let vectors = vectors();
    let fixture = &vectors["events"][name];
    let event_bytes: [u8; 276] = decode_hex(fixture["event_bytes_hex"].as_str().unwrap());
    let event = StationEvent::decode(&event_bytes).unwrap();
    let observed_rfid = match event.action {
        Action::Origin | Action::Transfer => {
            decode_hex(vectors["rfid"]["a_canonical_hex"].as_str().unwrap())
        }
        Action::Reidentify => decode_hex(vectors["rfid"]["b_canonical_hex"].as_str().unwrap()),
    };
    EventRecord {
        capture_id: Uuid::new_v4(),
        event_hash: decode_hex(fixture["event_hash_hex"].as_str().unwrap()),
        event,
        event_bytes,
        observed_rfid,
        station_pubkey: decode_hex(
            vectors["station"]["pubkey_compressed_hex"]
                .as_str()
                .unwrap(),
        ),
        station_signature: decode_hex(fixture["station_signature_hex"].as_str().unwrap()),
        tx_signature: None,
        status: "EVIDENCE_ACCEPTED".into(),
    }
}

fn instruction_data(
    response: &lastro_api::model::TransactionDataResponse,
    index: usize,
) -> Vec<u8> {
    BASE64
        .decode(&response.instructions[index].data_base64)
        .unwrap()
}

fn descriptor_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap())
}

#[test]
fn origin_transaction_places_secp_before_lastro() {
    // PURPOSE: ORIGIN must use the exact instruction order inspected by the on-chain program.
    // ASSERT: Instruction 0 is Secp256r1, instruction 1 is ORIGIN for Lastro, and no third instruction exists.
    // FAILURE MEANS: The program could inspect a different instruction than the precompile actually verified.
    let response = build_transaction_data(PROGRAM_ID, &record("origin")).unwrap();
    assert_eq!(response.instructions.len(), 2);
    assert_eq!(response.instructions[0].program_id, SECP256R1_PROGRAM_ID);
    assert_eq!(response.instructions[1].program_id, PROGRAM_ID);
    assert_eq!(
        instruction_data(&response, 1)[..8],
        lastro_instruction_discriminator(Action::Origin)
    );
}

#[test]
fn transfer_transaction_places_secp_before_lastro() {
    // PURPOSE: TRANSFER must use the same cryptographic envelope and current custodian signer.
    // ASSERT: Secp256r1 is first, TRANSFER is second, and custodian A is the required signer/account authority.
    // FAILURE MEANS: TRANSFER could bypass the exact StationEvent binding or use stale authority semantics.
    let record = record("transfer");
    let response = build_transaction_data(PROGRAM_ID, &record).unwrap();
    assert_eq!(response.instructions.len(), 2);
    assert_eq!(response.instructions[0].program_id, SECP256R1_PROGRAM_ID);
    assert_eq!(
        instruction_data(&response, 1)[..8],
        lastro_instruction_discriminator(Action::Transfer)
    );
    assert_eq!(
        response.required_signer,
        solana_pubkey::Pubkey::new_from_array(record.event.from_custodian).to_string()
    );
    assert_eq!(
        response.instructions[1].accounts[0].address,
        response.required_signer
    );
    assert!(response.instructions[1].accounts[0].is_signer);
}

#[test]
fn reidentify_transaction_places_secp_before_lastro() {
    // PURPOSE: REIDENTIFY must bind both old and replacement RFID accounts to the same signed event.
    // ASSERT: The envelope is Secp256r1 then REIDENTIFY and contains the exact old/new RfidBinding PDAs in order.
    // FAILURE MEANS: Reidentification could mutate identity bindings outside the Station signature context.
    let record = record("reidentify");
    let response = build_transaction_data(PROGRAM_ID, &record).unwrap();
    let program_id = parse_program_id(PROGRAM_ID).unwrap();
    let (old_binding, _) = rfid_binding_address(
        &program_id,
        &record.event.deployment_id,
        &record.event.old_rfid_hash,
    );
    let (new_binding, _) = rfid_binding_address(
        &program_id,
        &record.event.deployment_id,
        &record.event.new_rfid_hash,
    );
    assert_eq!(response.instructions[0].program_id, SECP256R1_PROGRAM_ID);
    assert_eq!(
        instruction_data(&response, 1)[..8],
        lastro_instruction_discriminator(Action::Reidentify)
    );
    assert_eq!(
        response.instructions[1].accounts[3].address,
        old_binding.to_string()
    );
    assert_eq!(
        response.instructions[1].accounts[4].address,
        new_binding.to_string()
    );
    assert_ne!(old_binding, new_binding);
}

#[test]
fn secp_message_offset_is_derived_from_serialized_lastro_instruction() {
    // PURPOSE: The precompile offset must reference the actual raw StationEvent location after Anchor serialization.
    // ASSERT: The descriptor offset resolves to the unique 276-byte event subsequence inside instruction 1.
    // FAILURE MEANS: An IDL/layout change could make the precompile verify bytes different from those Lastro processes.
    let record = record("origin");
    let response = build_transaction_data(PROGRAM_ID, &record).unwrap();
    let secp = instruction_data(&response, 0);
    let lastro = instruction_data(&response, 1);
    let offset = usize::from(descriptor_u16(&secp, 10));
    assert_eq!(&lastro[offset..offset + 276], record.event_bytes.as_slice());
    assert_eq!(
        lastro
            .windows(276)
            .filter(|window| *window == record.event_bytes)
            .count(),
        1
    );
}

#[test]
fn secp_message_length_is_276() {
    // PURPOSE: Secp256r1 must verify exactly one complete StationEvent and no truncation/extension.
    // ASSERT: The official descriptor message size is exactly the frozen 276-byte StationEvent length.
    // FAILURE MEANS: The precompile could authenticate a different message boundary than the protocol parser.
    let response = build_transaction_data(PROGRAM_ID, &record("transfer")).unwrap();
    let secp = instruction_data(&response, 0);
    assert_eq!(descriptor_u16(&secp, 12), LASTRO_SIGNED_MESSAGE_LEN);
    assert_eq!(descriptor_u16(&secp, 12), 276);
}

#[test]
fn secp_uses_submitted_station_pubkey_and_signature() {
    // PURPOSE: The API must preserve Station cryptographic identity instead of substituting server material.
    // ASSERT: The Secp256r1 instruction contains the exact submitted 64-byte signature and 33-byte compressed key.
    // FAILURE MEANS: The backend could alter or impersonate Station evidence during transaction preparation.
    let record = record("reidentify");
    let response = build_transaction_data(PROGRAM_ID, &record).unwrap();
    let secp = instruction_data(&response, 0);
    assert_eq!(&secp[16..80], record.station_signature.as_slice());
    assert_eq!(&secp[80..113], record.station_pubkey.as_slice());
}

#[test]
fn transaction_uses_current_custodian_as_required_signer() {
    // PURPOSE: Wallet authority must come directly from StationEvent transition semantics.
    // ASSERT: ORIGIN requires the destination custodian; TRANSFER and REIDENTIFY require the current/source custodian.
    // FAILURE MEANS: A stale or future custodian could be asked to authorize a protected transition.
    let origin = record("origin");
    let transfer = record("transfer");
    let reidentify = record("reidentify");
    assert_eq!(
        build_transaction_data(PROGRAM_ID, &origin)
            .unwrap()
            .required_signer,
        solana_pubkey::Pubkey::new_from_array(origin.event.to_custodian).to_string()
    );
    assert_eq!(
        build_transaction_data(PROGRAM_ID, &transfer)
            .unwrap()
            .required_signer,
        solana_pubkey::Pubkey::new_from_array(transfer.event.from_custodian).to_string()
    );
    assert_eq!(
        build_transaction_data(PROGRAM_ID, &reidentify)
            .unwrap()
            .required_signer,
        solana_pubkey::Pubkey::new_from_array(reidentify.event.from_custodian).to_string()
    );
}

#[test]
fn serialized_transaction_size_is_measured_for_every_action() {
    // PURPOSE: The complete legacy envelope must stay below Solana's 1232-byte packet limit for every core operation.
    // ASSERT: ORIGIN, TRANSFER, and REIDENTIFY each report a nonzero complete serialized size at or below 1232 bytes.
    // FAILURE MEANS: The demo could construct cryptographically correct transactions that cannot be submitted to the network.
    for name in ["origin", "transfer", "reidentify"] {
        let response = build_transaction_data(PROGRAM_ID, &record(name)).unwrap();
        assert!(response.measured_serialized_bytes > 64 + 3 + 32);
        assert!(response.measured_serialized_bytes <= 1232, "{name}");
    }
}

#[test]
fn secp_message_is_raw_station_event_not_event_hash() {
    // PURPOSE: Prevent accidental double hashing between Station ECDSA and the Solana Secp256r1 precompile.
    // ASSERT: Descriptor points to 276 raw event bytes inside Lastro instruction 1 and not the 32-byte event hash.
    // FAILURE MEANS: Station, browser, precompile, and program could authenticate different messages.
    let record = record("origin");
    let response = build_transaction_data(PROGRAM_ID, &record).unwrap();
    let secp = instruction_data(&response, 0);
    let lastro = instruction_data(&response, 1);
    let offset = usize::from(descriptor_u16(&secp, 10));
    let length = usize::from(descriptor_u16(&secp, 12));
    assert_eq!(length, 276);
    assert_eq!(
        &lastro[offset..offset + length],
        record.event_bytes.as_slice()
    );
    assert_ne!(&lastro[offset..offset + 32], record.event_hash.as_slice());
}

#[test]
fn secp_instruction_data_uses_official_u16_offset_layout_and_is_113_bytes() {
    // PURPOSE: Freeze Lastro's one-signature Secp256r1 wire layout to the official seven-u16 descriptor contract.
    // ASSERT: Header, offsets, instruction indexes, raw signature and compressed key occupy the exact expected byte ranges.
    // FAILURE MEANS: Solana runtime and Lastro could interpret the precompile descriptor differently.
    let station_signature = [0x5a; 64];
    let mut station_pubkey = [0x6b; 33];
    station_pubkey[0] = 0x02;
    let message_offset = 0x1234;
    let data = build_secp256r1_instruction_data(Secp256r1Descriptor {
        station_pubkey33: &station_pubkey,
        station_signature64: &station_signature,
        message_data_offset: message_offset,
    })
    .unwrap();

    assert_eq!(data.len(), LASTRO_SECP_INSTRUCTION_DATA_LEN);
    assert_eq!(data[0], 1);
    assert_eq!(data[1], 0);
    assert_eq!(descriptor_u16(&data, 2), LASTRO_SECP_SIGNATURE_OFFSET);
    assert_eq!(
        descriptor_u16(&data, 4),
        LASTRO_SECP_PRECOMPILE_INSTRUCTION_INDEX
    );
    assert_eq!(descriptor_u16(&data, 6), LASTRO_SECP_PUBKEY_OFFSET);
    assert_eq!(
        descriptor_u16(&data, 8),
        LASTRO_SECP_PRECOMPILE_INSTRUCTION_INDEX
    );
    assert_eq!(descriptor_u16(&data, 10), message_offset);
    assert_eq!(descriptor_u16(&data, 12), LASTRO_SIGNED_MESSAGE_LEN);
    assert_eq!(descriptor_u16(&data, 14), LASTRO_INSTRUCTION_INDEX);
    assert_eq!(&data[16..80], station_signature.as_slice());
    assert_eq!(&data[80..113], station_pubkey.as_slice());
}

#[test]
fn action_account_sets_are_minimal_and_explicit() {
    // PURPOSE: The wallet must not receive unrelated writable accounts in a Lastro transition.
    // ASSERT: Only the documented action-specific account counts exist and sysvar/system positions are fixed.
    // FAILURE MEANS: A compromised builder could smuggle unrelated writable accounts into a custodian signature request.
    let origin = build_transaction_data(PROGRAM_ID, &record("origin")).unwrap();
    assert_eq!(origin.instructions[1].accounts.len(), 6);
    assert_eq!(
        origin.instructions[1].accounts[4].address,
        INSTRUCTIONS_SYSVAR_ID
    );
    assert_eq!(
        origin.instructions[1].accounts[5].address,
        SYSTEM_PROGRAM_ID
    );

    let transfer = build_transaction_data(PROGRAM_ID, &record("transfer")).unwrap();
    assert_eq!(transfer.instructions[1].accounts.len(), 5);
    assert_eq!(
        transfer.instructions[1].accounts[4].address,
        INSTRUCTIONS_SYSVAR_ID
    );

    let reidentify = build_transaction_data(PROGRAM_ID, &record("reidentify")).unwrap();
    assert_eq!(reidentify.instructions[1].accounts.len(), 7);
    assert_eq!(
        reidentify.instructions[1].accounts[5].address,
        INSTRUCTIONS_SYSVAR_ID
    );
    assert_eq!(
        reidentify.instructions[1].accounts[6].address,
        SYSTEM_PROGRAM_ID
    );
}
