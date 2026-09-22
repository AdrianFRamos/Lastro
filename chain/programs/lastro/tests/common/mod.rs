#![allow(dead_code)]

use std::{env, path::PathBuf};

use anchor_lang::{AccountDeserialize, InstructionData};
use lastro::{
    constants::{ANIMAL_STATE_SEED, PROTOCOL_CONFIG_SEED, RFID_BINDING_SEED},
    state::{AnimalState, ProtocolConfig, RfidBinding},
};
use lastro_protocol::{
    Action, StationEvent,
    crypto::derive_station_id,
    rfid::{canonical_rfid_from_u64, hash_canonical_rfid},
};
use litesvm::{
    LiteSVM,
    types::{FailedTransactionMetadata, TransactionMetadata, TransactionResult},
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as P256Signer};
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signer::Signer as SolanaSigner;
use solana_transaction::Transaction;

pub const TRANSACTION_LIMIT: usize = 1232;
pub const STATION_EVENT_OFFSET_IN_ANCHOR_IX: u16 = 8;
pub const SECP_SIGNATURE_OFFSET: u16 = 16;
pub const SECP_PUBLIC_KEY_OFFSET: u16 = 80;
pub const SECP_DATA_LEN: usize = 113;
pub const LAMPORTS_PER_TEST_WALLET: u64 = 10_000_000_000;

const TEST_PRIVATE_SCALAR: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
];
const P256_ORDER: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84,
    0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
];

pub struct Harness {
    pub svm: LiteSVM,
    pub authority: Keypair,
    pub wallet_a: Keypair,
    pub wallet_b: Keypair,
    pub wallet_c: Keypair,
    pub deployment_id: [u8; 32],
    pub station_signing_key: SigningKey,
    pub station_pubkey33: [u8; 33],
    pub station_id: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct FlowEvents {
    pub animal_id: [u8; 32],
    pub rfid_a: [u8; 32],
    pub rfid_b: [u8; 32],
    pub origin: StationEvent,
    pub transfer_ab: StationEvent,
    pub reidentify: StationEvent,
    pub transfer_bc: StationEvent,
}

#[derive(Clone, Copy, Debug)]
pub struct SecpDescriptor {
    pub signature_offset: u16,
    pub signature_instruction_index: u16,
    pub public_key_offset: u16,
    pub public_key_instruction_index: u16,
    pub message_offset: u16,
    pub message_length: u16,
    pub message_instruction_index: u16,
}

impl Default for SecpDescriptor {
    fn default() -> Self {
        Self {
            signature_offset: SECP_SIGNATURE_OFFSET,
            signature_instruction_index: 0,
            public_key_offset: SECP_PUBLIC_KEY_OFFSET,
            public_key_instruction_index: 0,
            message_offset: STATION_EVENT_OFFSET_IN_ANCHOR_IX,
            message_length: 276,
            message_instruction_index: 1,
        }
    }
}

impl Harness {
    pub fn new() -> Self {
        let mut svm = LiteSVM::new().with_precompiles();
        let program = program_id();
        svm.add_program_from_file(program, program_so_path())
            .expect("Lastro SBF program must be built before LiteSVM tests");

        let authority = Keypair::new();
        let wallet_a = Keypair::new();
        let wallet_b = Keypair::new();
        let wallet_c = Keypair::new();
        for key in [&authority, &wallet_a, &wallet_b, &wallet_c] {
            svm.airdrop(&key.pubkey(), LAMPORTS_PER_TEST_WALLET)
                .expect("test wallet airdrop must succeed");
        }

        let station_signing_key = SigningKey::from_bytes((&TEST_PRIVATE_SCALAR).into())
            .expect("fixed test-only P-256 scalar must be valid");
        let encoded = station_signing_key.verifying_key().to_encoded_point(true);
        let station_pubkey33: [u8; 33] = encoded
            .as_bytes()
            .try_into()
            .expect("compressed P-256 key must be exactly 33 bytes");
        let station_id = derive_station_id(&station_pubkey33)
            .expect("test Station public key must derive a StationID");

        Self {
            svm,
            authority,
            wallet_a,
            wallet_b,
            wallet_c,
            deployment_id: [0xd0; 32],
            station_signing_key,
            station_pubkey33,
            station_id,
        }
    }

