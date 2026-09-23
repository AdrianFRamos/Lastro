//! LiteSVM integration contracts for canonical account derivation.

mod common;

use common::*;

#[test]
fn animal_state_pda_uses_deployment_and_animal_id() {
    // PURPOSE: AnimalState must be namespaced by deployment and AnimalID.
    // ARRANGE: Valid ORIGIN plus an instruction whose AnimalState account is derived from another AnimalID.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xb1; 32]);
    let mut wrong = build_envelope(&flow.origin, &h.station_signing_key, &h.station_pubkey33);
    wrong[1].accounts[2].pubkey = animal_state_pda(&h.deployment_id, &[0xee; 32]).0;
    // ACTION: Execute with the wrong PDA, then execute the canonical envelope.
    let result = send_event_with_envelope(&mut h.svm, wrong, &h.wallet_a);
    // ASSERT: Wrong PDA fails; canonical ["animal",D,A] succeeds.
    assert_failure(result);
    h.svm.expire_blockhash();
    assert_success(send_event(
        &mut h.svm,
        &flow.origin,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_a,
    ));
    // FAILURE MEANS: An account from another animal/deployment could be mutated.
}

#[test]
fn rfid_binding_pda_uses_deployment_and_rfid_hash() {
    // PURPOSE: RfidBinding must be namespaced by deployment and RFID hash.
    // ARRANGE: Valid ORIGIN and a binding account derived from a different hash.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xb2; 32]);
    let mut wrong = build_envelope(&flow.origin, &h.station_signing_key, &h.station_pubkey33);
    wrong[1].accounts[3].pubkey = rfid_binding_pda(&h.deployment_id, &[0xdd; 32]).0;
    // ACTION: Execute the wrong binding PDA.
    let result = send_event_with_envelope(&mut h.svm, wrong, &h.wallet_a);
    // ASSERT: It fails; no AnimalState or binding is created.
    assert_failure(result);
    assert!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow.animal_id).0
        )
        .is_none()
    );
    assert!(account_data(&h.svm, &rfid_binding_pda(&h.deployment_id, &flow.rfid_a).0).is_none());
    // FAILURE MEANS: A binding for another RFID could be marked ACTIVE/RETIRED.
}

#[test]
fn protocol_config_pda_uses_deployment_id() {
    // PURPOSE: An event may use only the config for its own deployment.
    // ARRANGE: Initialize D1 and derive an unrelated D2 config address.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xb3; 32]);
    let mut wrong = build_envelope(&flow.origin, &h.station_signing_key, &h.station_pubkey33);
    wrong[1].accounts[1].pubkey = protocol_config_pda(&[0x7d; 32]).0;
    // ACTION: Submit a D1 event while providing config D2.
    let result = send_event_with_envelope(&mut h.svm, wrong, &h.wallet_a);
    // ASSERT: It fails before state transition and D1 animal state does not exist.
    assert_failure(result);
    assert!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow.animal_id).0
        )
        .is_none()
    );
    // FAILURE MEANS: A Station authorized in another deployment could sign here.
}

#[test]
fn wrong_pda_accounts_are_rejected() {
    // PURPOSE: Every derived account must be validated by the program, not the client.
    // ARRANGE: Valid ORIGIN instruction with syntactically present but wrong animal and RFID PDAs.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xb4; 32]);
    let mut wrong = build_envelope(&flow.origin, &h.station_signing_key, &h.station_pubkey33);
    wrong[1].accounts[2].pubkey = animal_state_pda(&h.deployment_id, &[0xaa; 32]).0;
    wrong[1].accounts[3].pubkey = rfid_binding_pda(&h.deployment_id, &[0xbb; 32]).0;
    // ACTION: Execute in LiteSVM.
    let result = send_event_with_envelope(&mut h.svm, wrong, &h.wallet_a);
    // ASSERT: Anchor seed constraints fail and canonical state remains absent.
    assert_failure(result);
    assert!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow.animal_id).0
        )
        .is_none()
    );
    // FAILURE MEANS: A malicious client could redirect writes to arbitrary accounts.
}
