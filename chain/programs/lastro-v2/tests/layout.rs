use anchor_lang::prelude::Pubkey;
use lastro_v2::{
    constants::{is_valid_asset_type, is_valid_facility_type, is_valid_party_role},
    state::{
        AssetState, EventAnchor, FacilityRecord, IntentState, PartyRecord, ProtocolConfigV2,
        StationRecord,
    },
};

#[test]
fn account_space_constants_include_anchor_discriminator() {
    assert!(ProtocolConfigV2::SPACE > 8);
    assert!(StationRecord::SPACE > 8);
    assert!(FacilityRecord::SPACE > 8);
    assert!(PartyRecord::SPACE > 8);
    assert!(AssetState::SPACE > 8);
    assert!(IntentState::SPACE > 8);
    assert!(EventAnchor::SPACE > 8);
}

#[test]
fn enum_ranges_are_closed_and_stable() {
    assert!(is_valid_asset_type(1));
    assert!(is_valid_asset_type(8));
    assert!(!is_valid_asset_type(0));
    assert!(!is_valid_asset_type(9));
    assert!(is_valid_facility_type(3));
    assert!(!is_valid_facility_type(8));
    assert!(is_valid_party_role(1));
    assert!(is_valid_party_role(11));
    assert!(!is_valid_party_role(12));
}

#[test]
fn asset_closed_is_terminal_for_closed_and_retired_statuses() {
    let mut asset = AssetState {
        asset_id: [1; 32],
        asset_type: 1,
        status: 4,
        deployment_id: [2; 32],
        custodian: Pubkey::new_unique(),
        parent_root: [0; 32],
        lineage_root: [3; 32],
        current_lot_id: [0; 32],
        available_weight_grams: 10,
        event_sequence: 0,
        state_version: 0,
        last_event_hash: [0; 32],
        reserved_by: [0; 32],
        reserved_until: 0,
        flags: 0,
        bump: 1,
    };
    assert!(asset.is_closed());
    asset.status = 7;
    assert!(asset.is_closed());
    asset.status = 1;
    assert!(!asset.is_closed());
}
