//! On-chain validation of the shared v2 domain envelope.

use anchor_lang::prelude::*;
use lastro_protocol::v2::{DomainEventEnvelope, EventType, V2_ENVELOPE_LEN};

use crate::{error::LastroV2Error, state::AssetState};

pub fn parse_observation(
    event: &[u8; V2_ENVELOPE_LEN],
    config_deployment_id: [u8; 32],
    subject_id: [u8; 32],
    event_id: [u8; 32],
    station_id: [u8; 32],
    asset: &AssetState,
    now: i64,
) -> Result<DomainEventEnvelope> {
    let envelope = DomainEventEnvelope::decode(event)
        .map_err(|_| error!(LastroV2Error::InvalidDomainEvent))?;
    require!(
        envelope.event_type == EventType::ObservationRecorded as u16,
        LastroV2Error::InvalidDomainEvent
    );
    require!(
        envelope.deployment_id == config_deployment_id,
        LastroV2Error::InvalidDomainEvent
    );
    require!(
        envelope.subject_id == subject_id,
        LastroV2Error::InvalidDomainEvent
    );
    require!(
        envelope.event_id == event_id,
        LastroV2Error::InvalidDomainEvent
    );
    require!(
        envelope.source_id == station_id,
        LastroV2Error::InvalidStationProof
    );
    require!(
        envelope.state_version == asset.state_version.saturating_add(1),
        LastroV2Error::InvalidEventStateVersion
    );
    require!(
        envelope.expected_previous_hash == asset.last_event_hash,
        LastroV2Error::InvalidEventPredecessor
    );
    require!(
        now >= envelope.observed_at && now <= envelope.expires_at,
        LastroV2Error::EventOutsideValidityWindow
    );
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lastro_protocol::v2::EventType;

    fn envelope() -> [u8; V2_ENVELOPE_LEN] {
        DomainEventEnvelope::new(
            EventType::ObservationRecorded,
            [1; 32],
            [2; 32],
            [3; 32],
            1,
            [0; 32],
            [4; 32],
            [5; 32],
            10,
            20,
        )
        .expect("valid envelope")
        .encode()
        .expect("valid bytes")
    }

    #[test]
    fn observation_requires_exact_predecessor_and_version() {
        let asset = AssetState {
            asset_id: [2; 32],
            asset_type: 1,
            status: 1,
            deployment_id: [1; 32],
            custodian: Pubkey::new_unique(),
            parent_root: [0; 32],
            lineage_root: [0; 32],
            current_lot_id: [0; 32],
            available_weight_grams: 100,
            reserved_weight_grams: 0,
            event_sequence: 0,
            state_version: 0,
            last_event_hash: [0; 32],
            reserved_by: [0; 32],
            reserved_until: 0,
            flags: 0,
            bump: 1,
        };
        assert!(
            parse_observation(&envelope(), [1; 32], [2; 32], [3; 32], [5; 32], &asset, 10).is_ok()
        );

        let mut stale = asset;
        stale.state_version = 1;
        assert!(
            parse_observation(&envelope(), [1; 32], [2; 32], [3; 32], [5; 32], &stale, 10).is_err()
        );
    }
}
