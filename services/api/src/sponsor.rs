//! Platform fee sponsorship (SPEC v2.1, S1).
//!
//! The sponsor is a keypair that pays network fees and the rent of accounts the Lastro program
//! creates, so a custodian wallet with zero lamports can still authorize transitions. It never
//! authorizes anything: the program checks authority on the separate signer, and this module
//! refuses to co-sign any transaction that is not byte-for-byte what the API prepared.
//!
//! Without `LASTRO_SPONSOR_KEYPAIR_PATH` the deployment is self-paid and nothing here runs.

use std::path::Path;

use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use serde_json::{Value, json};
use solana_pubkey::Pubkey;

use crate::{
    error::ApiError,
    model::{InstructionDto, TransactionDataResponse},
    solana::{secp256r1::SECP256R1_PROGRAM_ID, v2_transaction_builder::COMPUTE_BUDGET_PROGRAM_ID},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

/// Base fee per signature, including each Secp256r1 precompile signature.
pub const LAMPORTS_PER_SIGNATURE: u64 = 5_000;
/// Rent-exempt minimum = (data + 128 account overhead) x 3_480 lamports/byte-year x 2 years.
const RENT_LAMPORTS_PER_BYTE: u64 = 6_960;
const ACCOUNT_STORAGE_OVERHEAD: u64 = 128;
/// Compute units charged when no explicit limit instruction is present.
const DEFAULT_COMPUTE_UNIT_LIMIT: u64 = 200_000;

/// Accounts created by each sponsorable Lastro instruction, by data size (v2.1 layouts).
const ASSET_STATE_SPACE: u64 = 341;
const EVENT_ANCHOR_SPACE: u64 = 83;
const RFID_BINDING_SPACE: u64 = 74;
const INTENT_STATE_SPACE: u64 = 204;
const SESSION_GRANT_SPACE: u64 = 176;
const BATCH_ANCHOR_SPACE: u64 = 169;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SponsorLimits {
    /// Fee + rent one transaction may cost the sponsor.
    pub max_lamports_per_transaction: u64,
    /// Rolling 24 h budget per authorizing signer (organization wallet).
    pub daily_lamports_per_signer: u64,
    /// Rolling 24 h budget for the whole deployment.
    pub daily_lamports_total: u64,
}

impl SponsorLimits {
    pub const DEFAULT_MAX_LAMPORTS_PER_TRANSACTION: u64 = 10_000_000;
    pub const DEFAULT_DAILY_LAMPORTS_PER_SIGNER: u64 = 1_000_000_000;
    pub const DEFAULT_DAILY_LAMPORTS_TOTAL: u64 = 10_000_000_000;
}

impl Default for SponsorLimits {
    fn default() -> Self {
        Self {
            max_lamports_per_transaction: Self::DEFAULT_MAX_LAMPORTS_PER_TRANSACTION,
            daily_lamports_per_signer: Self::DEFAULT_DAILY_LAMPORTS_PER_SIGNER,
            daily_lamports_total: Self::DEFAULT_DAILY_LAMPORTS_TOTAL,
        }
    }
}

pub struct Sponsor {
    key: SigningKey,
    pubkey: Pubkey,
    pub limits: SponsorLimits,
}

impl Sponsor {
    /// Loads a Solana CLI keypair file (JSON array of 64 bytes: secret then public key).
    pub fn from_keypair_file(path: &Path, limits: SponsorLimits) -> Result<Self, ApiError> {
        let invalid = || {
            ApiError::Config(
                "LASTRO_SPONSOR_KEYPAIR_PATH must point to a Solana keypair JSON file".into(),
            )
        };
        let text = std::fs::read_to_string(path).map_err(|_| invalid())?;
        let bytes: Vec<u8> = serde_json::from_str(&text).map_err(|_| invalid())?;
        let bytes: [u8; 64] = bytes.try_into().map_err(|_| invalid())?;
        Self::from_keypair_bytes(&bytes, limits)
    }

    pub fn from_keypair_bytes(bytes: &[u8; 64], limits: SponsorLimits) -> Result<Self, ApiError> {
        let secret: [u8; 32] = bytes[..32].try_into().map_err(|_| ApiError::Internal)?;
        let key = SigningKey::from_bytes(&secret);
        if key.verifying_key().as_bytes() != &bytes[32..] {
            return Err(ApiError::Config(
                "sponsor keypair public half does not match its secret".into(),
            ));
        }
        let pubkey = Pubkey::new_from_array(key.verifying_key().to_bytes());
        Ok(Self {
            key,
            pubkey,
            limits,
        })
    }

    pub fn pubkey(&self) -> Pubkey {
        self.pubkey
    }

    fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.key.sign(message).to_bytes()
    }
}

