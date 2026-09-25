#![allow(dead_code)]

use std::{env, path::PathBuf};

use anchor_lang::{AccountDeserialize, InstructionData};
use litesvm::{LiteSVM, types::TransactionMetadata};
use p256::ecdsa::SigningKey;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

pub const LAMPORTS_PER_TEST_WALLET: u64 = 10_000_000_000;
pub const COMPUTE_UNIT_LIMIT: u32 = 1_400_000;
pub const DEPLOYMENT_ID: [u8; 32] = [0xd2; 32];

pub type TestResult = Result<TransactionMetadata, Box<litesvm::types::FailedTransactionMetadata>>;

pub struct Harness {
    pub svm: LiteSVM,
    pub authority: Keypair,
    pub other: Keypair,
    pub deployment_id: [u8; 32],
}

impl Harness {
    pub fn new() -> Self {
        let mut svm = LiteSVM::new();
        svm.add_program_from_file(program_id(), program_so_path())
            .expect("lastro-v2 SBF must be built before LiteSVM tests");
        let authority = Keypair::new();
        let other = Keypair::new();
        for wallet in [&authority, &other] {
            svm.airdrop(&wallet.pubkey(), LAMPORTS_PER_TEST_WALLET)
                .expect("test wallet airdrop must succeed");
        }
        Self {
            svm,
            authority,
            other,
            deployment_id: DEPLOYMENT_ID,
        }
    }

    pub fn initialize(&mut self) {
        assert_success(send_initialize(
            &mut self.svm,
            &self.authority,
            self.deployment_id,
        ));
    }
}

pub fn program_id() -> Pubkey {
    Pubkey::new_from_array(lastro_v2::ID.to_bytes())
}

pub fn system_program_id() -> Pubkey {
    Pubkey::new_from_array(anchor_lang::system_program::ID.to_bytes())
}

pub fn config_pda(deployment_id: &[u8; 32]) -> (Pubkey, u8) {
    find_pda(&[lastro_v2::constants::CONFIG_V2_SEED, deployment_id])
}

pub fn station_registry_pda(deployment_id: &[u8; 32]) -> (Pubkey, u8) {
    find_pda(&[lastro_v2::constants::STATION_REGISTRY_SEED, deployment_id])
}

pub fn facility_registry_pda(deployment_id: &[u8; 32]) -> (Pubkey, u8) {
    find_pda(&[lastro_v2::constants::FACILITY_REGISTRY_SEED, deployment_id])
}

pub fn party_registry_pda(deployment_id: &[u8; 32]) -> (Pubkey, u8) {
    find_pda(&[lastro_v2::constants::PARTY_REGISTRY_SEED, deployment_id])
}

pub fn document_registry_pda(deployment_id: &[u8; 32]) -> (Pubkey, u8) {
    find_pda(&[lastro_v2::constants::DOCUMENT_REGISTRY_SEED, deployment_id])
}

pub fn asset_pda(deployment_id: &[u8; 32], asset_id: &[u8; 32]) -> (Pubkey, u8) {
    find_pda(&[lastro_v2::constants::ASSET_SEED, deployment_id, asset_id])
}

pub fn station_pda(deployment_id: &[u8; 32], station_id: &[u8; 32]) -> (Pubkey, u8) {
    find_pda(&[
        lastro_v2::constants::STATION_V2_SEED,
        deployment_id,
        station_id,
    ])
}

pub fn intent_pda(
    deployment_id: &[u8; 32],
    subject_id: &[u8; 32],
    intent_id: &[u8; 32],
) -> (Pubkey, u8) {
    find_pda(&[
        lastro_v2::constants::INTENT_SEED,
        deployment_id,
        subject_id,
        intent_id,
    ])
}

pub fn facility_pda(deployment_id: &[u8; 32], facility_id: &[u8; 32]) -> (Pubkey, u8) {
    find_pda(&[
        lastro_v2::constants::FACILITY_SEED,
        deployment_id,
        facility_id,
    ])
}

pub fn party_pda(deployment_id: &[u8; 32], party_id: &[u8; 32]) -> (Pubkey, u8) {
    find_pda(&[lastro_v2::constants::PARTY_SEED, deployment_id, party_id])
}

fn find_pda(seeds: &[&[u8]]) -> (Pubkey, u8) {
    let (pda, bump) = Pubkey::find_program_address(seeds, &program_id());
    (pda, bump)
}

pub fn send_initialize(
    svm: &mut LiteSVM,
    authority: &Keypair,
    deployment_id: [u8; 32],
) -> TestResult {
    let (config, _) = config_pda(&deployment_id);
    let (station_registry, _) = station_registry_pda(&deployment_id);
    let (facility_registry, _) = facility_registry_pda(&deployment_id);
    let (party_registry, _) = party_registry_pda(&deployment_id);
    let (document_registry, _) = document_registry_pda(&deployment_id);
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new(config, false),
            AccountMeta::new(station_registry, false),
            AccountMeta::new(facility_registry, false),
            AccountMeta::new(party_registry, false),
            AccountMeta::new(document_registry, false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::InitializeV2 {
            deployment_id,
            schema_version: lastro_v2::constants::SCHEMA_VERSION,
            max_asset_weight_grams: 2_000_000,
            mass_tolerance_basis_points: 500,
            max_event_age_seconds: 86_400,
        }
        .data(),
    };
    send_instructions(
        svm,
        vec![
            ComputeBudgetInstruction::set_compute_unit_limit(COMPUTE_UNIT_LIMIT),
            ix,
        ],
        authority,
    )
}

