//! LiteSVM integration contracts for ORIGIN.

mod common;

use common::*;
use lastro::constants::RFID_STATUS_ACTIVE;
use lastro_protocol::rfid::{canonical_rfid_from_u64, hash_canonical_rfid};

#[test]
fn origin_creates_animal_state_and_active_rfid_binding() {
    // PURPOSE: ORIGIN materializes canonical identity and the current binding.
    // ARRANGE: Config D/K; ORIGIN seq1/rev1, RFID A, custodian A; valid Secp; nonexistent PDAs.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0x11; 32]);
    // ACTION: Submit the Secp+origin envelope with Wallet A signer.
    assert_success(send_event(&mut h.svm, &flow.origin, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a));
    // ASSERT: AnimalState and RfidBinding(A) match the signed evidence exactly.
    let animal = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    assert_eq!(animal.animal_id, flow.animal_id);
    assert_eq!(animal.current_rfid_hash, flow.rfid_a);
    assert_eq!(animal.current_custodian.to_bytes(), flow.origin.to_custodian);
    assert_eq!(animal.identity_revision, 1);
    assert_eq!(animal.event_sequence, 1);
    assert_eq!(animal.last_event_hash, flow.origin.event_hash());
    let binding = rfid_binding(&h.svm, &h.deployment_id, &flow.rfid_a);
    assert_eq!(binding.animal_id, flow.animal_id);
    assert_eq!(binding.rfid_hash, flow.rfid_a);
    assert_eq!(binding.status, RFID_STATUS_ACTIVE);
    // FAILURE MEANS: Initial state would not reflect signed physical evidence.
}

#[test]
fn origin_requires_sequence_one_revision_one_zero_predecessor() {
    // PURPOSE: History must start at one unique canonical position.
    // ARRANGE: Signed variants with one invalid origin field at a time.
    let mut variants = Vec::new();
    let base_h = Harness::new();
    let base = base_h.flow([0x21; 32]).origin;
    let mut wrong_sequence = base.clone(); wrong_sequence.event_sequence = 2; variants.push(wrong_sequence);
    let mut wrong_revision = base.clone(); wrong_revision.identity_revision = 2; variants.push(wrong_revision);
    let mut wrong_predecessor = base.clone(); wrong_predecessor.previous_event_hash = [7; 32]; variants.push(wrong_predecessor);
    let mut wrong_old_rfid = base.clone(); wrong_old_rfid.old_rfid_hash = [8; 32]; variants.push(wrong_old_rfid);

    for (index, event) in variants.into_iter().enumerate() {
        let mut h = Harness::new();
        h.initialize();
        let (animal, _) = animal_state_pda(&h.deployment_id, &event.animal_id);
        let (binding, _) = rfid_binding_pda(&h.deployment_id, &event.new_rfid_hash);
        // ACTION: Execute each cryptographically valid but semantically invalid ORIGIN.
        let result = send_event(&mut h.svm, &event, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a);
        // ASSERT: Every variant fails atomically without creating either state account.
        assert_failure(result);
        assert!(account_data(&h.svm, &animal).is_none(), "variant {index} created AnimalState");
        assert!(account_data(&h.svm, &binding).is_none(), "variant {index} created RfidBinding");
    }
    // FAILURE MEANS: History could start in the middle or carry a fictitious predecessor.
}

#[test]
fn origin_requires_to_custodian_signer() {
    // PURPOSE: The initial custodian must authorize assuming authority.
    // ARRANGE: Valid ORIGIN event with to=A, but rewrite signer account to B for the adversarial transaction.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0x31; 32]);
    let mut envelope = build_envelope(&flow.origin, &h.station_signing_key, &h.station_pubkey33);
    envelope[1].accounts[0].pubkey = h.wallet_b.pubkey();
    // ACTION: Submit with signer B, then submit the canonical envelope with signer A.
    let bad = send_event_with_envelope(&mut h.svm, envelope, &h.wallet_b);
    // ASSERT: Wrong signer fails; Wallet A succeeds.
    assert_failure_contains(bad, "InvalidCustodian");
    h.svm.expire_blockhash();
    assert_success(send_event(&mut h.svm, &flow.origin, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a));
    // FAILURE MEANS: Backend/Station could assign custody without the corresponding wallet.
}

#[test]
fn origin_rejects_existing_animal_state() {
    // PURPOSE: One AnimalID cannot have two origins.
    // ARRANGE: Execute a valid ORIGIN for AnimalID X.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0x41; 32]);
    assert_success(send_event(&mut h.svm, &flow.origin, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a));
    let (animal_address, _) = animal_state_pda(&h.deployment_id, &flow.animal_id);
    let before = account_data(&h.svm, &animal_address).unwrap();
    let mut second = flow.origin.clone();
    second.new_rfid_hash = hash_canonical_rfid(&canonical_rfid_from_u64(0x8000_1300_0000_0099));
    h.svm.expire_blockhash();
    // ACTION: Submit a second signed ORIGIN for X with another RFID.
    let result = send_event(&mut h.svm, &second, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a);
    // ASSERT: It fails and the original canonical account is byte-for-byte unchanged.
    assert_failure(result);
    assert_eq!(account_data(&h.svm, &animal_address).unwrap(), before);
    // FAILURE MEANS: Two competing histories could be created for the same AnimalID.
}

#[test]
fn origin_rejects_rfid_ever_bound_to_another_history() {
    // PURPOSE: An already-known RFID cannot start a competing history.
    // ARRANGE: ORIGIN Animal A with RFID X.
    let mut h = Harness::new();
    h.initialize();
    let flow_a = h.flow([0x51; 32]);
    assert_success(send_event(&mut h.svm, &flow_a.origin, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a));
    let mut flow_b = h.flow([0x52; 32]);
    flow_b.origin.new_rfid_hash = flow_a.rfid_a;
    let (animal_b, _) = animal_state_pda(&h.deployment_id, &flow_b.animal_id);
    h.svm.expire_blockhash();
    // ACTION: Attempt ORIGIN Animal B using X.
    let result = send_event(&mut h.svm, &flow_b.origin, &h.station_signing_key, &h.station_pubkey33, &h.wallet_a);
    // ASSERT: It fails atomically because the global binding PDA already exists; B is not created.
    assert_failure(result);
    assert!(account_data(&h.svm, &animal_b).is_none());
    assert_eq!(rfid_binding(&h.svm, &h.deployment_id, &flow_a.rfid_a).animal_id, flow_a.animal_id);
    // FAILURE MEANS: The same physical identifier could authenticate two logical identities.
}