/// Fee payer for a transaction `signer` authorizes: the sponsor, unless there is none or the
/// signer is the sponsor itself (a sponsor never sponsors its own key into an authority).
pub fn payer_for(sponsor: Option<&Sponsor>, signer: Pubkey) -> Pubkey {
    sponsor
        .map(Sponsor::pubkey)
        .filter(|payer| *payer != signer)
        .unwrap_or(signer)
}

/// Every fee payer the API accepts for a transaction `signer` authorized: the current sponsor
/// first, then the signer itself (a self-paid transaction remains valid if sponsorship was
/// unavailable or changed between preparation and confirmation).
pub fn payer_candidates(sponsor: Option<&Sponsor>, signer: Pubkey) -> Vec<Pubkey> {
    let mut payers = vec![payer_for(sponsor, signer)];
    if payers[0] != signer {
        payers.push(signer);
    }
    payers
}

fn rent(space: u64) -> u64 {
    (space + ACCOUNT_STORAGE_OVERHEAD) * RENT_LAMPORTS_PER_BYTE
}

/// Upper bound of what one prepared transaction costs its fee payer: one base fee per
/// signature (Secp256r1 signatures included), the priority fee, and the rent of every account
/// the Lastro instruction creates. Unknown Lastro instructions are not sponsorable.
pub fn estimate_cost_lamports(
    expected: &TransactionDataResponse,
    discriminators: &SponsorableDiscriminators,
) -> Result<u64, ApiError> {
    let mut signers = std::collections::HashSet::new();
    signers.insert(expected.fee_payer.as_str());
    let mut precompile_signatures = 0u64;
    let mut created_rent = 0u64;
    let mut unit_limit = DEFAULT_COMPUTE_UNIT_LIMIT;
    let mut unit_price = 0u64;
    for instruction in &expected.instructions {
        let data = BASE64_STANDARD
            .decode(&instruction.data_base64)
            .map_err(|_| ApiError::Internal)?;
        for account in &instruction.accounts {
            if account.is_signer {
                signers.insert(account.address.as_str());
            }
        }
        if instruction.program_id == SECP256R1_PROGRAM_ID {
            precompile_signatures += u64::from(*data.first().ok_or(ApiError::Internal)?);
        } else if instruction.program_id == COMPUTE_BUDGET_PROGRAM_ID {
            match (data.first(), data.len()) {
                (Some(2), 5) => {
                    unit_limit = u64::from(u32::from_le_bytes(
                        data[1..5].try_into().map_err(|_| ApiError::Internal)?,
                    ));
                }
                (Some(3), 9) => {
                    unit_price =
                        u64::from_le_bytes(data[1..9].try_into().map_err(|_| ApiError::Internal)?);
                }
                _ => return Err(ApiError::Internal),
            }
        } else if instruction.program_id == expected.lastro_program_id {
            created_rent += discriminators.created_rent(&data)?;
        } else {
            return Err(ApiError::Internal);
        }
    }
    let signature_count = u64::try_from(signers.len()).map_err(|_| ApiError::Internal)?;
    let priority_fee = (unit_limit * unit_price).div_ceil(1_000_000);
    Ok(
        (signature_count + precompile_signatures) * LAMPORTS_PER_SIGNATURE
            + priority_fee
            + created_rent,
    )
}

/// Discriminators of the Lastro instructions the sponsor may fund, with the rent each creates.
pub struct SponsorableDiscriminators {
    entries: Vec<([u8; 8], u64)>,
}

impl Default for SponsorableDiscriminators {
    fn default() -> Self {
        let entry = |name: &str, rent_lamports: u64| (discriminator(name), rent_lamports);
        Self {
            entries: vec![
                entry("register_asset", rent(ASSET_STATE_SPACE)),
                entry("record_observation", rent(EVENT_ANCHOR_SPACE)),
                entry(
                    "bind_identifier",
                    rent(EVENT_ANCHOR_SPACE) + rent(RFID_BINDING_SPACE),
                ),
                entry(
                    "replace_identifier",
                    rent(EVENT_ANCHOR_SPACE) + rent(RFID_BINDING_SPACE),
                ),
                entry("create_intent", rent(INTENT_STATE_SPACE)),
                entry("accept_custody_transfer", 0),
                entry("open_session", rent(SESSION_GRANT_SPACE)),
                entry("anchor_observation_batch", rent(BATCH_ANCHOR_SPACE)),
                entry("revoke_session", 0),
            ],
        }
    }
}

