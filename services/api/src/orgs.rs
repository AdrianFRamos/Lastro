//! Organization accounts without browser wallets (SPEC v2.1, S2).
//!
//! Each organization owns one custodian wallet. Its Ed25519 secret exists in plaintext only in
//! memory while a transaction is signed; at rest it is sealed with AES-256-GCM under a
//! deployment master key (`LASTRO_ORG_KEY_ENCRYPTION_KEY_HEX`) that never enters the database.
//! The ciphertext is bound to its organization and wallet, so sealed keys cannot be swapped
//! between rows. A hosted wallet provider can replace [`LocalKeyVault`] behind the same calls.
//!
//! People sign in with a one-time code and act through roles; a role only reaches the
//! transaction purposes [`role_allows`] lists, and only for its own organization's wallet.

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use ed25519_dalek::{Signer as _, SigningKey};
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{error::ApiError, model::SponsorPurpose};

pub const SEALED_SECRET_LEN: usize = 12 + 32 + 16;
pub const KEY_VERSION: i16 = 1;
const SEAL_DOMAIN: &[u8] = b"LASTRO_V2_ORG_KEY\0";
const OTP_DOMAIN: &[u8] = b"LASTRO_V2_OTP\0";
const TOKEN_DOMAIN: &[u8] = b"LASTRO_V2_USER_TOKEN\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    OrgAdmin,
    OperadorCurral,
    Auditor,
    Leitura,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OrgAdmin => "ORG_ADMIN",
            Self::OperadorCurral => "OPERADOR_CURRAL",
            Self::Auditor => "AUDITOR",
            Self::Leitura => "LEITURA",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ApiError> {
        match value {
            "ORG_ADMIN" => Ok(Self::OrgAdmin),
            "OPERADOR_CURRAL" => Ok(Self::OperadorCurral),
            "AUDITOR" => Ok(Self::Auditor),
            "LEITURA" => Ok(Self::Leitura),
            _ => Err(ApiError::Validation(
                "role must be ORG_ADMIN, OPERADOR_CURRAL, AUDITOR or LEITURA".into(),
            )),
        }
    }
}

/// What each role may have the organization wallet sign. A corral operator runs handling
/// sessions; identity changes and custody transfers need an administrator; auditors and
/// read-only users sign nothing.
pub fn role_allows(role: Role, purpose: &SponsorPurpose) -> bool {
    match role {
        Role::OrgAdmin => true,
        Role::OperadorCurral => matches!(
            purpose,
            SponsorPurpose::SessionOpen { .. }
                | SponsorPurpose::SessionBatch { .. }
                | SponsorPurpose::SessionRevoke { .. }
        ),
        Role::Auditor | Role::Leitura => false,
    }
}

/// Seals and opens organization secrets under the deployment master key.
pub struct LocalKeyVault {
    cipher: Aes256Gcm,
    otp_pepper: [u8; 32],
}

impl LocalKeyVault {
    pub fn new(master_key: &[u8; 32]) -> Self {
        let mut pepper = Sha256::new();
        pepper.update(OTP_DOMAIN);
        pepper.update(master_key);
        Self {
            cipher: Aes256Gcm::new(master_key.into()),
            otp_pepper: pepper.finalize().into(),
        }
    }

    /// A fresh organization wallet: (public key, sealed secret).
    pub fn generate(&self, org_id: Uuid) -> Result<(Pubkey, [u8; SEALED_SECRET_LEN]), ApiError> {
        // Long-lived custody key: drawn from the OS generator and wiped after sealing.
        let mut secret = Zeroizing::new([0u8; 32]);
        rand::TryRngCore::try_fill_bytes(&mut rand::rngs::OsRng, secret.as_mut())
            .map_err(|_| ApiError::Internal)?;
        let key = SigningKey::from_bytes(&secret);
        let wallet = Pubkey::new_from_array(key.verifying_key().to_bytes());
        Ok((wallet, self.seal(org_id, wallet, &secret)?))
    }

