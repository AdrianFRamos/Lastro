//! LiteSVM integration contracts for TRANSFER.

mod common;

use common::*;
use lastro::constants::RFID_STATUS_ACTIVE;
use solana_signer::Signer;

fn setup() -> (Harness, FlowEvents) {
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0x61; 32]);
    assert_success(send_event(&mut h.svm, &flow.origin, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a));
    h.svm.expire_blockhash();
    (h, flow)
}

#[test]
fn transfer_a_to_b_updates_only_custodian_sequence_hash() {
    // PURPOSE: TRANSFER changes authority without changing physical identity/revision.
    // ARRANGE: State seq1/rev1/RFID X/custodian A/H1 and valid TRANSFER A→B.
    let (mut h, flow) = setup();
    // ACTION: Execute with signer A.
    assert_success(send_event(&mut h.svm, &flow.transfer_ab, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a));
    // ASSERT: Only custodian/sequence/hash advance; RFID/revision/binding stay unchanged.
    let animal = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    assert_eq!(animal.current_custodian.to_bytes(), h.wallet_b.pubkey().to_bytes());
    assert_eq!(animal.event_sequence, 2);
    assert_eq!(animal.last_event_hash, flow.transfer_ab.event_hash());
    assert_eq!(animal.current_rfid_hash, flow.rfid_a);
    assert_eq!(animal.identity_revision, 1);
    assert_eq!(rfid_binding(&h.svm, &h.deployment_id, &flow.rfid_a).status, RFID_STATUS_ACTIVE);
    // FAILURE MEANS: TRANSFER could reidentify or corrupt unrelated state.
}

#[test]
fn old_custodian_cannot_transfer_after_a_to_b() {
    // PURPOSE: Old authority loses power immediately after confirmation.
    // ARRANGE: State is already at B after A→B.
    let (mut h, flow) = setup();
    assert_success(send_event(&mut h.svm, &flow.transfer_ab, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a));
    let before = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    let stale = lastro_protocol::StationEvent {
        event_sequence: 3,
        previous_event_hash: flow.transfer_ab.event_hash(),
        from_custodian: h.wallet_a.pubkey().to_bytes(),
        to_custodian: h.wallet_c.pubkey().to_bytes(),
        ..flow.transfer_ab.clone()
    };
    h.svm.expire_blockhash();
    // ACTION: A signs a new TRANSFER after custody already belongs to B.
    let result = send_event(&mut h.svm, &stale, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a);
    // ASSERT: It fails and state remains B.
    assert_failure_contains(result, "InvalidCustodian");
    let after = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    assert_eq!(after.current_custodian, before.current_custodian);
    assert_eq!(after.event_sequence, before.event_sequence);
    assert_eq!(after.last_event_hash, before.last_event_hash);
    // FAILURE MEANS: The old custodian could create a fork.
}

#[test]
fn transfer_requires_physical_rfid_equal_current_binding() {
    // PURPOSE: TRANSFER requires a reread of the current RFID.
    // ARRANGE: Animal current RFID X; Station signs a transfer containing Y while the client still supplies X binding.
    let (mut h, flow) = setup();
    let mut wrong = flow.transfer_ab.clone();
    wrong.old_rfid_hash = flow.rfid_b;
    wrong.new_rfid_hash = flow.rfid_b;
    let before = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    let mut envelope = build_envelope(&wrong, &h.station_signing_key, &h.station_pubkey33);
    envelope[1].accounts[3].pubkey = rfid_binding_pda(&h.deployment_id, &flow.rfid_a).0;
    // ACTION: Execute the cryptographically valid envelope.
    let result = send_event_with_envelope(&mut h.svm, envelope, &h.wallet_a);
    // ASSERT: Account constraints/transition validation reject it atomically.
    assert_failure(result);
    let after = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    assert_eq!(after.event_sequence, before.event_sequence);
    assert_eq!(after.current_rfid_hash, before.current_rfid_hash);
    // FAILURE MEANS: A Station could transfer another tag under this animal context.
}

#[test]
fn transfer_rejects_sequence_gap() {
    // PURPOSE: Events cannot skip a position.
    // ARRANGE: State sequence 1 and a signed TRANSFER with sequence 3.
    let (mut h, flow) = setup();
    let mut wrong = flow.transfer_ab.clone();
    wrong.event_sequence = 3;
    // ACTION: Submit the gap event.
    let result = send_event(&mut h.svm, &wrong, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a);
    // ASSERT: It fails and sequence remains 1.
    assert_failure_contains(result, "InvalidSequence");
    assert_eq!(animal_state(&h.svm, &h.deployment_id, &flow.animal_id).event_sequence, 1);
    // FAILURE MEANS: Gaps could hide missing events.
}

#[test]
fn transfer_rejects_wrong_predecessor() {
    // PURPOSE: Every event must chain to the current last_event_hash.
    // ARRANGE: State last_hash H and a validly signed event pointing at H2.
    let (mut h, flow) = setup();
    let mut wrong = flow.transfer_ab.clone();
    wrong.previous_event_hash = [0x77; 32];
    let before = account_data(&h.svm, &animal_state_pda(&h.deployment_id, &flow.animal_id).0).unwrap();
    // ACTION: Submit the fork candidate.
    let result = send_event(&mut h.svm, &wrong, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a);
    // ASSERT: It fails; no mutation occurs.
    assert_failure_contains(result, "InvalidPredecessor");
    assert_eq!(account_data(&h.svm, &animal_state_pda(&h.deployment_id, &flow.animal_id).0).unwrap(), before);
    // FAILURE MEANS: A Station-signed fork could be accepted outside canonical state.
}

#[test]
fn transfer_rejects_revision_change() {
    // PURPOSE: TRANSFER cannot advance or decrease identity_revision.
    // ARRANGE: State revision 1 and signed TRANSFER variants revision 2 and 0.
    for revision in [2u32, 0u32] {
        let (mut h, flow) = setup();
        let mut wrong = flow.transfer_ab.clone();
        wrong.identity_revision = revision;
        // ACTION: Submit each revision-changing variant.
        let result = send_event(&mut h.svm, &wrong, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a);
        // ASSERT: It fails; revision remains 1.
        assert_failure(result);
        assert_eq!(animal_state(&h.svm, &h.deployment_id, &flow.animal_id).identity_revision, 1);
    }
    // FAILURE MEANS: A physical identity change could occur without REIDENTIFY.
}

#[test]
fn transfer_rejects_same_destination_as_current_custodian() {
    // PURPOSE: TRANSFER must represent a real custody change; a no-op must not consume sequence.
    // ARRANGE: State current custodian=A; signed TRANSFER from=A,to=A.
    let (mut h, flow) = setup();
    let mut wrong = flow.transfer_ab.clone();
    wrong.to_custodian = h.wallet_a.pubkey().to_bytes();
    // ACTION: Execute with signer A and otherwise valid context.
    let result = send_event(&mut h.svm, &wrong, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a);
    // ASSERT: It fails; sequence/hash/custodian remain unchanged.
    assert_failure(result);
    let animal = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    assert_eq!(animal.event_sequence, 1);
    assert_eq!(animal.current_custodian.to_bytes(), h.wallet_a.pubkey().to_bytes());
    assert_eq!(animal.last_event_hash, flow.origin.event_hash());
    // FAILURE MEANS: An empty TRANSFER could consume a history position and confuse auditing/UI.
}
