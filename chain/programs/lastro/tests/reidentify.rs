//! LiteSVM integration contracts for REIDENTIFY.

mod common;

use common::*;
use lastro::constants::{RFID_STATUS_ACTIVE, RFID_STATUS_RETIRED};
use solana_signer::Signer;

fn setup() -> (Harness, FlowEvents) {
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0x71; 32]);
    advance_to_transfer(&mut h, &flow);
    h.svm.expire_blockhash();
    (h, flow)
}

#[test]
fn reidentify_preserves_animal_id_and_custodian() {
    // PURPOSE: REIDENTIFY changes only the physical binding and related revision.
    // ARRANGE: Animal A, custodian B, RFID X, revision/sequence 1/2.
    let (mut h, flow) = setup();
    // ACTION: Execute REIDENTIFY X→Y with B signer.
    assert_success(send_event(&mut h.svm, &flow.reidentify, &h.station_signing_key, &h.station_pubkey33, &h.wallet_b));
    // ASSERT: AnimalID/custodian remain; current RFID=Y; revision=2; sequence=3; last hash is updated.
    let animal = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    assert_eq!(animal.animal_id, flow.animal_id);
    assert_eq!(animal.current_custodian.to_bytes(), h.wallet_b.pubkey().to_bytes());
    assert_eq!(animal.current_rfid_hash, flow.rfid_b);
    assert_eq!(animal.identity_revision, 2);
    assert_eq!(animal.event_sequence, 3);
    assert_eq!(animal.last_event_hash, flow.reidentify.event_hash());
    // FAILURE MEANS: Reidentification could become a transfer or create a new identity.
}

#[test]
fn reidentify_increments_revision_and_sequence_once() {
    // PURPOSE: Counters advance exactly one position.
    // ARRANGE: State revision 1/sequence 2.
    let (mut h, flow) = setup();
    // ACTION: Execute one valid REIDENTIFY, then rebuild the same envelope with a fresh blockhash.
    assert_success(send_event(&mut h.svm, &flow.reidentify, &h.station_signing_key, &h.station_pubkey33, &h.wallet_b));
    let once = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    h.svm.expire_blockhash();
    let replay = send_event(&mut h.svm, &flow.reidentify, &h.station_signing_key, &h.station_pubkey33, &h.wallet_b);
    // ASSERT: revision=2 and sequence=3 exactly; replay fails and leaves both unchanged.
    assert_eq!((once.identity_revision, once.event_sequence), (2, 3));
    assert_failure(replay);
    let after = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    assert_eq!((after.identity_revision, after.event_sequence), (2, 3));
    // FAILURE MEANS: Counters would lose their anti-stale meaning.
}

#[test]
fn reidentify_retires_old_binding_and_activates_new() {
    // PURPOSE: Old RFID history must remain while the new RFID becomes current.
    // ARRANGE: Binding X ACTIVE for Animal A; binding Y does not exist.
    let (mut h, flow) = setup();
    let (new_binding, _) = rfid_binding_pda(&h.deployment_id, &flow.rfid_b);
    assert!(account_data(&h.svm, &new_binding).is_none());
    // ACTION: Execute REIDENTIFY X→Y.
    assert_success(send_event(&mut h.svm, &flow.reidentify, &h.station_signing_key, &h.station_pubkey33, &h.wallet_b));
    // ASSERT: X=RETIRED pointing to A; Y=ACTIVE pointing to A; AnimalState.current=Y.
    let old = rfid_binding(&h.svm, &h.deployment_id, &flow.rfid_a);
    let new = rfid_binding(&h.svm, &h.deployment_id, &flow.rfid_b);
    assert_eq!((old.status, old.animal_id), (RFID_STATUS_RETIRED, flow.animal_id));
    assert_eq!((new.status, new.animal_id), (RFID_STATUS_ACTIVE, flow.animal_id));
    assert_eq!(animal_state(&h.svm, &h.deployment_id, &flow.animal_id).current_rfid_hash, flow.rfid_b);
    // FAILURE MEANS: Historical lookup/uniqueness could diverge.
}

