//! Deployment-authority registry transactions for S5: `register_facility` (with the
//! boundary's `geo_commitment`) and `set_station_facility`. Signed by the authority wallet.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use solana_pubkey::Pubkey;

use crate::{
    error::ApiError,
    model::{AccountMetaDto, InstructionDto, TransactionDataResponse},
    solana::{
        v2_session::discriminator,
        v2_transaction_builder::{
            finish, v2_facility_address, v2_protocol_config_address, v2_station_address,
        },
    },
};

const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const FACILITY_REGISTRY_SEED: &[u8] = b"facility-registry";

pub fn v2_facility_registry_address(program_id: &Pubkey, deployment_id: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[FACILITY_REGISTRY_SEED, deployment_id], program_id)
}

/// Terms of one on-chain facility registration.
#[derive(Clone, Copy, Debug)]
pub struct FacilityTerms {
    pub facility_id: [u8; 32],
    pub owner: Pubkey,
    pub facility_type: u8,
    pub credential_hash: [u8; 32],
    pub valid_from: i64,
    pub valid_until: i64,
    pub geo_commitment: [u8; 32],
}

pub fn build_register_facility(
    program_id: Pubkey,
    deployment_id: &[u8; 32],
    authority: Pubkey,
    terms: FacilityTerms,
) -> Result<TransactionDataResponse, ApiError> {
    let mut data = discriminator("register_facility").to_vec();
    data.extend_from_slice(&terms.facility_id);
    data.extend_from_slice(terms.owner.as_ref());
    data.push(terms.facility_type);
    data.extend_from_slice(&terms.credential_hash);
    data.extend_from_slice(&terms.valid_from.to_le_bytes());
    data.extend_from_slice(&terms.valid_until.to_le_bytes());
    data.extend_from_slice(&terms.geo_commitment);
    let accounts = vec![
        meta(authority, true, true),
        meta(
            v2_protocol_config_address(&program_id, deployment_id).0,
            false,
            false,
        ),
        meta(
            v2_facility_registry_address(&program_id, deployment_id).0,
            false,
            false,
        ),
        meta(
            v2_facility_address(&program_id, deployment_id, &terms.facility_id).0,
            false,
            true,
        ),
        AccountMetaDto {
            address: SYSTEM_PROGRAM_ID.into(),
            is_signer: false,
            is_writable: false,
        },
    ];
    single(
        program_id,
        authority,
        accounts,
        data,
        "v2 facility registration",
    )
}

pub fn build_set_station_facility(
    program_id: Pubkey,
    deployment_id: &[u8; 32],
    authority: Pubkey,
    station_id: [u8; 32],
    facility_id: [u8; 32],
) -> Result<TransactionDataResponse, ApiError> {
    let accounts = vec![
        meta(authority, true, false),
        meta(
            v2_protocol_config_address(&program_id, deployment_id).0,
            false,
            false,
        ),
        meta(
            v2_station_address(&program_id, deployment_id, &station_id).0,
            false,
            true,
        ),
        meta(
            v2_facility_address(&program_id, deployment_id, &facility_id).0,
            false,
            false,
        ),
    ];
    single(
        program_id,
        authority,
        accounts,
        discriminator("set_station_facility").to_vec(),
        "v2 Station installation",
    )
}

fn single(
    program_id: Pubkey,
    signer: Pubkey,
    accounts: Vec<AccountMetaDto>,
    data: Vec<u8>,
    label: &str,
) -> Result<TransactionDataResponse, ApiError> {
    finish(
        program_id,
        signer,
        signer,
        vec![InstructionDto {
            program_id: program_id.to_string(),
            accounts,
            data_base64: BASE64_STANDARD.encode(data),
        }],
        label,
    )
}

fn meta(address: Pubkey, is_signer: bool, is_writable: bool) -> AccountMetaDto {
    AccountMetaDto {
        address: address.to_string(),
        is_signer,
        is_writable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facility_registration_carries_the_geo_commitment_last() {
        // PURPOSE: the authority signs exactly the boundary commitment the API computed.
        // FAILURE MEANS: a facility could be registered with another polygon than shown.
        let tx = build_register_facility(
            Pubkey::new_unique(),
            &[1; 32],
            Pubkey::new_unique(),
            FacilityTerms {
                facility_id: [2; 32],
                owner: Pubkey::new_unique(),
                facility_type: 1,
                credential_hash: [3; 32],
                valid_from: 10,
                valid_until: 20,
                geo_commitment: [4; 32],
            },
        )
        .unwrap();
        let data = BASE64_STANDARD
            .decode(&tx.instructions[0].data_base64)
            .unwrap();
        assert_eq!(data.len(), 8 + 32 + 32 + 1 + 32 + 8 + 8 + 32);
        assert_eq!(&data[data.len() - 32..], &[4; 32]);
        assert!(tx.measured_serialized_bytes <= 1232);
    }
}
