//! P0 integration contracts for the v2 foundation.

mod common;

use common::*;
use lastro_protocol::crypto::derive_station_id;
use lastro_v2::state::{
    AssetState, FacilityRecord, IntentState, ProtocolConfigV2, RegistryRoot, StationRecord,
};
use solana_signer::Signer;

#[test]
fn initialize_creates_config_and_all_registry_roots() {
    let mut h = Harness::new();
    let metadata = assert_success(send_initialize(&mut h.svm, &h.authority, h.deployment_id));
    assert!(metadata.compute_units_consumed > 0);

    let config: ProtocolConfigV2 = account(&h.svm, &config_pda(&h.deployment_id).0);
    assert_eq!(config.authority, h.authority.pubkey());
    assert_eq!(config.deployment_id, h.deployment_id);
    assert_eq!(config.schema_version, lastro_v2::constants::SCHEMA_VERSION);
    assert_eq!(
        config.station_registry,
        station_registry_pda(&h.deployment_id).0
    );
    assert_eq!(
        config.facility_registry,
        facility_registry_pda(&h.deployment_id).0
    );
    assert_eq!(
        config.party_registry,
        party_registry_pda(&h.deployment_id).0
    );
    assert_eq!(
        config.document_registry,
        document_registry_pda(&h.deployment_id).0
    );

    let station_root: RegistryRoot = account(&h.svm, &station_registry_pda(&h.deployment_id).0);
    let facility_root: RegistryRoot = account(&h.svm, &facility_registry_pda(&h.deployment_id).0);
    let party_root: RegistryRoot = account(&h.svm, &party_registry_pda(&h.deployment_id).0);
    let document_root: RegistryRoot = account(&h.svm, &document_registry_pda(&h.deployment_id).0);
    assert_eq!(station_root.deployment_id, h.deployment_id);
    assert_eq!(facility_root.deployment_id, h.deployment_id);
    assert_eq!(party_root.deployment_id, h.deployment_id);
    assert_eq!(document_root.deployment_id, h.deployment_id);
    assert_eq!(station_root.registry_type, 1);
    assert_eq!(facility_root.registry_type, 2);
    assert_eq!(party_root.registry_type, 3);
    assert_eq!(document_root.registry_type, 4);
}

#[test]
fn initialize_is_one_time_and_does_not_replace_config() {
    let mut h = Harness::new();
    h.initialize();
    let config_address = config_pda(&h.deployment_id).0;
    let before = h.svm.get_account(&config_address).expect("config").data;
    h.svm.expire_blockhash();

    assert_failure(send_initialize(&mut h.svm, &h.authority, h.deployment_id));
    let after = h.svm.get_account(&config_address).expect("config").data;
    assert_eq!(after, before);
}

#[test]
fn wrong_authority_cannot_register_asset() {
    let mut h = Harness::new();
    h.initialize();
    let asset_id = [0xa1; 32];
    h.svm.expire_blockhash();

    assert_failure(send_register_asset(
        &mut h.svm,
        &h.other,
        h.deployment_id,
        asset_id,
        h.other.pubkey(),
        500_000,
    ));
    assert!(!account_exists(
        &h.svm,
        &asset_pda(&h.deployment_id, &asset_id).0
    ));
}

#[test]
fn asset_registration_is_unique_and_enforces_weight_limit() {
    let mut h = Harness::new();
    h.initialize();
    let asset_id = [0xa2; 32];
    h.svm.expire_blockhash();

    assert_success(send_register_asset(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        asset_id,
        h.authority.pubkey(),
        500_000,
    ));
    let asset: AssetState = account(&h.svm, &asset_pda(&h.deployment_id, &asset_id).0);
    assert_eq!(asset.asset_id, asset_id);
    assert_eq!(asset.asset_type, 1);
    assert_eq!(asset.available_weight_grams, 500_000);
    assert_eq!(asset.state_version, 0);
    assert_eq!(asset.last_event_hash, [0; 32]);

    h.svm.expire_blockhash();
    assert_failure(send_register_asset(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        asset_id,
        h.authority.pubkey(),
        500_000,
    ));
    let still: AssetState = account(&h.svm, &asset_pda(&h.deployment_id, &asset_id).0);
    assert_eq!(still.state_version, 0);

    let too_heavy = [0xa3; 32];
    h.svm.expire_blockhash();
    assert_failure(send_register_asset(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        too_heavy,
        h.authority.pubkey(),
        2_000_001,
    ));
    assert!(!account_exists(
        &h.svm,
        &asset_pda(&h.deployment_id, &too_heavy).0
    ));
}