    pub fn initialize(&mut self) {
        let result = send_initialize(
            &mut self.svm,
            &self.authority,
            self.deployment_id,
            self.station_pubkey33,
        );
        assert_success(result);
    }

    pub fn flow(&self, animal_id: [u8; 32]) -> FlowEvents {
        make_flow(
            self.deployment_id,
            animal_id,
            self.station_id,
            self.wallet_a.pubkey().to_bytes(),
            self.wallet_b.pubkey().to_bytes(),
            self.wallet_c.pubkey().to_bytes(),
        )
    }
}

pub fn program_id() -> Pubkey {
    Pubkey::new_from_array(lastro::ID.to_bytes())
}

pub fn system_program_id() -> Pubkey {
    Pubkey::new_from_array(anchor_lang::system_program::ID.to_bytes())
}

pub fn instructions_sysvar_id() -> Pubkey {
    Pubkey::new_from_array(anchor_lang::solana_program::sysvar::instructions::ID.to_bytes())
}

pub fn secp256r1_program_id() -> Pubkey {
    Pubkey::new_from_array(solana_secp256r1_program::ID.to_bytes())
}

pub fn protocol_config_pda(deployment_id: &[u8; 32]) -> (Pubkey, u8) {
    let (pda, bump) = anchor_lang::prelude::Pubkey::find_program_address(
        &[PROTOCOL_CONFIG_SEED, deployment_id],
        &lastro::ID,
    );
    (Pubkey::new_from_array(pda.to_bytes()), bump)
}

pub fn animal_state_pda(deployment_id: &[u8; 32], animal_id: &[u8; 32]) -> (Pubkey, u8) {
    let (pda, bump) = anchor_lang::prelude::Pubkey::find_program_address(
        &[ANIMAL_STATE_SEED, deployment_id, animal_id],
        &lastro::ID,
    );
    (Pubkey::new_from_array(pda.to_bytes()), bump)
}

pub fn rfid_binding_pda(deployment_id: &[u8; 32], rfid_hash: &[u8; 32]) -> (Pubkey, u8) {
    let (pda, bump) = anchor_lang::prelude::Pubkey::find_program_address(
        &[RFID_BINDING_SEED, deployment_id, rfid_hash],
        &lastro::ID,
    );
    (Pubkey::new_from_array(pda.to_bytes()), bump)
}

pub fn program_so_path() -> PathBuf {
    if let Some(path) = env::var_os("LASTRO_PROGRAM_SO") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/deploy/lastro.so")
}

pub fn send_initialize(
    svm: &mut LiteSVM,
    authority: &Keypair,
    deployment_id: [u8; 32],
    station_pubkey33: [u8; 33],
) -> TransactionResult {
    let (config, _) = protocol_config_pda(&deployment_id);
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new(config, false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro::instruction::Initialize {
            deployment_id,
            station_pubkey33,
        }
        .data(),
    };
    send_instructions(svm, vec![ix], authority)
}

pub fn build_lastro_instruction(event: &StationEvent) -> Instruction {
    let raw = event.encode();
    let (config, _) = protocol_config_pda(&event.deployment_id);
    let (animal, _) = animal_state_pda(&event.deployment_id, &event.animal_id);
    let data = match event.action {
        Action::Origin => lastro::instruction::Origin { event: raw }.data(),
        Action::Transfer => lastro::instruction::Transfer { event: raw }.data(),
        Action::Reidentify => lastro::instruction::Reidentify { event: raw }.data(),
    };

    let accounts = match event.action {
        Action::Origin => {
            let (binding, _) = rfid_binding_pda(&event.deployment_id, &event.new_rfid_hash);
            vec![
                AccountMeta::new(Pubkey::new_from_array(event.to_custodian), true),
                AccountMeta::new_readonly(config, false),
                AccountMeta::new(animal, false),
                AccountMeta::new(binding, false),
                AccountMeta::new_readonly(instructions_sysvar_id(), false),
                AccountMeta::new_readonly(system_program_id(), false),
            ]
        }
        Action::Transfer => {
            let (binding, _) = rfid_binding_pda(&event.deployment_id, &event.old_rfid_hash);
            vec![
                AccountMeta::new_readonly(Pubkey::new_from_array(event.from_custodian), true),
                AccountMeta::new_readonly(config, false),
                AccountMeta::new(animal, false),
                AccountMeta::new_readonly(binding, false),
                AccountMeta::new_readonly(instructions_sysvar_id(), false),
            ]
        }
        Action::Reidentify => {
            let (old_binding, _) = rfid_binding_pda(&event.deployment_id, &event.old_rfid_hash);
            let (new_binding, _) = rfid_binding_pda(&event.deployment_id, &event.new_rfid_hash);
            vec![
                AccountMeta::new(Pubkey::new_from_array(event.from_custodian), true),
                AccountMeta::new_readonly(config, false),
                AccountMeta::new(animal, false),
                AccountMeta::new(old_binding, false),
                AccountMeta::new(new_binding, false),
                AccountMeta::new_readonly(instructions_sysvar_id(), false),
                AccountMeta::new_readonly(system_program_id(), false),
            ]
        }
    };

    Instruction {
        program_id: program_id(),
        accounts,
        data,
    }
}

pub fn signing_key_from_small_scalar(value: u8) -> SigningKey {
    assert!(value != 0, "P-256 private scalar must be nonzero");
    let mut scalar = [0u8; 32];
    scalar[31] = value;
    SigningKey::from_bytes((&scalar).into()).expect("small deterministic P-256 scalar must be valid")
}

pub fn compressed_pubkey(signing_key: &SigningKey) -> [u8; 33] {
    signing_key
        .verifying_key()
        .to_encoded_point(true)
        .as_bytes()
        .try_into()
        .expect("compressed P-256 public key must be 33 bytes")
}

pub fn low_s_signature(signing_key: &SigningKey, event_bytes: &[u8; 276]) -> [u8; 64] {
    let signature: Signature = signing_key.sign(event_bytes);
    let signature = signature.normalize_s().unwrap_or(signature);
    signature.to_bytes().into()
}

pub fn high_s_from_low_s(low_s: [u8; 64]) -> [u8; 64] {
    let mut high = low_s;
    let mut borrow = 0u16;
    for index in (0..32).rev() {
        let minuend = P256_ORDER[index] as u16;
        let subtrahend = low_s[32 + index] as u16 + borrow;
        if minuend >= subtrahend {
            high[32 + index] = (minuend - subtrahend) as u8;
            borrow = 0;
        } else {
            high[32 + index] = (minuend + 256 - subtrahend) as u8;
            borrow = 1;
        }
    }
    assert_eq!(borrow, 0, "low-S scalar must be below P-256 order");
    high
}

pub fn build_secp_instruction(
    event_bytes: &[u8; 276],
    signing_key: &SigningKey,
    station_pubkey33: &[u8; 33],
    descriptor: SecpDescriptor,
) -> Instruction {
    build_secp_instruction_with_signature(
        low_s_signature(signing_key, event_bytes),
        station_pubkey33,
        descriptor,
    )
}

pub fn build_secp_instruction_with_signature(
    signature: [u8; 64],
    station_pubkey33: &[u8; 33],
    descriptor: SecpDescriptor,
) -> Instruction {
    let mut data = Vec::with_capacity(SECP_DATA_LEN);
    data.push(1);
    data.push(0);
    for value in [
        descriptor.signature_offset,
        descriptor.signature_instruction_index,
        descriptor.public_key_offset,
        descriptor.public_key_instruction_index,
        descriptor.message_offset,
        descriptor.message_length,
        descriptor.message_instruction_index,
    ] {
        data.extend_from_slice(&value.to_le_bytes());
    }
    data.extend_from_slice(&signature);
    data.extend_from_slice(station_pubkey33);
    assert_eq!(data.len(), SECP_DATA_LEN);

    Instruction {
        program_id: secp256r1_program_id(),
        accounts: vec![],
        data,
    }
}

pub fn build_envelope(
    event: &StationEvent,
    signing_key: &SigningKey,
    station_pubkey33: &[u8; 33],
) -> Vec<Instruction> {
    let lastro_ix = build_lastro_instruction(event);
    assert_eq!(&lastro_ix.data[8..], &event.encode());
    let secp_ix = build_secp_instruction(
        &event.encode(),
        signing_key,
        station_pubkey33,
        SecpDescriptor::default(),
    );
    vec![secp_ix, lastro_ix]
}

pub fn send_event(
    svm: &mut LiteSVM,
    event: &StationEvent,
    signing_key: &SigningKey,
    station_pubkey33: &[u8; 33],
    signer: &Keypair,
) -> TransactionResult {
    send_instructions(
        svm,
        build_envelope(event, signing_key, station_pubkey33),
        signer,
    )
}

pub fn send_event_with_envelope(
    svm: &mut LiteSVM,
    instructions: Vec<Instruction>,
    signer: &Keypair,
) -> TransactionResult {
    send_instructions(svm, instructions, signer)
}

pub fn build_transaction(
    svm: &LiteSVM,
    instructions: Vec<Instruction>,
    signer: &Keypair,
) -> Transaction {
    let message = Message::new(&instructions, Some(&signer.pubkey()));
    Transaction::new(&[signer], message, svm.latest_blockhash())
}

pub fn send_instructions(
    svm: &mut LiteSVM,
    instructions: Vec<Instruction>,
    signer: &Keypair,
) -> TransactionResult {
    let tx = build_transaction(svm, instructions, signer);
    svm.send_transaction(tx)
}

pub fn serialized_transaction_len(tx: &Transaction) -> usize {
    wincode::serialize(tx)
        .expect("Solana transaction must serialize")
        .len()
}

pub fn assert_success(result: TransactionResult) -> TransactionMetadata {
    match result {
        Ok(meta) => meta,
        Err(err) => panic!("transaction unexpectedly failed: {:?}\n{}", err.err, err.meta.logs.join("\n")),
    }
}

pub fn assert_failure(result: TransactionResult) -> FailedTransactionMetadata {
    match result {
        Ok(meta) => panic!("transaction unexpectedly succeeded: {}", meta.pretty_logs()),
        Err(err) => err,
    }
}

pub fn assert_failure_contains(result: TransactionResult, needle: &str) -> FailedTransactionMetadata {
    let failure = assert_failure(result);
    let logs = failure.meta.logs.join("\n");
    assert!(
        logs.contains(needle),
        "expected failure logs to contain {needle:?}; error={:?}; logs=\n{logs}",
        failure.err,
    );
    failure
}

pub fn account_data(svm: &LiteSVM, address: &Pubkey) -> Option<Vec<u8>> {
    svm.get_account(address).map(|account| account.data)
}

pub fn protocol_config(svm: &LiteSVM, deployment_id: &[u8; 32]) -> ProtocolConfig {
    let (address, _) = protocol_config_pda(deployment_id);
    let account = svm.get_account(&address).expect("ProtocolConfig must exist");
    assert_eq!(account.owner, program_id(), "ProtocolConfig owner must be Lastro");
    let mut bytes = account.data.as_slice();
    ProtocolConfig::try_deserialize(&mut bytes).expect("ProtocolConfig must deserialize")
}

pub fn animal_state(svm: &LiteSVM, deployment_id: &[u8; 32], animal_id: &[u8; 32]) -> AnimalState {
    let (address, _) = animal_state_pda(deployment_id, animal_id);
    let account = svm.get_account(&address).expect("AnimalState must exist");
    assert_eq!(account.owner, program_id(), "AnimalState owner must be Lastro");
    let mut bytes = account.data.as_slice();
    AnimalState::try_deserialize(&mut bytes).expect("AnimalState must deserialize")
}

pub fn rfid_binding(svm: &LiteSVM, deployment_id: &[u8; 32], rfid_hash: &[u8; 32]) -> RfidBinding {
    let (address, _) = rfid_binding_pda(deployment_id, rfid_hash);
    let account = svm.get_account(&address).expect("RfidBinding must exist");
    assert_eq!(account.owner, program_id(), "RfidBinding owner must be Lastro");
    let mut bytes = account.data.as_slice();
    RfidBinding::try_deserialize(&mut bytes).expect("RfidBinding must deserialize")
}

pub fn make_flow(
    deployment_id: [u8; 32],
    animal_id: [u8; 32],
    station_id: [u8; 32],
    wallet_a: [u8; 32],
    wallet_b: [u8; 32],
    wallet_c: [u8; 32],
) -> FlowEvents {
    let rfid_a = hash_canonical_rfid(&canonical_rfid_from_u64(0x8000_1300_0000_0001));
    let rfid_b = hash_canonical_rfid(&canonical_rfid_from_u64(0x8000_1300_0000_0002));
    let origin = StationEvent {
        action: Action::Origin,
        deployment_id,
        animal_id,
        station_id,
        event_sequence: 1,
        identity_revision: 1,
        previous_event_hash: [0; 32],
        old_rfid_hash: [0; 32],
        new_rfid_hash: rfid_a,
        from_custodian: [0; 32],
        to_custodian: wallet_a,
    };
    let transfer_ab = StationEvent {
        action: Action::Transfer,
        deployment_id,
        animal_id,
        station_id,
        event_sequence: 2,
        identity_revision: 1,
        previous_event_hash: origin.event_hash(),
        old_rfid_hash: rfid_a,
        new_rfid_hash: rfid_a,
        from_custodian: wallet_a,
        to_custodian: wallet_b,
    };
    let reidentify = StationEvent {
        action: Action::Reidentify,
        deployment_id,
        animal_id,
        station_id,
        event_sequence: 3,
        identity_revision: 2,
        previous_event_hash: transfer_ab.event_hash(),
        old_rfid_hash: rfid_a,
        new_rfid_hash: rfid_b,
        from_custodian: wallet_b,
        to_custodian: wallet_b,
    };
    let transfer_bc = StationEvent {
        action: Action::Transfer,
        deployment_id,
        animal_id,
        station_id,
        event_sequence: 4,
        identity_revision: 2,
        previous_event_hash: reidentify.event_hash(),
        old_rfid_hash: rfid_b,
        new_rfid_hash: rfid_b,
        from_custodian: wallet_b,
        to_custodian: wallet_c,
    };
    FlowEvents {
        animal_id,
        rfid_a,
        rfid_b,
        origin,
        transfer_ab,
        reidentify,
        transfer_bc,
    }
}

pub fn advance_to_transfer(harness: &mut Harness, flow: &FlowEvents) {
    assert_success(send_event(
        &mut harness.svm,
        &flow.origin,
        &harness.station_signing_key,
        &harness.station_pubkey33,
        &harness.wallet_a,
    ));
    harness.svm.expire_blockhash();
    assert_success(send_event(
        &mut harness.svm,
        &flow.transfer_ab,
        &harness.station_signing_key,
        &harness.station_pubkey33,
        &harness.wallet_a,
    ));
}

pub fn advance_to_reidentify(harness: &mut Harness, flow: &FlowEvents) {
    advance_to_transfer(harness, flow);
    harness.svm.expire_blockhash();
    assert_success(send_event(
        &mut harness.svm,
        &flow.reidentify,
        &harness.station_signing_key,
        &harness.station_pubkey33,
        &harness.wallet_b,
    ));
}

pub fn advance_full_flow(harness: &mut Harness, flow: &FlowEvents) {
    advance_to_reidentify(harness, flow);
    harness.svm.expire_blockhash();
    assert_success(send_event(
        &mut harness.svm,
        &flow.transfer_bc,
        &harness.station_signing_key,
        &harness.station_pubkey33,
        &harness.wallet_b,
    ));
}