    pub fn seal(
        &self,
        org_id: Uuid,
        wallet: Pubkey,
        secret: &[u8; 32],
    ) -> Result<[u8; SEALED_SECRET_LEN], ApiError> {
        let nonce: [u8; 12] = rand::random();
        let ciphertext = self
            .cipher
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: secret,
                    aad: &associated_data(org_id, wallet),
                },
            )
            .map_err(|_| ApiError::Internal)?;
        let mut sealed = [0u8; SEALED_SECRET_LEN];
        sealed[..12].copy_from_slice(&nonce);
        sealed[12..].copy_from_slice(&ciphertext);
        Ok(sealed)
    }

    /// The organization's signing key; fails if the row was tampered with or swapped.
    pub fn open(
        &self,
        org_id: Uuid,
        wallet: Pubkey,
        sealed: &[u8; SEALED_SECRET_LEN],
    ) -> Result<SigningKey, ApiError> {
        let nonce: [u8; 12] = sealed[..12].try_into().map_err(|_| ApiError::Internal)?;
        let secret: Zeroizing<Vec<u8>> = self
            .cipher
            .decrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: &sealed[12..],
                    aad: &associated_data(org_id, wallet),
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| ApiError::Conflict("organization key cannot be opened".into()))?;
        let secret: Zeroizing<[u8; 32]> = Zeroizing::new(
            secret
                .as_slice()
                .try_into()
                .map_err(|_| ApiError::Internal)?,
        );
        let key = SigningKey::from_bytes(&secret);
        if key.verifying_key().to_bytes() != wallet.to_bytes() {
            return Err(ApiError::Conflict(
                "organization key does not match its wallet".into(),
            ));
        }
        Ok(key)
    }

    /// Keyed hash of a one-time code, bound to the user it was issued to.
    pub fn code_hash(&self, user_id: Uuid, code: &str) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(OTP_DOMAIN);
        hasher.update(self.otp_pepper);
        hasher.update(user_id.as_bytes());
        hasher.update(code.as_bytes());
        hasher.finalize().into()
    }
}

pub fn token_hash(token: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(TOKEN_DOMAIN);
    hasher.update(token.as_bytes());
    hasher.finalize().into()
}

/// Six decimal digits, uniformly drawn.
pub fn new_code() -> String {
    format!("{:06}", rand::random_range(0..1_000_000u32))
}

/// 32 random bytes, hex encoded.
pub fn new_token() -> String {
    hex::encode(rand::random::<[u8; 32]>())
}

pub fn sign(key: &SigningKey, message: &[u8]) -> [u8; 64] {
    key.sign(message).to_bytes()
}

fn associated_data(org_id: Uuid, wallet: Pubkey) -> Vec<u8> {
    let mut aad = SEAL_DOMAIN.to_vec();
    aad.extend_from_slice(org_id.as_bytes());
    aad.extend_from_slice(wallet.as_ref());
    aad
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sealed_key_opens_only_for_its_own_organization() {
        // PURPOSE: S2 — the database alone never yields a usable organization key.
        // ASSERT: the right row opens; another org id, wallet or master key does not.
        // FAILURE MEANS: a database leak or row swap would hand out custodian keys.
        let vault = LocalKeyVault::new(&[7; 32]);
        let org = Uuid::from_bytes([1; 16]);
        let (wallet, sealed) = vault.generate(org).unwrap();
        let key = vault.open(org, wallet, &sealed).unwrap();
        assert_eq!(key.verifying_key().to_bytes(), wallet.to_bytes());
        assert!(
            vault
                .open(Uuid::from_bytes([2; 16]), wallet, &sealed)
                .is_err()
        );
        assert!(vault.open(org, Pubkey::new_unique(), &sealed).is_err());
        assert!(
            LocalKeyVault::new(&[8; 32])
                .open(org, wallet, &sealed)
                .is_err()
        );
        let mut tampered = sealed;
        tampered[20] ^= 1;
        assert!(vault.open(org, wallet, &tampered).is_err());
    }

    #[test]
    fn corral_operators_reach_only_session_purposes() {
        let session = SponsorPurpose::SessionOpen {
            session_id: "00".repeat(32),
        };
        let custody = SponsorPurpose::CustodyPropose {
            transfer_id: Uuid::nil(),
        };
        assert!(role_allows(Role::OperadorCurral, &session));
        assert!(!role_allows(Role::OperadorCurral, &custody));
        assert!(role_allows(Role::OrgAdmin, &custody));
        assert!(!role_allows(Role::Auditor, &session));
        assert!(!role_allows(Role::Leitura, &session));
    }

    #[test]
    fn codes_are_six_digits_and_hashes_bind_the_user() {
        let vault = LocalKeyVault::new(&[7; 32]);
        let code = new_code();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
        assert_ne!(
            vault.code_hash(Uuid::from_bytes([1; 16]), &code),
            vault.code_hash(Uuid::from_bytes([2; 16]), &code)
        );
    }
}