pub fn send_register_asset(
    svm: &mut LiteSVM,
    authority: &Keypair,
    deployment_id: [u8; 32],
    asset_id: [u8; 32],
    custodian: Pubkey,
    weight: u64,
) -> TestResult {
    let (config, _) = config_pda(&deployment_id);
    let (asset, _) = asset_pda(&deployment_id, &asset_id);
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(asset, false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::RegisterAsset {
            asset_id,
            asset_type: 1,
            custodian,
            parent_root: [0; 32],
            lineage_root: [asset_id[0]; 32],
            available_weight_grams: weight,
        }
        .data(),
    };
    send_instructions(svm, vec![ix], authority)
}

pub fn send_register_station(
    svm: &mut LiteSVM,
    authority: &Keypair,
    deployment_id: [u8; 32],
    station_id: [u8; 32],
    pubkey33: [u8; 33],
) -> TestResult {
    let (config, _) = config_pda(&deployment_id);
    let (station_registry, _) = station_registry_pda(&deployment_id);
    let (station, _) = station_pda(&deployment_id, &station_id);
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new_readonly(station_registry, false),
            AccountMeta::new(station, false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::RegisterStationV2 {
            station_id,
            key_id: [9; 32],
            pubkey33,
            valid_from: 1,
            valid_until: 86_400,
            firmware_hash: [8; 32],
        }
        .data(),
    };
    send_instructions(svm, vec![ix], authority)
}

pub fn send_create_intent(
    svm: &mut LiteSVM,
    actor: &Keypair,
    deployment_id: [u8; 32],
    subject_id: [u8; 32],
    intent_id: [u8; 32],
    expires_at: i64,
    payload_hash: [u8; 32],
) -> TestResult {
    let (config, _) = config_pda(&deployment_id);
    let (intent, _) = intent_pda(&deployment_id, &subject_id, &intent_id);
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(actor.pubkey(), true),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(intent, false),
            AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::CreateIntent {
            intent_id,
            subject_id,
            intent_type: 1,
            expected_state_version: 0,
            nonce: 1,
            expires_at,
            payload_hash,
        }
        .data(),
    };
    send_instructions(svm, vec![ix], actor)
}

pub fn send_cancel_intent(
    svm: &mut LiteSVM,
    actor: &Keypair,
    deployment_id: [u8; 32],
    subject_id: [u8; 32],
    intent_id: [u8; 32],
) -> TestResult {
    let (config, _) = config_pda(&deployment_id);
    let (intent, _) = intent_pda(&deployment_id, &subject_id, &intent_id);
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(actor.pubkey(), true),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(intent, false),
        ],
        data: lastro_v2::instruction::CancelIntent {}.data(),
    };
    send_instructions(svm, vec![ix], actor)
}

pub fn send_consume_intent(
    svm: &mut LiteSVM,
    actor: &Keypair,
    deployment_id: [u8; 32],
    subject_id: [u8; 32],
    intent_id: [u8; 32],
    payload_hash: [u8; 32],
) -> TestResult {
    let (config, _) = config_pda(&deployment_id);
    let (intent, _) = intent_pda(&deployment_id, &subject_id, &intent_id);
    let ix = Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new_readonly(actor.pubkey(), true),
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(intent, false),
        ],
        data: lastro_v2::instruction::ConsumeIntent {
            expected_state_version: 0,
            payload_hash,
        }
        .data(),
    };
    send_instructions(svm, vec![ix], actor)
}

pub fn send_instructions(
    svm: &mut LiteSVM,
    instructions: Vec<Instruction>,
    payer: &Keypair,
) -> TestResult {
    let message = Message::new(&instructions, Some(&payer.pubkey()));
    let tx = Transaction::new(&[payer], message, svm.latest_blockhash());
    svm.send_transaction(tx).map_err(Box::new)
}

pub fn signing_key() -> SigningKey {
    SigningKey::from_bytes((&[1u8; 32]).into()).expect("test P-256 key")
}

pub fn compressed_pubkey(key: &SigningKey) -> [u8; 33] {
    key.verifying_key()
        .to_encoded_point(true)
        .as_bytes()
        .try_into()
        .expect("compressed public key")
}

pub fn assert_success(result: TestResult) -> TransactionMetadata {
    match result {
        Ok(meta) => meta,
        Err(err) => panic!(
            "transaction unexpectedly failed: {:?}\n{}",
            err.err,
            err.meta.logs.join("\n")
        ),
    }
}

pub fn assert_failure(result: TestResult) {
    if let Ok(meta) = result {
        panic!("transaction unexpectedly succeeded: {}", meta.pretty_logs());
    }
}

pub fn account<T: AccountDeserialize>(svm: &LiteSVM, address: &Pubkey) -> T {
    let account = svm.get_account(address).expect("account must exist");
    let mut bytes = account.data.as_slice();
    T::try_deserialize(&mut bytes).expect("account must deserialize")
}

pub fn account_exists(svm: &LiteSVM, address: &Pubkey) -> bool {
    svm.get_account(address).is_some()
}

pub fn program_so_path() -> PathBuf {
    if let Some(path) = env::var_os("LASTRO_V2_PROGRAM_SO") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/deploy/lastro-v2.so")
}
