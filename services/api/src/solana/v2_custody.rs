//! Two-phase v2 custody transfer transactions.
//!
//! Phase 1 (`create_intent`, type CUSTODY_TRANSFER) is signed by the current on-chain
//! custodian and commits to the recipient wallet. Phase 2 (`accept_custody_transfer`) is
//! signed by the recipient. The API only prepares descriptors; wallets sign, and the
//! projection is updated after the finalized chain state shows the new custodian.
//! A `payer` (the custodian itself or the fee sponsor) funds the intent; acceptance closes it
//! and returns that rent to the payer the intent recorded.

use lastro_protocol::v2::custody_transfer_payload_hash;
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;
use uuid::Uuid;

use crate::{
    error::ApiError,
    model::{AccountMetaDto, InstructionDto, TransactionDataResponse},
    solana::v2_transaction_builder::{finish, v2_asset_address, v2_protocol_config_address},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
const INTENT_SEED: &[u8] = b"intent";
const INTENT_TYPE_CUSTODY_TRANSFER: u16 = 2;
const CUSTODY_INTENT_DOMAIN: &[u8] = b"LASTRO_V2_CUSTODY_INTENT\0";

/// Deterministic, so both phases (and retries) address the same on-chain intent.
pub fn custody_intent_id(transfer_id: Uuid) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(CUSTODY_INTENT_DOMAIN);
    hasher.update(transfer_id.as_bytes());
    hasher.finalize().into()
}

pub fn custody_intent_nonce(transfer_id: Uuid) -> u64 {
    let bytes = transfer_id.as_bytes();
    u64::from_le_bytes(bytes[..8].try_into().expect("uuid has 16 bytes"))
}

pub fn v2_intent_address(
    program_id: &Pubkey,
    deployment_id: &[u8; 32],
    subject_id: &[u8; 32],
    intent_id: &[u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[INTENT_SEED, deployment_id, subject_id, intent_id],
        program_id,
    )
}

pub struct CustodyTransferPlan {
    pub program_id: Pubkey,
    pub deployment_id: [u8; 32],
    pub asset_id: [u8; 32],
    pub transfer_id: Uuid,
    pub recipient: Pubkey,
    /// The asset `state_version` the proposal is bound to.
    pub expected_state_version: u64,
}

impl CustodyTransferPlan {
    fn intent(&self) -> Pubkey {
        v2_intent_address(
            &self.program_id,
            &self.deployment_id,
            &self.asset_id,
            &custody_intent_id(self.transfer_id),
        )
        .0
    }

    /// Phase 1, signed by `current_custodian`; `payer` funds the intent and the fee.
    pub fn propose(
        &self,
        current_custodian: Pubkey,
        payer: Pubkey,
        expires_at: i64,
    ) -> Result<TransactionDataResponse, ApiError> {
        if current_custodian == self.recipient {
            return Err(ApiError::Conflict(
                "recipient already holds custody of this asset".into(),
            ));
        }
        let nonce = custody_intent_nonce(self.transfer_id);
        let payload = custody_transfer_payload_hash(
            self.deployment_id,
            self.asset_id,
            self.recipient.to_bytes(),
            self.expected_state_version,
            nonce,
        );
        let mut data = discriminator("create_intent").to_vec();
        data.extend_from_slice(&custody_intent_id(self.transfer_id));
        data.extend_from_slice(&self.asset_id);
        data.extend_from_slice(&INTENT_TYPE_CUSTODY_TRANSFER.to_le_bytes());
        data.extend_from_slice(&self.expected_state_version.to_le_bytes());
        data.extend_from_slice(&nonce.to_le_bytes());
        data.extend_from_slice(&expires_at.to_le_bytes());
        data.extend_from_slice(&payload);
        let accounts = vec![
            meta(current_custodian, true, false),
            meta(payer, true, true),
            meta(self.config(), false, false),
            meta(self.asset(), false, false),
            meta(self.intent(), false, true),
            AccountMetaDto {
                address: SYSTEM_PROGRAM_ID.into(),
                is_signer: false,
                is_writable: false,
            },
        ];
        self.single(current_custodian, payer, accounts, data)
    }

