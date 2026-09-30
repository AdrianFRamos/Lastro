use anchor_lang::{AnchorSerialize, prelude::Pubkey};
use lastro_v2::{
    constants::{is_valid_asset_type, is_valid_facility_type, is_valid_party_role},
    state::{
        AssetState, EventAnchor, FacilityRecord, IntentState, LineageAnchor, PartyRecord,
        ProtocolConfigV2, RegistryRoot, RfidBinding, StationRecord, TransformationAnchor,
        TransformationReservation,
    },
};

/// Every allocation must equal the 8-byte discriminator plus the exact Borsh size.
/// A smaller SPACE makes `init` fail with AccountDidNotSerialize at runtime.
fn assert_exact_space<T: AnchorSerialize>(name: &str, value: &T, space: usize) {
    let mut bytes = Vec::new();
    value.serialize(&mut bytes).expect("account serializes");
    let serialized = bytes.len() + 8;
    assert_eq!(
        space, serialized,
        "{name}::SPACE does not match its serialized size"
    );
}

#[test]
fn account_space_constants_match_serialized_layouts() {
    // PURPOSE: on-chain account sizes equal their serialized layouts.
    // FAILURE MEANS: accounts would be allocated too small or with a layout clients misread.
    let key = Pubkey::new_unique();
    assert_exact_space(
        "ProtocolConfigV2",
        &ProtocolConfigV2 {
            authority: key,
            deployment_id: [0; 32],
            schema_version: 0,
            station_registry: key,
            facility_registry: key,
            party_registry: key,
            document_registry: key,
            max_asset_weight_grams: 0,
            mass_tolerance_basis_points: 0,
            max_event_age_seconds: 0,
            bump: 0,
        },
        ProtocolConfigV2::SPACE,
    );
    assert_exact_space(
        "RegistryRoot",
        &RegistryRoot {
            deployment_id: [0; 32],
            registry_type: 0,
            bump: 0,
        },
        RegistryRoot::SPACE,
    );
    assert_exact_space(
        "StationRecord",
        &StationRecord {
            station_id: [0; 32],
            key_id: [0; 32],
            pubkey33: [0; 33],
            status: 0,
            valid_from: 0,
            valid_until: 0,
            firmware_hash: [0; 32],
            bump: 0,
        },
        StationRecord::SPACE,
    );
    assert_exact_space(
        "FacilityRecord",
        &FacilityRecord {
            facility_id: [0; 32],
            owner: key,
            facility_type: 0,
            status: 0,
            credential_hash: [0; 32],
            valid_from: 0,
            valid_until: 0,
            bump: 0,
        },
        FacilityRecord::SPACE,
    );
    assert_exact_space(
        "PartyRecord",
        &PartyRecord {
            party_id: [0; 32],
            wallet: key,
            role: 0,
            status: 0,
            bump: 0,
        },
        PartyRecord::SPACE,
    );
    assert_exact_space("AssetState", &asset(), AssetState::SPACE);
    assert_exact_space(
        "RfidBinding",
        &RfidBinding {
            asset_id: [0; 32],
            rfid_hash: [0; 32],
            status: 0,
            bump: 0,
        },
        RfidBinding::SPACE,
    );
    assert_exact_space(
        "IntentState",
        &IntentState {
            intent_id: [0; 32],
            subject_id: [0; 32],
            intent_type: 0,
            expected_state_version: 0,
            nonce: 0,
            actor: key,
            status: 0,
            expires_at: 0,
            consumed_at: 0,
            payload_hash: [0; 32],
            bump: 0,
        },
        IntentState::SPACE,
    );
    assert_exact_space(
        "EventAnchor",
        &EventAnchor {
            event_id: [0; 32],
            deployment_id: [0; 32],
            subject_id: [0; 32],
            source_id: [0; 32],
            event_type: 0,
            state_version: 0,
            observed_at: 0,
            expires_at: 0,
            expected_previous_hash: [0; 32],
            payload_hash: [0; 32],
            event_hash: [0; 32],
            bump: 0,
        },
        EventAnchor::SPACE,
    );
    assert_exact_space(
        "LineageAnchor",
        &LineageAnchor {
            asset_id: [0; 32],
            lineage_root: [0; 32],
            parent_root: [0; 32],
            edge_count: 0,
            sequence: 0,
            last_transformation: [0; 32],
            bump: 0,
        },
        LineageAnchor::SPACE,
    );
    assert_exact_space(
        "TransformationAnchor",
        &TransformationAnchor {
            transformation_id: [0; 32],
            facility_id: [0; 32],
            transformation_type: 0,
            input_root: [0; 32],
            output_root: [0; 32],
            input_count: 0,
            output_count: 0,
            reserved_input_count: 0,
            consumed_input_count: 0,
            created_output_count: 0,
            input_weight_grams: 0,
            reserved_input_weight_grams: 0,
            consumed_input_weight_grams: 0,
            output_weight_grams: 0,
            created_output_weight_grams: 0,
            byproduct_weight_grams: 0,
            loss_weight_grams: 0,
            created_byproduct_weight_grams: 0,
            tolerance_basis_points: 0,
            status: 0,
            manifest_hash: [0; 32],
            sequence: 0,
            expires_at: 0,
            bump: 0,
        },
        TransformationAnchor::SPACE,
    );
    assert_exact_space(
        "TransformationReservation",
        &TransformationReservation {
            transformation_id: [0; 32],
            asset_id: [0; 32],
            weight_grams: 0,
            expected_state_version: 0,
            reserved_until: 0,
            consumed_at: 0,
            bump: 0,
        },
        TransformationReservation::SPACE,
    );
}

/// The API decodes AssetState at a fixed 341-byte layout (services/api/src/solana/rpc.rs).
#[test]
fn asset_state_space_matches_api_decoder_layout() {
    // PURPOSE: the API decodes AssetState at the same size the program allocates.
    // FAILURE MEANS: the API would reject or misread every canonical asset.
    assert_eq!(AssetState::SPACE, 341);
}

fn asset() -> AssetState {
    AssetState {
        asset_id: [1; 32],
        asset_type: 1,
        status: 1,
        deployment_id: [2; 32],
        custodian: Pubkey::new_unique(),
        parent_root: [0; 32],
        lineage_root: [3; 32],
        current_lot_id: [0; 32],
        available_weight_grams: 10,
        reserved_weight_grams: 0,
        event_sequence: 0,
        state_version: 0,
        last_event_hash: [0; 32],
        reserved_by: [0; 32],
        reserved_until: 0,
        flags: 0,
        current_rfid_hash: [0; 32],
        bump: 1,
    }
}

#[test]
fn enum_ranges_are_closed_and_stable() {
    // PURPOSE: enum values stored on-chain are validated against closed ranges.
    // FAILURE MEANS: unknown asset types or statuses could be written on-chain.
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
    // PURPOSE: closed and retired assets accept no further transitions.
    // FAILURE MEANS: a retired animal could receive new events.
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
        reserved_weight_grams: 0,
        event_sequence: 0,
        state_version: 0,
        last_event_hash: [0; 32],
        reserved_by: [0; 32],
        reserved_until: 0,
        flags: 0,
        current_rfid_hash: [0; 32],
        bump: 1,
    };
    assert!(asset.is_closed());
    asset.status = 7;
    assert!(asset.is_closed());
    asset.status = 1;
    assert!(!asset.is_closed());
}
