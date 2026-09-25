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

macro_rules! stable_enum {
    ($(#[$meta:meta])* $name:ident : $repr:ty { $($variant:ident = $value:expr),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr($repr)]
        pub enum $name {
            $($variant = $value),+
        }

        impl TryFrom<$repr> for $name {
            type Error = ProtocolError;

            fn try_from(value: $repr) -> Result<Self, Self::Error> {
                match value {
                    $( $value => Ok(Self::$variant), )+
                    _ => Err(ProtocolError::UnknownV2Enum),
                }
            }
        }
    };
}

stable_enum! {
    /// Canonical asset kinds in the livestock and product chain.
    AssetType: u8 {
        Animal = 1,
        Lot = 2,
        Carcass = 3,
        CutBatch = 4,
        ProductLot = 5,
        Package = 6,
        ByproductLot = 7,
        Shipment = 8
    }
}

stable_enum! {
    /// Canonical lifecycle status for an asset.
    AssetStatus: u8 {
        Active = 1,
        InTransit = 2,
        Consumed = 3,
        Closed = 4,
        QualityHold = 5,
        Recalled = 6,
        Retired = 7
    }
}

stable_enum! {
    /// Facility classification used by authorization guards.
    FacilityType: u8 {
        Farm = 1,
        TransportHub = 2,
        Slaughterhouse = 3,
        ProcessingFacility = 4,
        DistributionCenter = 5,
        Retail = 6,
        InspectionSite = 7
    }
}

stable_enum! {
    /// Facility lifecycle status. Revocation is terminal for the record.
    FacilityStatus: u8 {
        Active = 1,
        Suspended = 2,
        Revoked = 3,
        Expired = 4
    }
}

stable_enum! {
    /// Station key lifecycle status.
    StationStatus: u8 {
        Active = 1,
        Suspended = 2,
        Revoked = 3,
        Expired = 4
    }
}

stable_enum! {
    /// Transformation lifecycle status.
    TransformationStatus: u8 {
        Open = 1,
        Finalizing = 2,
        Finalized = 3,
        Aborted = 4,
        Expired = 5
    }
}

stable_enum! {
    /// Recall lifecycle status.
    RecallStatus: u8 {
        Open = 1,
        Closed = 2,
        Cancelled = 3
    }
}

stable_enum! {
    /// Provenance class for a domain observation or document.
    ProvenanceType: u8 {
        Station = 1,
        Scale = 2,
        Facility = 3,
        Custodian = 4,
        OfficialSource = 5
    }
}

stable_enum! {
    /// Units allowed in compact on-chain measurements.
    UnitCode: u8 {
        Gram = 1,
        Kilogram = 2,
        Head = 3,
        Package = 4
    }
}

stable_enum! {
    /// Role of a leaf in a transformation or lineage commitment.
    LineageRole: u8 {
        Input = 1,
        Output = 2,
        Byproduct = 3,
        Loss = 4
    }
}

stable_enum! {
    /// Domain events accepted by the v2 protocol.
    EventType: u16 {
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
        AssetMigrated = 17
    }
}

stable_enum! {
    /// Intent classes that can reserve a state transition.
    IntentType: u16 {
        Observation = 1,
        CustodyTransfer = 2,
        Transformation = 3,
        Shipment = 4,
        Recall = 5
    }
}

stable_enum! {
    /// Terminal-aware lifecycle for an on-chain intent.
    IntentStatus: u8 {
        Open = 1,
        Cancelled = 2,
        Consumed = 3,
        Expired = 4
    }
}

pub fn require_nonzero_id(id: &[u8; 32]) -> Result<(), ProtocolError> {
    if id.iter().all(|byte| *byte == 0) {
        return Err(ProtocolError::InvalidV2Identifier);
    }
    Ok(())
}