    /// Phase 2, signed by the recipient wallet; `fee_payer` pays the network fee and
    /// `rent_payer` (recorded in the intent at proposal) receives the closed intent's rent.
    pub fn accept(
        &self,
        fee_payer: Pubkey,
        rent_payer: Pubkey,
    ) -> Result<TransactionDataResponse, ApiError> {
        let mut data = discriminator("accept_custody_transfer").to_vec();
        data.extend_from_slice(&self.expected_state_version.to_le_bytes());
        let accounts = vec![
            meta(self.recipient, true, false),
            meta(self.config(), false, false),
            meta(self.asset(), false, true),
            meta(self.intent(), false, true),
            meta(rent_payer, false, true),
        ];
        self.single(self.recipient, fee_payer, accounts, data)
    }

    fn config(&self) -> Pubkey {
        v2_protocol_config_address(&self.program_id, &self.deployment_id).0
    }

    fn asset(&self) -> Pubkey {
        v2_asset_address(&self.program_id, &self.deployment_id, &self.asset_id).0
    }

    fn single(
        &self,
        signer: Pubkey,
        fee_payer: Pubkey,
        accounts: Vec<AccountMetaDto>,
        data: Vec<u8>,
    ) -> Result<TransactionDataResponse, ApiError> {
        let instructions = vec![InstructionDto {
            program_id: self.program_id.to_string(),
            accounts,
            data_base64: BASE64_STANDARD.encode(data),
        }];
        finish(
            self.program_id,
            signer,
            fee_payer,
            instructions,
            "v2 custody transfer",
        )
    }
}

fn discriminator(name: &str) -> [u8; 8] {
    let digest = Sha256::digest(format!("global:{name}").as_bytes());
    digest[..8].try_into().expect("SHA-256 has 32 bytes")
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

    fn plan() -> CustodyTransferPlan {
        CustodyTransferPlan {
            program_id: Pubkey::new_unique(),
            deployment_id: [1; 32],
            asset_id: [2; 32],
            transfer_id: Uuid::from_bytes([3; 16]),
            recipient: Pubkey::new_unique(),
            expected_state_version: 4,
        }
    }

    #[test]
    fn both_phases_address_the_same_intent_and_bind_the_recipient() {
        // PURPOSE: the recipient can only accept the exact proposal the custodian signed.
        // ASSERT: propose/accept share the intent PDA; the proposal payload commits to the
        // recipient, version and nonce; signers are custodian then recipient.
        // FAILURE MEANS: a proposal could be accepted by another wallet or never be found.
        let plan = plan();
        let custodian = Pubkey::new_unique();
        let sponsor = Pubkey::new_unique();
        let propose = plan.propose(custodian, sponsor, 99).unwrap();
        let accept = plan.accept(sponsor, sponsor).unwrap();
        assert_eq!(propose.required_signer, custodian.to_string());
        assert_eq!(propose.fee_payer, sponsor.to_string());
        assert_eq!(accept.required_signer, plan.recipient.to_string());
        assert_eq!(
            propose.instructions[0].accounts[4].address,
            accept.instructions[0].accounts[3].address
        );
        // The proposer authorizes read-only; the payer funds; acceptance refunds the payer.
        assert!(!propose.instructions[0].accounts[0].is_writable);
        assert!(propose.instructions[0].accounts[1].is_writable);
        assert_eq!(
            accept.instructions[0].accounts[4].address,
            sponsor.to_string()
        );

        let data = BASE64_STANDARD
            .decode(&propose.instructions[0].data_base64)
            .unwrap();
        assert_eq!(&data[..8], &discriminator("create_intent"));
        assert_eq!(data.len(), 8 + 32 + 32 + 2 + 8 + 8 + 8 + 32);
        let expected_payload = custody_transfer_payload_hash(
            plan.deployment_id,
            plan.asset_id,
            plan.recipient.to_bytes(),
            4,
            custody_intent_nonce(plan.transfer_id),
        );
        assert_eq!(&data[data.len() - 32..], &expected_payload);
        assert!(propose.measured_serialized_bytes < 1232);

        assert!(plan.propose(plan.recipient, sponsor, 99).is_err());
    }
}
