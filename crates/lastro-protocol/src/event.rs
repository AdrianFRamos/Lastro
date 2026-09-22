//! StationEvent encoder, decoder, and pure transition rules.
//! Encoding uses explicit offsets and little-endian integers. Rust struct memory layout is never serialized.

use sha2::{Digest, Sha256};

use crate::{
    action::Action,
    constants::{offset, MAGIC, RESERVED, STATION_EVENT_LEN, VERSION},
    error::ProtocolError,
    ids::*,
};

const ZERO32: [u8; 32] = [0; 32];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StationEvent {
    pub action: Action,
    pub deployment_id: DeploymentId,
    pub animal_id: AnimalId,
    pub station_id: StationId,
    pub event_sequence: u64,
    pub identity_revision: u32,
    pub previous_event_hash: EventHash,
    pub old_rfid_hash: RfidHash,
    pub new_rfid_hash: RfidHash,
    pub from_custodian: Custodian,
    pub to_custodian: Custodian,
}

impl StationEvent {
    pub fn encode(&self) -> [u8; STATION_EVENT_LEN] {
        let mut out = [0u8; STATION_EVENT_LEN];
        out[offset::MAGIC..offset::VERSION].copy_from_slice(&MAGIC);
        out[offset::VERSION] = VERSION;
        out[offset::ACTION] = self.action as u8;
        out[offset::RESERVED..offset::DEPLOYMENT_ID].copy_from_slice(&RESERVED);
        out[offset::DEPLOYMENT_ID..offset::ANIMAL_ID].copy_from_slice(&self.deployment_id);
        out[offset::ANIMAL_ID..offset::STATION_ID].copy_from_slice(&self.animal_id);
        out[offset::STATION_ID..offset::EVENT_SEQUENCE].copy_from_slice(&self.station_id);
        out[offset::EVENT_SEQUENCE..offset::IDENTITY_REVISION]
            .copy_from_slice(&self.event_sequence.to_le_bytes());
        out[offset::IDENTITY_REVISION..offset::PREVIOUS_EVENT_HASH]
            .copy_from_slice(&self.identity_revision.to_le_bytes());
        out[offset::PREVIOUS_EVENT_HASH..offset::OLD_RFID_HASH]
            .copy_from_slice(&self.previous_event_hash);
        out[offset::OLD_RFID_HASH..offset::NEW_RFID_HASH].copy_from_slice(&self.old_rfid_hash);
        out[offset::NEW_RFID_HASH..offset::FROM_CUSTODIAN].copy_from_slice(&self.new_rfid_hash);
        out[offset::FROM_CUSTODIAN..offset::TO_CUSTODIAN].copy_from_slice(&self.from_custodian);
        out[offset::TO_CUSTODIAN..offset::END].copy_from_slice(&self.to_custodian);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ProtocolError> {
        if bytes.len() != STATION_EVENT_LEN {
            return Err(ProtocolError::InvalidLength);
        }
        if bytes[offset::MAGIC..offset::VERSION] != MAGIC {
            return Err(ProtocolError::InvalidMagic);
        }
        if bytes[offset::VERSION] != VERSION {
            return Err(ProtocolError::UnsupportedVersion);
        }
        if bytes[offset::RESERVED..offset::DEPLOYMENT_ID] != RESERVED {
            return Err(ProtocolError::ReservedNotZero);
        }

        let event = Self {
            action: Action::try_from(bytes[offset::ACTION])?,
            deployment_id: bytes[offset::DEPLOYMENT_ID..offset::ANIMAL_ID]
                .try_into()
                .expect("fixed slice"),
            animal_id: bytes[offset::ANIMAL_ID..offset::STATION_ID]
                .try_into()
                .expect("fixed slice"),
            station_id: bytes[offset::STATION_ID..offset::EVENT_SEQUENCE]
                .try_into()
                .expect("fixed slice"),
            event_sequence: u64::from_le_bytes(
                bytes[offset::EVENT_SEQUENCE..offset::IDENTITY_REVISION]
                    .try_into()
                    .expect("fixed slice"),
            ),
            identity_revision: u32::from_le_bytes(
                bytes[offset::IDENTITY_REVISION..offset::PREVIOUS_EVENT_HASH]
                    .try_into()
                    .expect("fixed slice"),
            ),
            previous_event_hash: bytes[offset::PREVIOUS_EVENT_HASH..offset::OLD_RFID_HASH]
                .try_into()
                .expect("fixed slice"),
            old_rfid_hash: bytes[offset::OLD_RFID_HASH..offset::NEW_RFID_HASH]
                .try_into()
                .expect("fixed slice"),
            new_rfid_hash: bytes[offset::NEW_RFID_HASH..offset::FROM_CUSTODIAN]
                .try_into()
                .expect("fixed slice"),
            from_custodian: bytes[offset::FROM_CUSTODIAN..offset::TO_CUSTODIAN]
                .try_into()
                .expect("fixed slice"),
            to_custodian: bytes[offset::TO_CUSTODIAN..offset::END]
                .try_into()
                .expect("fixed slice"),
        };
        event.validate_semantics()?;
        Ok(event)
    }

    pub fn validate_semantics(&self) -> Result<(), ProtocolError> {
        match self.action {
            Action::Origin => {
                if self.event_sequence != 1
                    || self.identity_revision != 1
                    || self.previous_event_hash != ZERO32
                    || self.old_rfid_hash != ZERO32
                    || self.new_rfid_hash == ZERO32
                    || self.from_custodian != ZERO32
                    || self.to_custodian == ZERO32
                {
                    return Err(ProtocolError::InvalidSemantics);
                }
            }
            Action::Transfer => {
                if self.event_sequence < 2
                    || self.identity_revision == 0
                    || self.previous_event_hash == ZERO32
                    || self.old_rfid_hash == ZERO32
                    || self.old_rfid_hash != self.new_rfid_hash
                    || self.from_custodian == ZERO32
                    || self.to_custodian == ZERO32
                    || self.from_custodian == self.to_custodian
                {
                    return Err(ProtocolError::InvalidSemantics);
                }
            }
            Action::Reidentify => {
                if self.event_sequence < 2
                    || self.identity_revision < 2
                    || self.previous_event_hash == ZERO32
                    || self.old_rfid_hash == ZERO32
                    || self.new_rfid_hash == ZERO32
                    || self.old_rfid_hash == self.new_rfid_hash
                    || self.from_custodian == ZERO32
                    || self.from_custodian != self.to_custodian
                {
                    return Err(ProtocolError::InvalidSemantics);
                }
            }
        }
        Ok(())
    }

    pub fn validate_successor(&self, current: &StationEvent) -> Result<(), ProtocolError> {
        self.validate_semantics()?;
        if self.action == Action::Origin
            || self.deployment_id != current.deployment_id
            || self.animal_id != current.animal_id
            || self.event_sequence != current.event_sequence.checked_add(1).ok_or(ProtocolError::InvalidSuccessor)?
            || self.previous_event_hash != current.event_hash()
        {
            return Err(ProtocolError::InvalidSuccessor);
        }

        match self.action {
            Action::Origin => unreachable!("handled above"),
            Action::Transfer => {
                if self.identity_revision != current.identity_revision
                    || self.old_rfid_hash != current.new_rfid_hash
                    || self.new_rfid_hash != current.new_rfid_hash
                    || self.from_custodian != current.to_custodian
                {
                    return Err(ProtocolError::InvalidSuccessor);
                }
            }
            Action::Reidentify => {
                if self.identity_revision
                    != current
                        .identity_revision
                        .checked_add(1)
                        .ok_or(ProtocolError::InvalidSuccessor)?
                    || self.old_rfid_hash != current.new_rfid_hash
                    || self.from_custodian != current.to_custodian
                    || self.to_custodian != current.to_custodian
                {
                    return Err(ProtocolError::InvalidSuccessor);
                }
            }
        }
        Ok(())
    }

    pub fn event_hash(&self) -> EventHash {
        Sha256::digest(self.encode()).into()
    }
}
