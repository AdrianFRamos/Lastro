//! Handling-session transactions (SPEC v2.1, S3/S4a): `open_session` and `revoke_session`
//! signed by the custodian, `anchor_observation_batch` signed by the custodian (or the
//! deployment authority relaying for it). A separate `payer` funds the accounts created.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

use crate::{
    error::ApiError,
    model::{AccountMetaDto, InstructionDto, TransactionDataResponse},
    solana::v2_transaction_builder::{
        finish, v2_facility_address, v2_protocol_config_address, v2_station_address,
        v2_station_registry_address,
    },
};

const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const SESSION_SEED: &[u8] = b"session";
const BATCH_SEED: &[u8] = b"batch";

pub fn v2_session_address(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    session_id: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SESSION_SEED, deployment_id, session_id], program_id)
}

pub fn v2_batch_address(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    session_id: &[u8; 32],
    batch_index: u32,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            BATCH_SEED,
            deployment_id,
            session_id,
            &batch_index.to_le_bytes(),
        ],
        program_id,
    )
}

/// Terms of an `open_session` transaction.
#[derive(Clone, Copy, Debug)]
pub struct SessionTerms {
    pub session_id: [u8; 32],
    pub station_id: [u8; 32],
    /// Facility the Station is installed at; the program checks it is the Station's.
    pub facility_id: [u8; 32],
    pub allowed_actions: u16,
    pub max_events: u32,
    pub valid_from: i64,
    pub expires_at: i64,
}

pub fn build_open_session(
    program_id: Pubkey,
    deployment_id: &[u8; 32],
    custodian: Pubkey,
    payer: Pubkey,
    terms: SessionTerms,
) -> Result<TransactionDataResponse, ApiError> {
    let mut data = discriminator("open_session").to_vec();
    data.extend_from_slice(&terms.session_id);
    data.extend_from_slice(&terms.station_id);
    data.extend_from_slice(&terms.allowed_actions.to_le_bytes());
    data.extend_from_slice(&terms.max_events.to_le_bytes());
    data.extend_from_slice(&terms.valid_from.to_le_bytes());
    data.extend_from_slice(&terms.expires_at.to_le_bytes());
    let accounts = vec![
        meta(custodian, true, false),
        meta(payer, true, true),
        meta(
            v2_protocol_config_address(&program_id, deployment_id).0,
            false,
            false,
        ),
        meta(
            v2_station_registry_address(&program_id, deployment_id).0,
            false,
            false,
        ),
        meta(
            v2_station_address(&program_id, deployment_id, &terms.station_id).0,
            false,
            false,
        ),
        meta(
            v2_facility_address(&program_id, deployment_id, &terms.facility_id).0,
            false,
            false,
        ),
        meta(
            v2_session_address(&program_id, deployment_id, &terms.session_id).0,
            false,
            true,
        ),
        system_program(),
    ];
    single(
        program_id,
        custodian,
        payer,
        accounts,
        data,
        "v2 session opening",
    )
}

pub fn build_revoke_session(
    program_id: Pubkey,
    deployment_id: &[u8; 32],
    session_id: &[u8; 32],
    custodian: Pubkey,
    fee_payer: Pubkey,
) -> Result<TransactionDataResponse, ApiError> {
    let accounts = vec![
        meta(custodian, true, false),
        meta(
            v2_protocol_config_address(&program_id, deployment_id).0,
            false,
            false,
        ),
        meta(
            v2_session_address(&program_id, deployment_id, session_id).0,
            false,
            true,
        ),
    ];
    single(
        program_id,
        custodian,
        fee_payer,
        accounts,
        discriminator("revoke_session").to_vec(),
        "v2 session revocation",
    )
}

/// One anchored batch: its index is the session's `batch_count` when it is anchored.
#[derive(Clone, Copy, Debug)]
pub struct BatchTerms {
    pub session_id: [u8; 32],
    /// The session's Station; the program requires it ACTIVE when the batch is anchored.
    pub station_id: [u8; 32],
    pub batch_index: u32,
    pub merkle_root: [u8; 32],
    pub leaf_count: u32,
    pub first_observed_at: i64,
    pub last_observed_at: i64,
}

