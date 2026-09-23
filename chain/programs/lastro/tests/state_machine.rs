//! LiteSVM integration contracts for the complete Lastro state machine.

mod common;

use common::*;
use lastro::constants::{RFID_STATUS_ACTIVE, RFID_STATUS_RETIRED};
use solana_signer::Signer;

#[test]
fn full_origin_transfer_reidentify_transfer_reaches_expected_terminal_state() {
    // PURPOSE: Prove the complete hackathon state machine.
    // ARRANGE: Config + wallets A/B/C + RFID X/Y + four valid signed events.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xa1; 32]);
    // ACTION: Execute ORIGIN, A→B, REIDENTIFY X→Y, B→C.
    advance_full_flow(&mut h, &flow);
    // ASSERT: Final state is custodian C, RFID Y, revision2, sequence4, H4; X RETIRED,Y ACTIVE.
    let animal = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    assert_eq!(
        animal.current_custodian.to_bytes(),
        h.wallet_c.pubkey().to_bytes()
    );
    assert_eq!(animal.current_rfid_hash, flow.rfid_b);
    assert_eq!(animal.identity_revision, 2);
    assert_eq!(animal.event_sequence, 4);
    assert_eq!(animal.last_event_hash, flow.transfer_bc.event_hash());
    assert_eq!(
        rfid_binding(&h.svm, &h.deployment_id, &flow.rfid_a).status,
        RFID_STATUS_RETIRED
    );
    assert_eq!(
        rfid_binding(&h.svm, &h.deployment_id, &flow.rfid_b).status,
        RFID_STATUS_ACTIVE
    );
    // FAILURE MEANS: Instruction composition could fail even if isolated units pass.
}

#[test]
fn replay_of_any_consumed_event_fails() {
    // PURPOSE: Every consumed event is one-time.
    // ARRANGE: Execute each valid action once, then rebuild the consumed event with a fresh blockhash.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xa2; 32]);

    assert_success(send_event(
        &mut h.svm,
        &flow.origin,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_a,
    ));
    let snapshot = account_data(
        &h.svm,
        &animal_state_pda(&h.deployment_id, &flow.animal_id).0,
    )
    .unwrap();
    h.svm.expire_blockhash();
    // ACTION: Replay ORIGIN after it has already been consumed.
    assert_failure(send_event(
        &mut h.svm,
        &flow.origin,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_a,
    ));
    // ASSERT: The replay fails and state is unchanged.
    assert_eq!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow.animal_id).0
        )
        .unwrap(),
        snapshot
    );

    h.svm.expire_blockhash();
    assert_success(send_event(
        &mut h.svm,
        &flow.transfer_ab,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_a,
    ));
    let snapshot = account_data(
        &h.svm,
        &animal_state_pda(&h.deployment_id, &flow.animal_id).0,
    )
    .unwrap();
    h.svm.expire_blockhash();
    assert_failure(send_event(
        &mut h.svm,
        &flow.transfer_ab,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_a,
    ));
    assert_eq!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow.animal_id).0
        )
        .unwrap(),
        snapshot
    );

    h.svm.expire_blockhash();
    assert_success(send_event(
        &mut h.svm,
        &flow.reidentify,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_b,
    ));
    let snapshot = account_data(
        &h.svm,
        &animal_state_pda(&h.deployment_id, &flow.animal_id).0,
    )
    .unwrap();
    h.svm.expire_blockhash();
    assert_failure(send_event(
        &mut h.svm,
        &flow.reidentify,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_b,
    ));
    assert_eq!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow.animal_id).0
        )
        .unwrap(),
        snapshot
    );
    // FAILURE MEANS: Replay could duplicate transitions.
}

#[test]
fn stale_revision_after_reidentify_fails() {
    // PURPOSE: Pre-reidentification history never becomes valid again.
    // ARRANGE: State at revision2 after X→Y; signed TRANSFER carries revision1.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xa3; 32]);
    advance_to_reidentify(&mut h, &flow);
    let mut stale = flow.transfer_bc.clone();
    stale.identity_revision = 1;
    h.svm.expire_blockhash();
    // ACTION: Execute the stale-revision transfer.
    let result = send_event(
        &mut h.svm,
        &stale,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_b,
    );
    // ASSERT: It fails and canonical revision remains 2.
    assert_failure_contains(result, "InvalidRevision");
    assert_eq!(
        animal_state(&h.svm, &h.deployment_id, &flow.animal_id).identity_revision,
        2
    );
    // FAILURE MEANS: Old state could reappear.
}

#[test]
fn old_rfid_after_reidentify_fails() {
    // PURPOSE: A retired RFID cannot advance history.
    // ARRANGE: After X→Y, Station signs TRANSFER using X.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xa4; 32]);
    advance_to_reidentify(&mut h, &flow);
    let mut stale = flow.transfer_bc.clone();
    stale.old_rfid_hash = flow.rfid_a;
    stale.new_rfid_hash = flow.rfid_a;
    h.svm.expire_blockhash();
    // ACTION: Execute with the old tag hash.
    let result = send_event(
        &mut h.svm,
        &stale,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_b,
    );
    // ASSERT: It fails; current RFID remains Y and X stays retired.
    assert_failure(result);
    assert_eq!(
        animal_state(&h.svm, &h.deployment_id, &flow.animal_id).current_rfid_hash,
        flow.rfid_b
    );
    assert_eq!(
        rfid_binding(&h.svm, &h.deployment_id, &flow.rfid_a).status,
        RFID_STATUS_RETIRED
    );
    // FAILURE MEANS: An old tag could continue operating.
}

#[test]
fn fork_with_valid_station_signature_but_wrong_predecessor_fails() {
    // PURPOSE: A Station signature does not override canonical state.
    // ARRANGE: Two validly signed sequence-2 transfers from the same origin; execute A→B first.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xa5; 32]);
    assert_success(send_event(
        &mut h.svm,
        &flow.origin,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_a,
    ));
    let fork = lastro_protocol::StationEvent {
        to_custodian: h.wallet_c.pubkey().to_bytes(),
        ..flow.transfer_ab.clone()
    };
    h.svm.expire_blockhash();
    assert_success(send_event(
        &mut h.svm,
        &flow.transfer_ab,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_a,
    ));
    let before = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    h.svm.expire_blockhash();
    // ACTION: Execute the competing signed branch after canonical state advanced.
    let result = send_event(
        &mut h.svm,
        &fork,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_a,
    );
    // ASSERT: It fails despite valid Station proof; canonical state stays on the first branch.
    assert_failure(result);
    let after = animal_state(&h.svm, &h.deployment_id, &flow.animal_id);
    assert_eq!(after.current_custodian, before.current_custodian);
    assert_eq!(after.last_event_hash, before.last_event_hash);
    // FAILURE MEANS: Two signed histories could remain simultaneously valid.
}
