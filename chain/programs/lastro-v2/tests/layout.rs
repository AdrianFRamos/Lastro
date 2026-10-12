use anchor_lang::{AnchorSerialize, prelude::Pubkey};
use lastro_v2::{
    constants::{is_valid_asset_type, is_valid_facility_type, is_valid_party_role},
    state::{
        AssetState, BatchAnchor, EventAnchor, FacilityRecord, IntentState, LineageAnchor,
        PartyRecord, ProtocolConfigV2, RegistryRoot, RfidBinding, SessionGrant, StationRecord,
        TransformationAnchor, TransformationReservation,
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
            facility_id: [0; 32],
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
            geo_commitment: [0; 32],
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
            rent_payer: key,
            bump: 0,
        },
        IntentState::SPACE,
    );
    assert_exact_space(
        "EventAnchor",
        &EventAnchor {
            event_hash: [0; 32],
            subject_id: [0; 32],
            state_version: 0,
            event_type: 0,
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

/// The API reads the reduced EventAnchor and IntentState at fixed offsets
/// (services/api/src/solana/rpc.rs: decode_v2_event_anchor, decode_v2_intent).
#[test]
fn event_anchor_and_intent_offsets_match_the_api_decoders() {
    // PURPOSE: S4b/S1 — the program's serialized bytes sit where the API decodes them.
    // FAILURE MEANS: the API would misread anchors on confirmation or refund the wrong payer.
    let anchor = EventAnchor {
        event_hash: [0x11; 32],
        subject_id: [0x22; 32],
        state_version: 5,
        event_type: 18,
        bump: 7,
    };
    let mut bytes = vec![0u8; 8];
    anchor.serialize(&mut bytes).unwrap();
    assert_eq!(bytes.len(), 83);
    assert_eq!(&bytes[8..40], &[0x11; 32]);
    assert_eq!(&bytes[40..72], &[0x22; 32]);
    assert_eq!(&bytes[72..80], &5u64.to_le_bytes());
    assert_eq!(&bytes[80..82], &18u16.to_le_bytes());
    assert_eq!(bytes[82], 7);

    let actor = Pubkey::new_from_array([0x41; 32]);
    let rent_payer = Pubkey::new_from_array([0x61; 32]);
    let intent = IntentState {
        intent_id: [0x31; 32],
        subject_id: [0x32; 32],
        intent_type: 2,
        expected_state_version: 9,
        nonce: 3,
        actor,
        status: 1,
        expires_at: 100,
        consumed_at: 0,
        payload_hash: [0x51; 32],
        rent_payer,
        bump: 4,
    };
    let mut bytes = vec![0u8; 8];
    intent.serialize(&mut bytes).unwrap();
    assert_eq!(bytes.len(), 204);
    assert_eq!(&bytes[8..40], &[0x31; 32]);
    assert_eq!(&bytes[40..72], &[0x32; 32]);
    assert_eq!(&bytes[74..82], &9u64.to_le_bytes());
    assert_eq!(&bytes[90..122], actor.as_ref());
    assert_eq!(&bytes[171..203], rent_payer.as_ref());
    assert_eq!(bytes[203], 4);
}

/// API and browser decode SessionGrant (208) and BatchAnchor (201) at these offsets.
#[test]
fn session_and_batch_offsets_match_the_off_chain_decoders() {
    // PURPOSE: S3/S4a — off-chain readers find every session term where the program wrote it.
    // FAILURE MEANS: the verifier would check observations against the wrong window or Station.
    let custodian = Pubkey::new_from_array([0x41; 32]);
    let rent_payer = Pubkey::new_from_array([0x61; 32]);
    let session = SessionGrant {
        session_id: [0x11; 32],
        custodian,
        station_id: [0x22; 32],
        allowed_actions: 1,
        max_events: 2_000,
        events_used: 7,
        batch_count: 2,
        valid_from: 100,
        expires_at: 200,
        revoked_at: 150,
        status: 2,
        rent_payer,
        facility_id: [0xfa; 32],
        bump: 9,
    };
    let mut bytes = vec![0u8; 8];
    session.serialize(&mut bytes).unwrap();
    assert_eq!(bytes.len(), SessionGrant::SPACE);
    assert_eq!(bytes.len(), 208);
    assert_eq!(&bytes[8..40], &[0x11; 32]);
    assert_eq!(&bytes[40..72], custodian.as_ref());
    assert_eq!(&bytes[72..104], &[0x22; 32]);
    assert_eq!(&bytes[104..106], &1u16.to_le_bytes());
    assert_eq!(&bytes[106..110], &2_000u32.to_le_bytes());
    assert_eq!(&bytes[110..114], &7u32.to_le_bytes());
    assert_eq!(&bytes[114..118], &2u32.to_le_bytes());
    assert_eq!(&bytes[118..126], &100i64.to_le_bytes());
    assert_eq!(&bytes[126..134], &200i64.to_le_bytes());
    assert_eq!(&bytes[134..142], &150i64.to_le_bytes());
    assert_eq!(bytes[142], 2);
    assert_eq!(&bytes[143..175], rent_payer.as_ref());
    assert_eq!(&bytes[175..207], &[0xfa; 32]);
    assert_eq!(bytes[207], 9);

    let batch = BatchAnchor {
        session_id: [0x11; 32],
        batch_index: 3,
        custodian,
        station_id: [0x22; 32],
        merkle_root: [0x33; 32],
        leaf_count: 500,
        first_observed_at: 110,
        last_observed_at: 190,
        anchored_at: 300,
        facility_id: [0xfa; 32],
        bump: 4,
    };
    let mut bytes = vec![0u8; 8];
    batch.serialize(&mut bytes).unwrap();
    assert_eq!(bytes.len(), BatchAnchor::SPACE);
    assert_eq!(bytes.len(), 201);
    assert_eq!(&bytes[8..40], &[0x11; 32]);
    assert_eq!(&bytes[40..44], &3u32.to_le_bytes());
    assert_eq!(&bytes[44..76], custodian.as_ref());
    assert_eq!(&bytes[76..108], &[0x22; 32]);
    assert_eq!(&bytes[108..140], &[0x33; 32]);
    assert_eq!(&bytes[140..144], &500u32.to_le_bytes());
    assert_eq!(&bytes[144..152], &110i64.to_le_bytes());
    assert_eq!(&bytes[152..160], &190i64.to_le_bytes());
    assert_eq!(&bytes[160..168], &300i64.to_le_bytes());
    assert_eq!(&bytes[168..200], &[0xfa; 32]);
    assert_eq!(bytes[200], 4);
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
    assert!(!is_valid_party_role(0));
    assert!(!is_valid_party_role(13));
}

#[test]
fn exporter_is_its_own_stable_party_role() {
    // PURPOSE: the exporter (moves product out of the country) is a distinct on-chain role,
    //          separate from the transporter, distributor and retailer.
    // FAILURE MEANS: an exporter wallet would be indistinguishable from another link of the chain.
    use lastro_v2::constants::{
        PARTY_ROLE_DISTRIBUTOR, PARTY_ROLE_EXPORTER, PARTY_ROLE_RETAILER, PARTY_ROLE_TRANSPORTER,
    };
    assert_eq!(PARTY_ROLE_EXPORTER, 12);
    assert!(is_valid_party_role(PARTY_ROLE_EXPORTER));
    for other in [
        PARTY_ROLE_TRANSPORTER,
        PARTY_ROLE_DISTRIBUTOR,
        PARTY_ROLE_RETAILER,
    ] {
        assert_ne!(PARTY_ROLE_EXPORTER, other);
    }
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
