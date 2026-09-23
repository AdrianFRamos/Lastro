//! Fixed-offset on-chain StationEvent parsing and local action semantics.
//!
//! The parser never deserializes arbitrary bytes into a Rust struct and never reserializes
//! the signed message. All state-transition handlers operate against these original 276 bytes.

use anchor_lang::prelude::*;
use sha2::{Digest, Sha256};

use crate::{constants::STATION_EVENT_LEN, error::LastroError};

const ZERO32: [u8; 32] = [0; 32];

pub const ACTION_ORIGIN: u8 = 1;
pub const ACTION_TRANSFER: u8 = 2;
pub const ACTION_REIDENTIFY: u8 = 3;

pub struct ParsedEvent<'a> {
    pub raw: &'a [u8; STATION_EVENT_LEN],
}

impl ParsedEvent<'_> {
    pub fn action(&self) -> u8 {
        self.raw[5]
    }
    pub fn deployment_id(&self) -> [u8; 32] {
        self.raw[8..40].try_into().expect("fixed range")
    }
    pub fn animal_id(&self) -> [u8; 32] {
        self.raw[40..72].try_into().expect("fixed range")
    }
    pub fn station_id(&self) -> [u8; 32] {
        self.raw[72..104].try_into().expect("fixed range")
    }
    pub fn event_sequence(&self) -> u64 {
        u64::from_le_bytes(self.raw[104..112].try_into().expect("fixed range"))
    }
    pub fn identity_revision(&self) -> u32 {
        u32::from_le_bytes(self.raw[112..116].try_into().expect("fixed range"))
    }
    pub fn previous_event_hash(&self) -> [u8; 32] {
        self.raw[116..148].try_into().expect("fixed range")
    }
    pub fn old_rfid_hash(&self) -> [u8; 32] {
        self.raw[148..180].try_into().expect("fixed range")
    }
    pub fn new_rfid_hash(&self) -> [u8; 32] {
        self.raw[180..212].try_into().expect("fixed range")
    }
    pub fn from_custodian(&self) -> [u8; 32] {
        self.raw[212..244].try_into().expect("fixed range")
    }
    pub fn to_custodian(&self) -> [u8; 32] {
        self.raw[244..276].try_into().expect("fixed range")
    }

    pub fn event_hash(&self) -> [u8; 32] {
        Sha256::digest(self.raw).into()
    }
}

pub fn station_id_from_pubkey(station_pubkey33: &[u8; 33]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"LASTRO_STATION\0");
    hasher.update(station_pubkey33);
    hasher.finalize().into()
}

pub fn parse_event(bytes: &[u8]) -> Result<ParsedEvent<'_>> {
    let raw: &[u8; STATION_EVENT_LEN] = bytes
        .try_into()
        .map_err(|_| error!(LastroError::InvalidEvent))?;

    require!(&raw[0..4] == b"LSTR", LastroError::InvalidEvent);
    require!(raw[4] == 1, LastroError::InvalidEvent);
    require!(
        matches!(raw[5], ACTION_ORIGIN | ACTION_TRANSFER | ACTION_REIDENTIFY),
        LastroError::InvalidEvent
    );
    require!(raw[6] == 0 && raw[7] == 0, LastroError::InvalidEvent);

    let parsed = ParsedEvent { raw };
    validate_local_semantics(&parsed)?;
    Ok(parsed)
}

fn validate_local_semantics(event: &ParsedEvent<'_>) -> Result<()> {
    match event.action() {
        ACTION_ORIGIN => {
            require!(event.event_sequence() == 1, LastroError::InvalidSequence);
            require!(event.identity_revision() == 1, LastroError::InvalidRevision);
            require!(
                event.previous_event_hash() == ZERO32,
                LastroError::InvalidPredecessor
            );
            require!(
                event.old_rfid_hash() == ZERO32,
                LastroError::InvalidRfidTransition
            );
            require!(
                event.new_rfid_hash() != ZERO32,
                LastroError::InvalidRfidTransition
            );
            require!(
                event.from_custodian() == ZERO32,
                LastroError::InvalidCustodian
            );
            require!(
                event.to_custodian() != ZERO32,
                LastroError::InvalidCustodian
            );
        }
        ACTION_TRANSFER => {
            require!(event.event_sequence() >= 2, LastroError::InvalidSequence);
            require!(event.identity_revision() >= 1, LastroError::InvalidRevision);
            require!(
                event.previous_event_hash() != ZERO32,
                LastroError::InvalidPredecessor
            );
            require!(
                event.old_rfid_hash() != ZERO32,
                LastroError::InvalidRfidTransition
            );
            require!(
                event.old_rfid_hash() == event.new_rfid_hash(),
                LastroError::InvalidRfidTransition
            );
            require!(
                event.from_custodian() != ZERO32,
                LastroError::InvalidCustodian
            );
            require!(
                event.to_custodian() != ZERO32,
                LastroError::InvalidCustodian
            );
            require!(
                event.from_custodian() != event.to_custodian(),
                LastroError::InvalidCustodian
            );
        }
        ACTION_REIDENTIFY => {
            require!(event.event_sequence() >= 2, LastroError::InvalidSequence);
            require!(event.identity_revision() >= 2, LastroError::InvalidRevision);
            require!(
                event.previous_event_hash() != ZERO32,
                LastroError::InvalidPredecessor
            );
            require!(
                event.old_rfid_hash() != ZERO32,
                LastroError::InvalidRfidTransition
            );
            require!(
                event.new_rfid_hash() != ZERO32,
                LastroError::InvalidRfidTransition
            );
            require!(
                event.old_rfid_hash() != event.new_rfid_hash(),
                LastroError::InvalidRfidTransition
            );
            require!(
                event.from_custodian() != ZERO32,
                LastroError::InvalidCustodian
            );
            require!(
                event.from_custodian() == event.to_custodian(),
                LastroError::InvalidCustodian
            );
        }
        _ => return err!(LastroError::InvalidEvent),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: &[u8; STATION_EVENT_LEN] =
        include_bytes!("../../../../../test-vectors/origin.bin");

    #[test]
    fn frozen_origin_parses_without_reserialization() {
        let parsed = parse_event(ORIGIN).expect("frozen ORIGIN must parse");
        assert_eq!(parsed.action(), ACTION_ORIGIN);
        assert_eq!(parsed.event_sequence(), 1);
        assert_eq!(parsed.identity_revision(), 1);
        let expected_hash: [u8; 32] = Sha256::digest(ORIGIN).into();
        assert_eq!(parsed.event_hash(), expected_hash);
    }

    #[test]
    fn parser_rejects_reserved_byte_and_action_semantic_tampering() {
        let mut reserved = *ORIGIN;
        reserved[6] = 1;
        assert!(parse_event(&reserved).is_err());

        let mut wrong_action = *ORIGIN;
        wrong_action[5] = ACTION_TRANSFER;
        assert!(parse_event(&wrong_action).is_err());
    }

    #[test]
    fn station_id_is_domain_separated_from_public_key_bytes() {
        let key: [u8; 33] =
            hex::decode("036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296")
                .expect("valid fixture hex")
                .try_into()
                .expect("33-byte key");
        assert_eq!(
            hex::encode(station_id_from_pubkey(&key)),
            "56c266d8ab41a37e7a3bb80b33f6249c05bdb965c68bbeb4775303c69594df77",
        );
    }
}
