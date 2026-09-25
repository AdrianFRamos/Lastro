//! Stable domain enums and fixed-size identifiers for protocol v2.

use crate::error::ProtocolError;

pub type AssetId = [u8; 32];
pub type EventId = [u8; 32];
pub type FacilityId = [u8; 32];
pub type PartyId = [u8; 32];
pub type StationId = [u8; 32];
pub type TransformationId = [u8; 32];
pub type IntentId = [u8; 32];
pub type RecallId = [u8; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AssetType {
    Animal = 1,
    Lot = 2,
    Carcass = 3,
    CutBatch = 4,
    ProductLot = 5,
    Package = 6,
    ByproductLot = 7,
    Shipment = 8,
}

impl TryFrom<u8> for AssetType {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Animal),
            2 => Ok(Self::Lot),
            3 => Ok(Self::Carcass),
            4 => Ok(Self::CutBatch),
            5 => Ok(Self::ProductLot),
            6 => Ok(Self::Package),
            7 => Ok(Self::ByproductLot),
            8 => Ok(Self::Shipment),
            _ => Err(ProtocolError::UnknownV2Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AssetStatus {
    Active = 1,
    InTransit = 2,
    Consumed = 3,
    Closed = 4,
    QualityHold = 5,
    Recalled = 6,
    Retired = 7,
}

impl TryFrom<u8> for AssetStatus {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Active),
            2 => Ok(Self::InTransit),
            3 => Ok(Self::Consumed),
            4 => Ok(Self::Closed),
            5 => Ok(Self::QualityHold),
            6 => Ok(Self::Recalled),
            7 => Ok(Self::Retired),
            _ => Err(ProtocolError::UnknownV2Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum EventType {
    AssetRegistered = 1,
    ObservationRecorded = 2,
    LocationObserved = 3,
    CustodyTransferred = 4,
    SlaughterConfirmed = 5,
    CarcassCreated = 6,
    TransformationStarted = 7,
    TransformationFinalized = 8,
    ProductCreated = 9,
    PackageCreated = 10,
    ShipmentCreated = 11,
    ShipmentAccepted = 12,
    QualityHoldPlaced = 13,
    QualityHoldReleased = 14,
    RecallOpened = 15,
    RecallClosed = 16,
    AssetMigrated = 17,
}

impl TryFrom<u16> for EventType {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::AssetRegistered),
            2 => Ok(Self::ObservationRecorded),
            3 => Ok(Self::LocationObserved),
            4 => Ok(Self::CustodyTransferred),
            5 => Ok(Self::SlaughterConfirmed),
            6 => Ok(Self::CarcassCreated),
            7 => Ok(Self::TransformationStarted),
            8 => Ok(Self::TransformationFinalized),
            9 => Ok(Self::ProductCreated),
            10 => Ok(Self::PackageCreated),
            11 => Ok(Self::ShipmentCreated),
            12 => Ok(Self::ShipmentAccepted),
            13 => Ok(Self::QualityHoldPlaced),
            14 => Ok(Self::QualityHoldReleased),
            15 => Ok(Self::RecallOpened),
            16 => Ok(Self::RecallClosed),
            17 => Ok(Self::AssetMigrated),
            _ => Err(ProtocolError::UnknownV2Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum IntentType {
    Observation = 1,
    CustodyTransfer = 2,
    Transformation = 3,
    Shipment = 4,
    Recall = 5,
}

impl TryFrom<u16> for IntentType {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Observation),
            2 => Ok(Self::CustodyTransfer),
            3 => Ok(Self::Transformation),
            4 => Ok(Self::Shipment),
            5 => Ok(Self::Recall),
            _ => Err(ProtocolError::UnknownV2Enum),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum IntentStatus {
    Open = 1,
    Cancelled = 2,
    Consumed = 3,
    Expired = 4,
}

impl TryFrom<u8> for IntentStatus {
    type Error = ProtocolError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Open),
            2 => Ok(Self::Cancelled),
            3 => Ok(Self::Consumed),
            4 => Ok(Self::Expired),
            _ => Err(ProtocolError::UnknownV2Enum),
        }
    }
}

pub fn require_nonzero_id(id: &[u8; 32]) -> Result<(), ProtocolError> {
    if id.iter().all(|byte| *byte == 0) {
        return Err(ProtocolError::InvalidV2Identifier);
    }
    Ok(())
}