#[test]
fn station_registration_requires_valid_p256_key_and_derived_identity() {
    let mut h = Harness::new();
    h.initialize();
    let invalid_station = [0xb1; 32];
    h.svm.expire_blockhash();
    assert_failure(send_register_station(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        invalid_station,
        &[0x04; 33],
    ));
    assert!(!account_exists(
        &h.svm,
        &station_pda(&h.deployment_id, &invalid_station).0
    ));

    let key = signing_key();
    let pubkey33 = compressed_pubkey(&key);
    let station_id = derive_station_id(&pubkey33).expect("valid station key");
    h.svm.expire_blockhash();
    assert_success(send_register_station(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        station_id,
        pubkey33,
    ));
    let station: StationRecord = account(&h.svm, &station_pda(&h.deployment_id, &station_id).0);
    assert_eq!(station.station_id, station_id);
    assert_eq!(station.pubkey33, pubkey33);
    assert_eq!(station.status, lastro_v2::constants::STATION_STATUS_ACTIVE);
}

#[test]
fn facility_and_party_accounts_are_scoped_to_deployment() {
    let mut h = Harness::new();
    h.initialize();
    let facility_id = [0xc1; 32];
    let party_id = [0xc2; 32];
    let (config, _) = config_pda(&h.deployment_id);
    let (facility_root, _) = facility_registry_pda(&h.deployment_id);
    let (party_root, _) = party_registry_pda(&h.deployment_id);
    let (facility, _) = facility_pda(&h.deployment_id, &facility_id);
    let (party, _) = party_pda(&h.deployment_id, &party_id);

    let facility_ix = solana_instruction::Instruction {
        program_id: program_id(),
        accounts: vec![
            solana_instruction::AccountMeta::new(h.authority.pubkey(), true),
            solana_instruction::AccountMeta::new_readonly(config, false),
            solana_instruction::AccountMeta::new_readonly(facility_root, false),
            solana_instruction::AccountMeta::new(facility, false),
            solana_instruction::AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::RegisterFacility {
            facility_id,
            owner: h.authority.pubkey(),
            facility_type: lastro_v2::constants::FACILITY_TYPE_SLAUGHTERHOUSE,
            credential_hash: [0xc3; 32],
            valid_from: 1,
            valid_until: 86_400,
        }
        .data(),
    };
    assert_success(send_instructions(
        &mut h.svm,
        vec![facility_ix],
        &h.authority,
    ));
    let facility_account: FacilityRecord = account(&h.svm, &facility);
    assert_eq!(facility_account.facility_id, facility_id);
    assert_eq!(facility_account.owner, h.authority.pubkey());

    h.svm.expire_blockhash();
    let party_ix = solana_instruction::Instruction {
        program_id: program_id(),
        accounts: vec![
            solana_instruction::AccountMeta::new(h.authority.pubkey(), true),
            solana_instruction::AccountMeta::new_readonly(config, false),
            solana_instruction::AccountMeta::new_readonly(party_root, false),
            solana_instruction::AccountMeta::new(party, false),
            solana_instruction::AccountMeta::new_readonly(system_program_id(), false),
        ],
        data: lastro_v2::instruction::RegisterParty {
            party_id,
            wallet: h.authority.pubkey(),
            role: lastro_v2::constants::PARTY_ROLE_SLAUGHTERHOUSE,
        }
        .data(),
    };
    assert_success(send_instructions(&mut h.svm, vec![party_ix], &h.authority));
    assert!(account_exists(&h.svm, &party));
}

#[test]
fn intent_is_bound_to_actor_and_can_be_consumed_only_once() {
    let mut h = Harness::new();
    h.initialize();
    let subject_id = [0xd1; 32];
    let intent_id = [0xd2; 32];
    let payload_hash = [0xd3; 32];
    h.svm.expire_blockhash();

    assert_success(send_create_intent(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        subject_id,
        intent_id,
        60,
        payload_hash,
    ));
    let address = intent_pda(&h.deployment_id, &subject_id, &intent_id).0;
    let intent: IntentState = account(&h.svm, &address);
    assert_eq!(intent.actor, h.authority.pubkey());
    assert_eq!(intent.status, lastro_v2::constants::INTENT_STATUS_OPEN);

    h.svm.expire_blockhash();
    assert_failure(send_cancel_intent(
        &mut h.svm,
        &h.other,
        h.deployment_id,
        subject_id,
        intent_id,
    ));
    assert_eq!(
        account::<IntentState>(&h.svm, &address).status,
        lastro_v2::constants::INTENT_STATUS_OPEN
    );

    h.svm.expire_blockhash();
    assert_success(send_consume_intent(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        subject_id,
        intent_id,
        payload_hash,
    ));
    assert_eq!(
        account::<IntentState>(&h.svm, &address).status,
        lastro_v2::constants::INTENT_STATUS_CONSUMED
    );

    h.svm.expire_blockhash();
    assert_failure(send_consume_intent(
        &mut h.svm,
        &h.authority,
        h.deployment_id,
        subject_id,
        intent_id,
        payload_hash,
    ));
    assert_eq!(
        account::<IntentState>(&h.svm, &address).status,
        lastro_v2::constants::INTENT_STATUS_CONSUMED
    );
}