#[test]
fn reidentify_rejects_same_new_rfid() {
    // PURPOSE: REIDENTIFY must represent a real identifier change.
    // ARRANGE: Current state uses X and signed REIDENTIFY says old=X,new=X.
    let (mut h, flow) = setup();
    let mut wrong = flow.reidentify.clone();
    wrong.new_rfid_hash = flow.rfid_a;
    let before = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    // ACTION: Submit the no-op physical change.
    let result = send_event(&mut h.svm, &wrong, &h.station_signing_key, &h.station_pubkey33, &h.wallet_b);
    // ASSERT: It fails without incrementing revision/sequence.
    assert_failure(result);
    let after = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    assert_eq!(after.identity_revision, before.identity_revision);
    assert_eq!(after.event_sequence, before.event_sequence);
    assert_eq!(after.current_rfid_hash, before.current_rfid_hash);
    // FAILURE MEANS: Revision could advance without a physical change.
}

#[test]
fn reidentify_rejects_new_rfid_already_known() {
    // PURPOSE: The new RFID cannot belong to any previous history.
    // ARRANGE: Create Animal B with RFID Y, then attempt Animal A X→Y.
    let mut h = Harness::new();
    h.initialize();
    let flow_a = h.flow([0x81; 32]);
    let mut flow_b = h.flow([0x82; 32]);
    flow_b.origin.new_rfid_hash = flow_a.rfid_b;
    assert_success(send_event(&mut h.svm, &flow_b.origin, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a));
    h.svm.expire_blockhash();
    assert_success(send_event(&mut h.svm, &flow_a.origin, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a));
    h.svm.expire_blockhash();
    assert_success(send_event(&mut h.svm, &flow_a.transfer_ab, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a));
    let before_animal = account_data(&h.svm, &animal_state_pda(&h.deployment_id, &flow_a.animal_id).0).unwrap();
    let before_old = account_data(&h.svm, &rfid_binding_pda(&h.deployment_id, &flow_a.rfid_a).0).unwrap();
    h.svm.expire_blockhash();
    // ACTION: Attempt REIDENTIFY X→Y where Y already has a global binding PDA.
    let result = send_event(&mut h.svm, &flow_a.reidentify, &h.station_signing_key, &h.station_pubkey33, &h.wallet_b);
    // ASSERT: It fails atomically before changing AnimalState or X.
    assert_failure(result);
    assert_eq!(account_data(&h.svm, &animal_state_pda(&h.deployment_id, &flow_a.animal_id).0).unwrap(), before_animal);
    assert_eq!(account_data(&h.svm, &rfid_binding_pda(&h.deployment_id, &flow_a.rfid_a).0).unwrap(), before_old);
    // FAILURE MEANS: A retired/occupied RFID could be recycled and create ambiguity.
}

#[test]
fn reidentify_requires_current_custodian_signer() {
    // PURPOSE: Only the current custodian authorizes rebinding.
    // ARRANGE: State custodian B; valid evidence, but adversarial instruction names A as signer account.
    let (mut h, flow) = setup();
    let mut envelope = build_envelope(&flow.reidentify, &h.station_signing_key, &h.station_pubkey33);
    envelope[1].accounts[0].pubkey = h.wallet_a.pubkey();
    // ACTION: Submit with A, then submit the canonical instruction with B.
    let wrong = send_event_with_envelope(&mut h.svm, envelope, &h.wallet_a);
    // ASSERT: A fails; B succeeds.
    assert_failure_contains(wrong, "InvalidCustodian");
    h.svm.expire_blockhash();
    assert_success(send_event(&mut h.svm, &flow.reidentify, &h.station_signing_key, &h.station_pubkey33, &h.wallet_b));
    // FAILURE MEANS: An unauthorized operator could replace the canonical physical identifier.
}