impl SponsorableDiscriminators {
    fn created_rent(&self, data: &[u8]) -> Result<u64, ApiError> {
        let prefix = data.get(..8).ok_or(ApiError::Internal)?;
        self.entries
            .iter()
            .find(|(discriminator, _)| discriminator.as_slice() == prefix)
            .map(|(_, rent)| *rent)
            .ok_or_else(|| ApiError::Conflict("this Lastro instruction is not sponsorable".into()))
    }
}

fn discriminator(name: &str) -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("global:{name}").as_bytes());
    digest[..8]
        .try_into()
        .expect("SHA-256 output always contains eight bytes")
}

/// A legacy wire transaction split into its signature slots and message.
#[derive(Debug)]
pub struct LegacyTransaction {
    pub signatures: Vec<[u8; 64]>,
    pub message: Vec<u8>,
    pub num_required_signatures: u8,
    pub num_readonly_signed: u8,
    pub num_readonly_unsigned: u8,
    pub account_keys: Vec<Pubkey>,
    pub recent_blockhash: [u8; 32],
    pub instructions: Vec<CompiledInstruction>,
}

#[derive(Debug)]
pub struct CompiledInstruction {
    pub program_id_index: u8,
    pub accounts: Vec<u8>,
    pub data: Vec<u8>,
}

