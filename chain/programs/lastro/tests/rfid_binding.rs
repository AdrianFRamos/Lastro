//! LiteSVM integration contracts for the global RFID history index.

mod common;

use common::*;
use lastro::constants::{RFID_STATUS_ACTIVE, RFID_STATUS_RETIRED};

#[test]
fn only_one_active_binding_can_point_to_an_rfid() {
    // PURPOSE: One RFID hash cannot be current for two histories.
    // ARRANGE: Create binding X for Animal A.
    let mut h = Harness::new();
    h.initialize();
    let flow_a = h.flow([0x91; 32]);
    assert_success(send_event(
        &mut h.svm,
        &flow_a.origin,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_a,
    ));
    let mut flow_b = h.flow([0x92; 32]);
    flow_b.origin.new_rfid_hash = flow_a.rfid_a;
    h.svm.expire_blockhash();
    // ACTION: Attempt ORIGIN Animal B to X.
    let result = send_event(
        &mut h.svm,
        &flow_b.origin,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_a,
    );
    // ASSERT: Second creation fails; the single PDA X still points to A and is ACTIVE.
    assert_failure(result);
    let binding = rfid_binding(&h.svm, &h.deployment_id, &flow_a.rfid_a);
    assert_eq!(binding.animal_id, flow_a.animal_id);
    assert_eq!(binding.status, RFID_STATUS_ACTIVE);
    // FAILURE MEANS: RFID recovery would be ambiguous.
}

#[test]
fn retired_binding_keeps_original_animal_id() {
    // PURPOSE: Retirement preserves the historical link.
    // ARRANGE: Prepare A through transfer so B can REIDENTIFY X→Y.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0x93; 32]);
    advance_to_reidentify(&mut h, &flow);
    // ACTION: Read binding PDA X after reidentification.
    let binding = rfid_binding(&h.svm, &h.deployment_id, &flow.rfid_a);
    // ASSERT: status=RETIRED and original animal_id A remains; account still exists.
    assert_eq!(binding.status, RFID_STATUS_RETIRED);
    assert_eq!(binding.animal_id, flow.animal_id);
    assert!(account_data(&h.svm, &rfid_binding_pda(&h.deployment_id, &flow.rfid_a).0).is_some());
    // FAILURE MEANS: Previous physical history could be erased/reused.
}

#[test]
fn retired_rfid_cannot_be_reoriginated() {
    // PURPOSE: A retired RFID cannot open a new origin.
    // ARRANGE: Binding X is RETIRED after reidentify.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0x94; 32]);
    advance_to_reidentify(&mut h, &flow);
    let mut second = h.flow([0x95; 32]).origin;
    second.new_rfid_hash = flow.rfid_a;
    h.svm.expire_blockhash();
    // ACTION: Attempt ORIGIN for new Animal B with X.
    let result = send_event(
        &mut h.svm,
        &second,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_a,
    );
    // ASSERT: It fails because binding X still exists as historical state.
    assert_failure(result);
    assert_eq!(
        rfid_binding(&h.svm, &h.deployment_id, &flow.rfid_a).status,
        RFID_STATUS_RETIRED
    );
    // FAILURE MEANS: An old identifier could gain a new competing history.
}

#[test]
fn lookup_active_binding_matches_animal_state_current_rfid() {
    // PURPOSE: AnimalState and the RFID index must remain consistent.
    // ARRANGE: Execute the complete flow with current RFID Y.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0x96; 32]);
    advance_full_flow(&mut h, &flow);
    // ACTION: Read AnimalState and both RfidBinding accounts.
    let animal = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    let old = rfid_binding(&h.svm, &h.deployment_id, &flow.rfid_a);
    let current = rfid_binding(&h.svm, &h.deployment_id, &flow.rfid_b);
    // ASSERT: current hash Y matches ACTIVE Y; X remains RETIRED; both point to the same AnimalID.
    assert_eq!(animal.current_rfid_hash, flow.rfid_b);
    assert_eq!(
        (current.status, current.animal_id),
        (RFID_STATUS_ACTIVE, flow.animal_id)
    );
    assert_eq!(
        (old.status, old.animal_id),
        (RFID_STATUS_RETIRED, flow.animal_id)
    );
    // FAILURE MEANS: Two canonical on-chain sources could contradict each other.
}