pub fn build_anchor_batch(
    program_id: Pubkey,
    deployment_id: &[u8; 32],
    relayer: Pubkey,
    payer: Pubkey,
    terms: BatchTerms,
) -> Result<TransactionDataResponse, ApiError> {
    let mut data = discriminator("anchor_observation_batch").to_vec();
    data.extend_from_slice(&terms.merkle_root);
    data.extend_from_slice(&terms.leaf_count.to_le_bytes());
    data.extend_from_slice(&terms.first_observed_at.to_le_bytes());
    data.extend_from_slice(&terms.last_observed_at.to_le_bytes());
    let accounts = vec![
        meta(relayer, true, false),
        meta(payer, true, true),
        meta(
            v2_protocol_config_address(&program_id, deployment_id).0,
            false,
            false,
        ),
        meta(
            v2_session_address(&program_id, deployment_id, &terms.session_id).0,
            false,
            true,
        ),
        meta(
            v2_station_address(&program_id, deployment_id, &terms.station_id).0,
            false,
            false,
        ),
        meta(
            v2_batch_address(
                &program_id,
                deployment_id,
                &terms.session_id,
                terms.batch_index,
            )
            .0,
            false,
            true,
        ),
        system_program(),
    ];
    single(
        program_id,
        relayer,
        payer,
        accounts,
        data,
        "v2 observation batch",
    )
}

fn single(
    program_id: Pubkey,
    signer: Pubkey,
    fee_payer: Pubkey,
    accounts: Vec<AccountMetaDto>,
    data: Vec<u8>,
    label: &str,
) -> Result<TransactionDataResponse, ApiError> {
    finish(
        program_id,
        signer,
        fee_payer,
        vec![InstructionDto {
            program_id: program_id.to_string(),
            accounts,
            data_base64: BASE64_STANDARD.encode(data),
        }],
        label,
    )
}

fn system_program() -> AccountMetaDto {
    AccountMetaDto {
        address: SYSTEM_PROGRAM_ID.into(),
        is_signer: false,
        is_writable: false,
    }
}

fn meta(address: Pubkey, is_signer: bool, is_writable: bool) -> AccountMetaDto {
    AccountMetaDto {
        address: address.to_string(),
        is_signer,
        is_writable,
    }
}

pub(crate) fn discriminator(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("global:{name}").as_bytes());
    digest[..8]
        .try_into()
        .expect("SHA-256 output always contains eight bytes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_transactions_carry_the_exact_terms_and_fit_the_size_limit() {
        // PURPOSE: the wallet signs exactly the session terms the program checks.
        // FAILURE MEANS: a session could open with terms other than those prepared.
        let program_id = Pubkey::new_unique();
        let custodian = Pubkey::new_unique();
        let sponsor = Pubkey::new_unique();
        let terms = SessionTerms {
            session_id: [1; 32],
            station_id: [2; 32],
            facility_id: [5; 32],
            allowed_actions: 1,
            max_events: 500,
            valid_from: 10,
            expires_at: 20,
        };
        let open = build_open_session(program_id, &[3; 32], custodian, sponsor, terms).unwrap();
        let data = BASE64_STANDARD
            .decode(&open.instructions[0].data_base64)
            .unwrap();
        assert_eq!(data.len(), 8 + 32 + 32 + 2 + 4 + 8 + 8);
        assert_eq!(&data[..8], &discriminator("open_session"));
        assert_eq!(&data[74..78], &500u32.to_le_bytes());
        assert_eq!(open.fee_payer, sponsor.to_string());
        assert!(!open.instructions[0].accounts[0].is_writable);
        assert!(open.measured_serialized_bytes <= 1232);

        let batch = build_anchor_batch(
            program_id,
            &[3; 32],
            custodian,
            sponsor,
            BatchTerms {
                session_id: [1; 32],
                station_id: [2; 32],
                batch_index: 2,
                merkle_root: [4; 32],
                leaf_count: 300,
                first_observed_at: 11,
                last_observed_at: 19,
            },
        )
        .unwrap();
        assert_eq!(
            batch.instructions[0].accounts[4].address,
            v2_station_address(&program_id, &[3; 32], &[2; 32])
                .0
                .to_string(),
            "the program checks the Station is still ACTIVE"
        );
        assert_eq!(
            batch.instructions[0].accounts[5].address,
            v2_batch_address(&program_id, &[3; 32], &[1; 32], 2)
                .0
                .to_string()
        );
        assert!(batch.measured_serialized_bytes <= 1232);
    }
}