impl LegacyTransaction {
    pub fn decode(wire: &[u8]) -> Result<Self, ApiError> {
        let invalid =
            || ApiError::Validation("transaction is not a valid legacy transaction".into());
        let mut reader = Reader { bytes: wire, at: 0 };
        let signature_count = reader.shortvec().ok_or_else(invalid)?;
        let mut signatures = Vec::with_capacity(signature_count);
        for _ in 0..signature_count {
            signatures.push(reader.array::<64>().ok_or_else(invalid)?);
        }
        let message = wire.get(reader.at..).ok_or_else(invalid)?.to_vec();
        let header = reader.array::<3>().ok_or_else(invalid)?;
        // Versioned messages set the top bit of the first header byte.
        if header[0] & 0x80 != 0 || usize::from(header[0]) != signature_count {
            return Err(invalid());
        }
        let key_count = reader.shortvec().ok_or_else(invalid)?;
        let mut account_keys = Vec::with_capacity(key_count);
        for _ in 0..key_count {
            account_keys.push(Pubkey::new_from_array(
                reader.array::<32>().ok_or_else(invalid)?,
            ));
        }
        let recent_blockhash = reader.array::<32>().ok_or_else(invalid)?;
        let instruction_count = reader.shortvec().ok_or_else(invalid)?;
        let mut instructions = Vec::with_capacity(instruction_count);
        for _ in 0..instruction_count {
            let program_id_index = reader.array::<1>().ok_or_else(invalid)?[0];
            let account_count = reader.shortvec().ok_or_else(invalid)?;
            let accounts = reader.take(account_count).ok_or_else(invalid)?.to_vec();
            let data_len = reader.shortvec().ok_or_else(invalid)?;
            let data = reader.take(data_len).ok_or_else(invalid)?.to_vec();
            instructions.push(CompiledInstruction {
                program_id_index,
                accounts,
                data,
            });
        }
        if reader.at != wire.len()
            || usize::from(header[0]) > account_keys.len()
            || instructions.iter().any(|instruction| {
                usize::from(instruction.program_id_index) >= account_keys.len()
                    || instruction
                        .accounts
                        .iter()
                        .any(|index| usize::from(*index) >= account_keys.len())
            })
        {
            return Err(invalid());
        }
        Ok(Self {
            signatures,
            message,
            num_required_signatures: header[0],
            num_readonly_signed: header[1],
            num_readonly_unsigned: header[2],
            account_keys,
            recent_blockhash,
            instructions,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut wire = Vec::with_capacity(1 + 64 * self.signatures.len() + self.message.len());
        push_shortvec(&mut wire, self.signatures.len());
        for signature in &self.signatures {
            wire.extend_from_slice(signature);
        }
        wire.extend_from_slice(&self.message);
        wire
    }

    /// The same shape `getTransaction` returns with `encoding: "json"`, so the API compares a
    /// not-yet-broadcast transaction with exactly the code it uses for finalized ones.
    pub fn to_rpc_json(&self) -> Value {
        json!({
            "meta": { "err": null },
            "transaction": {
                "signatures": self
                    .signatures
                    .iter()
                    .map(|signature| bs58::encode(signature).into_string())
                    .collect::<Vec<_>>(),
                "message": {
                    "header": {
                        "numRequiredSignatures": self.num_required_signatures,
                        "numReadonlySignedAccounts": self.num_readonly_signed,
                        "numReadonlyUnsignedAccounts": self.num_readonly_unsigned,
                    },
                    "accountKeys": self
                        .account_keys
                        .iter()
                        .map(Pubkey::to_string)
                        .collect::<Vec<_>>(),
                    "recentBlockhash": bs58::encode(self.recent_blockhash).into_string(),
                    "instructions": self
                        .instructions
                        .iter()
                        .map(|instruction| json!({
                            "programIdIndex": instruction.program_id_index,
                            "accounts": instruction.accounts,
                            "data": bs58::encode(&instruction.data).into_string(),
                        }))
                        .collect::<Vec<_>>(),
                },
            },
        })
    }
}

/// Compiles `expected` into an unsigned legacy transaction with `recent_blockhash`, the way a
/// wallet does: fee payer first, then writable signers, read-only signers, writable and
/// read-only accounts, each class in first-appearance order. Every signature slot is empty.
pub fn compile_unsigned(
    expected: &TransactionDataResponse,
    recent_blockhash: [u8; 32],
) -> Result<LegacyTransaction, ApiError> {
    let parse = |address: &str| -> Result<Pubkey, ApiError> {
        address.parse().map_err(|_| ApiError::Internal)
    };
    // (address, signer, writable) in first-appearance order, fee payer first.
    let mut roles: Vec<(Pubkey, bool, bool)> = vec![(parse(&expected.fee_payer)?, true, true)];
    let mut upsert = |key: Pubkey, signer: bool, writable: bool| {
        if let Some(role) = roles.iter_mut().find(|role| role.0 == key) {
            role.1 |= signer;
            role.2 |= writable;
        } else {
            roles.push((key, signer, writable));
        }
    };
    for instruction in &expected.instructions {
        for account in &instruction.accounts {
            upsert(
                parse(&account.address)?,
                account.is_signer,
                account.is_writable,
            );
        }
        upsert(parse(&instruction.program_id)?, false, false);
    }
    let class = |role: &(Pubkey, bool, bool)| match (role.1, role.2) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    };
    let payer = roles.remove(0);
    roles.sort_by_key(class);
    roles.insert(0, payer);
    let count = |wanted: usize| roles.iter().filter(|role| class(role) == wanted).count();
    let to_u8 = |value: usize| u8::try_from(value).map_err(|_| ApiError::Internal);
    let required = to_u8(count(0) + count(1))?;
    let readonly_signed = to_u8(count(1))?;
    let readonly_unsigned = to_u8(count(3))?;
    let account_keys: Vec<Pubkey> = roles.iter().map(|role| role.0).collect();
    let index = |key: &Pubkey| -> Result<u8, ApiError> {
        to_u8(
            account_keys
                .iter()
                .position(|candidate| candidate == key)
                .ok_or(ApiError::Internal)?,
        )
    };
    let mut instructions = Vec::with_capacity(expected.instructions.len());
    for instruction in &expected.instructions {
        instructions.push(CompiledInstruction {
            program_id_index: index(&parse(&instruction.program_id)?)?,
            accounts: instruction
                .accounts
                .iter()
                .map(|account| index(&parse(&account.address)?))
                .collect::<Result<_, _>>()?,
            data: BASE64_STANDARD
                .decode(&instruction.data_base64)
                .map_err(|_| ApiError::Internal)?,
        });
    }
    let mut message = vec![required, readonly_signed, readonly_unsigned];
    push_shortvec(&mut message, account_keys.len());
    for key in &account_keys {
        message.extend_from_slice(key.as_ref());
    }
    message.extend_from_slice(&recent_blockhash);
    push_shortvec(&mut message, instructions.len());
    for instruction in &instructions {
        message.push(instruction.program_id_index);
        push_shortvec(&mut message, instruction.accounts.len());
        message.extend_from_slice(&instruction.accounts);
        push_shortvec(&mut message, instruction.data.len());
        message.extend_from_slice(&instruction.data);
    }
    Ok(LegacyTransaction {
        signatures: vec![[0u8; 64]; usize::from(required)],
        message,
        num_required_signatures: required,
        num_readonly_signed: readonly_signed,
        num_readonly_unsigned: readonly_unsigned,
        account_keys,
        recent_blockhash,
        instructions,
    })
}

impl LegacyTransaction {
    /// Puts `signature` in the slot of `signer`; the signer must be required by the message.
    pub fn set_signature(&mut self, signer: &Pubkey, signature: [u8; 64]) -> Result<(), ApiError> {
        let slot = self
            .account_keys
            .iter()
            .take(usize::from(self.num_required_signatures))
            .position(|key| key == signer)
            .ok_or(ApiError::Internal)?;
        self.signatures[slot] = signature;
        Ok(())
    }
}

/// Validates a user-signed transaction against what the API prepared and returns it with the
/// sponsor's signature in slot 0, ready to broadcast.
///
/// Refuses unless: the sponsor is the fee payer and not the authorizing signer, the message is
/// exactly `expected` (ComputeBudget values included, so a caller cannot raise the priority
/// fee), every other signer has already produced a valid signature, and the sponsor slot is
/// still empty.
pub fn co_sign(
    sponsor: &Sponsor,
    wire: &[u8],
    expected: &TransactionDataResponse,
) -> Result<LegacyTransaction, ApiError> {
    let sponsor_key = sponsor.pubkey().to_string();
    if expected.fee_payer != sponsor_key || expected.required_signer == sponsor_key {
        return Err(ApiError::Conflict(
            "this transaction is not prepared for sponsorship".into(),
        ));
    }
    let mut transaction = LegacyTransaction::decode(wire)?;
    if transaction.account_keys.first() != Some(&sponsor.pubkey())
        || transaction.signatures.first() != Some(&[0u8; 64])
    {
        return Err(ApiError::Conflict(
            "the sponsor must be the unsigned fee payer of this transaction".into(),
        ));
    }
    if !crate::solana::rpc::transaction_matches_json(
        &transaction.to_rpc_json(),
        &bs58::encode([0u8; 64]).into_string(),
        expected,
    )? || !same_instruction_data(&transaction, &expected.instructions)
    {
        return Err(ApiError::Conflict(
            "transaction differs from the one the API prepared".into(),
        ));
    }
    for (index, signature) in transaction.signatures.iter().enumerate().skip(1) {
        let key = VerifyingKey::from_bytes(&transaction.account_keys[index].to_bytes())
            .map_err(|_| ApiError::Conflict("signer is not a valid Ed25519 key".into()))?;
        key.verify(&transaction.message, &Signature::from_bytes(signature))
            .map_err(|_| {
                ApiError::Conflict(
                    "the authorizing wallet has not signed this exact transaction".into(),
                )
            })?;
    }
    transaction.signatures[0] = sponsor.sign(&transaction.message);
    Ok(transaction)
}

/// `transaction_matches_json` tolerates any ComputeBudget values after the protocol
/// instructions because they change no Lastro state; the sponsor pays for them, so here every
/// instruction, ComputeBudget included, must carry exactly the prepared bytes.
fn same_instruction_data(transaction: &LegacyTransaction, expected: &[InstructionDto]) -> bool {
    transaction.instructions.len() == expected.len()
        && transaction
            .instructions
            .iter()
            .zip(expected)
            .all(|(actual, expected)| {
                BASE64_STANDARD
                    .decode(&expected.data_base64)
                    .is_ok_and(|data| data == actual.data)
                    && transaction.account_keys[usize::from(actual.program_id_index)].to_string()
                        == expected.program_id
            })
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, len: usize) -> Option<&[u8]> {
        let end = self.at.checked_add(len)?;
        let slice = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(slice)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    /// Solana compact-u16: at most three bytes, canonical encoding only.
    fn shortvec(&mut self) -> Option<usize> {
        let mut value = 0usize;
        for index in 0..3 {
            let byte = *self.take(1)?.first()?;
            value |= usize::from(byte & 0x7f) << (7 * index);
            if byte & 0x80 == 0 {
                if index > 0 && byte == 0 {
                    return None;
                }
                return (value <= usize::from(u16::MAX)).then_some(value);
            }
        }
        None
    }
}

fn push_shortvec(out: &mut Vec<u8>, mut value: usize) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        byte |= 0x80;
        out.push(byte);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AccountMetaDto, TransactionVersionDto};

    const PROGRAM: &str = "Vote111111111111111111111111111111111111111";

    fn keypair(seed: u8) -> [u8; 64] {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let mut bytes = [0u8; 64];
        bytes[..32].copy_from_slice(&[seed; 32]);
        bytes[32..].copy_from_slice(key.verifying_key().as_bytes());
        bytes
    }

    fn sponsor() -> Sponsor {
        Sponsor::from_keypair_bytes(&keypair(1), SponsorLimits::default()).unwrap()
    }

    fn user() -> SigningKey {
        SigningKey::from_bytes(&[2; 32])
    }

    fn user_pubkey() -> Pubkey {
        Pubkey::new_from_array(user().verifying_key().to_bytes())
    }

    /// `accept_custody_transfer`-shaped instruction: user is a read-only signer.
    fn expected(fee_payer: Pubkey) -> TransactionDataResponse {
        let user = user_pubkey().to_string();
        let mut data = discriminator("accept_custody_transfer").to_vec();
        data.extend_from_slice(&3u64.to_le_bytes());
        let instructions = vec![InstructionDto {
            program_id: PROGRAM.into(),
            accounts: vec![
                AccountMetaDto {
                    address: user.clone(),
                    is_signer: true,
                    is_writable: false,
                },
                AccountMetaDto {
                    address: Pubkey::new_from_array([9; 32]).to_string(),
                    is_signer: false,
                    is_writable: true,
                },
            ],
            data_base64: BASE64_STANDARD.encode(data),
        }];
        TransactionDataResponse {
            required_signer: user,
            fee_payer: fee_payer.to_string(),
            lastro_program_id: PROGRAM.into(),
            measured_serialized_bytes:
                crate::solana::v2_transaction_builder::measure_legacy_transaction(
                    &instructions,
                    &fee_payer,
                ),
            instructions,
            transaction_version: TransactionVersionDto::Legacy,
        }
    }

    /// Compiles `expected` the way a wallet would: fee payer, user signer, writable account,
    /// program; the user signs and the sponsor slot stays empty.
    fn wire(fee_payer: Pubkey, data_override: Option<Vec<u8>>, user_signs: bool) -> Vec<u8> {
        let mut data = discriminator("accept_custody_transfer").to_vec();
        data.extend_from_slice(&3u64.to_le_bytes());
        let data = data_override.unwrap_or(data);
        let mut message = vec![2u8, 1, 1];
        message.push(4);
        for key in [
            fee_payer,
            user_pubkey(),
            Pubkey::new_from_array([9; 32]),
            Pubkey::from_str_const(PROGRAM),
        ] {
            message.extend_from_slice(key.as_ref());
        }
        message.extend_from_slice(&[7; 32]);
        message.push(1);
        message.push(3);
        message.push(2);
        message.extend_from_slice(&[1, 2]);
        message.push(u8::try_from(data.len()).unwrap());
        message.extend_from_slice(&data);
        let user_signature = if user_signs {
            user().sign(&message).to_bytes()
        } else {
            [0; 64]
        };
        let mut wire = vec![2u8];
        wire.extend_from_slice(&[0; 64]);
        wire.extend_from_slice(&user_signature);
        wire.extend_from_slice(&message);
        wire
    }

    #[test]
    fn the_sponsor_co_signs_only_the_exact_prepared_transaction() {
        // PURPOSE: I7 + S1 policy — the sponsor pays only for what the API prepared.
        // ASSERT: the exact user-signed transaction gets a valid sponsor signature in slot 0;
        // altered data, a missing user signature or a non-sponsored preparation are refused.
        // FAILURE MEANS: anyone could make the platform pay for arbitrary transactions.
        let sponsor = sponsor();
        let payer = sponsor.pubkey();
        let expected = expected(payer);
        assert_eq!(
            expected.measured_serialized_bytes,
            wire(payer, None, true).len()
        );

        let signed = co_sign(&sponsor, &wire(payer, None, true), &expected).unwrap();
        let sponsor_key = VerifyingKey::from_bytes(&payer.to_bytes()).unwrap();
        sponsor_key
            .verify(
                &signed.message,
                &Signature::from_bytes(&signed.signatures[0]),
            )
            .unwrap();
        assert_eq!(
            LegacyTransaction::decode(&signed.encode())
                .unwrap()
                .signatures[0],
            signed.signatures[0]
        );

        let mut altered = discriminator("accept_custody_transfer").to_vec();
        altered.extend_from_slice(&4u64.to_le_bytes());
        assert!(co_sign(&sponsor, &wire(payer, Some(altered), true), &expected).is_err());
        assert!(co_sign(&sponsor, &wire(payer, None, false), &expected).is_err());
        let self_paid = self::expected(user_pubkey());
        assert!(co_sign(&sponsor, &wire(payer, None, true), &self_paid).is_err());
    }

    #[test]
    fn payer_selection_never_turns_the_sponsor_into_its_own_authority() {
        let sponsor = sponsor();
        let user = user_pubkey();
        assert_eq!(payer_for(Some(&sponsor), user), sponsor.pubkey());
        assert_eq!(payer_for(None, user), user);
        assert_eq!(
            payer_for(Some(&sponsor), sponsor.pubkey()),
            sponsor.pubkey()
        );
        assert_eq!(
            payer_candidates(Some(&sponsor), user),
            vec![sponsor.pubkey(), user]
        );
        assert_eq!(payer_candidates(None, user), vec![user]);
    }

    #[test]
    fn cost_estimate_counts_signatures_priority_fee_and_created_rent() {
        let sponsor = sponsor();
        let mut prepared = expected(sponsor.pubkey());
        let discriminators = SponsorableDiscriminators::default();
        // accept creates nothing: two signatures only.
        assert_eq!(
            estimate_cost_lamports(&prepared, &discriminators).unwrap(),
            2 * LAMPORTS_PER_SIGNATURE
        );
        let mut data = discriminator("create_intent").to_vec();
        data.extend_from_slice(&[0; 98]);
        prepared.instructions[0].data_base64 = BASE64_STANDARD.encode(data);
        let mut price = vec![3u8];
        price.extend_from_slice(&1_000_000u64.to_le_bytes());
        prepared.instructions.push(InstructionDto {
            program_id: COMPUTE_BUDGET_PROGRAM_ID.into(),
            accounts: Vec::new(),
            data_base64: BASE64_STANDARD.encode(price),
        });
        assert_eq!(
            estimate_cost_lamports(&prepared, &discriminators).unwrap(),
            2 * LAMPORTS_PER_SIGNATURE + 200_000 + rent(INTENT_STATE_SPACE)
        );
        prepared.instructions[0].data_base64 = BASE64_STANDARD.encode([0u8; 16]);
        assert!(estimate_cost_lamports(&prepared, &discriminators).is_err());
    }

    #[test]
    fn a_compiled_transaction_is_exactly_the_prepared_one() {
        // PURPOSE: S2 — what the API signs for an organization is byte-compatible with what it
        // prepared and measured, sponsored or self-paid.
        // FAILURE MEANS: managed signing would broadcast a transaction confirmation rejects.
        for fee_payer in [sponsor().pubkey(), user_pubkey()] {
            let prepared = expected(fee_payer);
            let compiled = compile_unsigned(&prepared, [7; 32]).unwrap();
            assert_eq!(compiled.encode().len(), prepared.measured_serialized_bytes);
            let signature = bs58::encode([0u8; 64]).into_string();
            assert!(
                crate::solana::rpc::transaction_matches_json(
                    &compiled.to_rpc_json(),
                    &signature,
                    &prepared
                )
                .unwrap()
            );
            let round_trip = LegacyTransaction::decode(&compiled.encode()).unwrap();
            assert_eq!(round_trip.message, compiled.message);
        }
    }

    #[test]
    fn decoder_rejects_versioned_and_trailing_bytes() {
        let payer = sponsor().pubkey();
        let mut versioned = wire(payer, None, true);
        versioned[1 + 128] |= 0x80;
        assert!(LegacyTransaction::decode(&versioned).is_err());
        let mut trailing = wire(payer, None, true);
        trailing.push(0);
        assert!(LegacyTransaction::decode(&trailing).is_err());
    }
}
