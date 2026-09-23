//! LiteSVM integration contracts binding Secp256r1 verification to the exact StationEvent.
//! The shared harness constructs `LiteSVM::new().with_precompiles()` so these tests exercise the native precompile.

mod common;

use common::*;

#[test]
fn valid_secp_over_exact_event_is_accepted() {
    // PURPOSE: The cryptographic bridge accepts only the correct envelope.
    // ARRANGE: Config registers K; valid low-S signed ORIGIN; Secp ix0 points to event bytes in Lastro ix1.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xc1; 32]);
    // ACTION: Execute the exact two-instruction envelope.
    let result = send_event(
        &mut h.svm,
        &flow.origin,
        &h.station_signing_key,
        &h.station_pubkey33,
        &h.wallet_a,
    );
    // ASSERT: Precompile and Lastro both accept and create canonical state.
    assert_success(result);
    assert_eq!(
        animal_state(&h.svm, &h.deployment_id, &flow.animal_id).last_event_hash,
        flow.origin.event_hash()
    );
    // FAILURE MEANS: The Station→chain happy path would be broken.
}

#[test]
fn missing_secp_instruction_fails() {
    // PURPOSE: A wallet alone does not replace physical evidence.
    // ARRANGE: Valid Lastro instruction and correct signer, without Secp256r1 instruction.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xc2; 32]);
    let lastro_ix = build_lastro_instruction(&flow.origin);
    // ACTION: Execute only Lastro.
    let result = send_event_with_envelope(&mut h.svm, vec![lastro_ix], &h.wallet_a);
    // ASSERT: Station-proof validation fails and state does not exist.
    assert_failure(result);
    assert!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow.animal_id).0
        )
        .is_none()
    );
    // FAILURE MEANS: Backend/wallet could fabricate a transition without a Station signature.
}

#[test]
fn wrong_station_pubkey_fails() {
    // PURPOSE: The signature must use exactly the registered Station.
    // ARRANGE: Config contains K1; event bytes are validly signed by K2 and Secp carries K2.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xc3; 32]);
    let k2 = signing_key_from_small_scalar(2);
    let k2_pub = compressed_pubkey(&k2);
    let lastro_ix = build_lastro_instruction(&flow.origin);
    let secp_ix = build_secp_instruction(
        &flow.origin.encode(),
        &k2,
        &k2_pub,
        SecpDescriptor::default(),
    );
    // ACTION: Execute valid Secp for K2 + Lastro configured for K1.
    let result = send_event_with_envelope(&mut h.svm, vec![secp_ix, lastro_ix], &h.wallet_a);
    // ASSERT: Precompile may accept K2, but Lastro rejects key mismatch; no state is created.
    assert_failure_contains(result, "InvalidStationProof");
    assert!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow.animal_id).0
        )
        .is_none()
    );
    // FAILURE MEANS: Any Station could operate the deployment.
}

#[test]
fn signature_and_public_key_descriptor_offsets_and_indexes_are_exact() {
    // PURPOSE: Every descriptor field locating signature/key bytes is part of the frozen Station proof envelope.
    // ARRANGE: Build four otherwise-valid ORIGIN envelopes, changing one signature/key offset/index field at a time.
    let variants = [
        SecpDescriptor {
            signature_offset: SECP_SIGNATURE_OFFSET + 1,
            ..SecpDescriptor::default()
        },
        SecpDescriptor {
            signature_instruction_index: 1,
            ..SecpDescriptor::default()
        },
        SecpDescriptor {
            public_key_offset: SECP_PUBLIC_KEY_OFFSET + 1,
            ..SecpDescriptor::default()
        },
        SecpDescriptor {
            public_key_instruction_index: 1,
            ..SecpDescriptor::default()
        },
    ];

    for (index, descriptor) in variants.into_iter().enumerate() {
        let mut h = Harness::new();
        h.initialize();
        let flow = h.flow([0xb0 + index as u8; 32]);
        let secp = build_secp_instruction(
            &flow.origin.encode(),
            &h.station_signing_key,
            &h.station_pubkey33,
            descriptor,
        );
        let lastro_ix = build_lastro_instruction(&flow.origin);
        // ACTION: Submit the malformed descriptor through native Secp256r1 + Lastro execution.
        let result = send_event_with_envelope(&mut h.svm, vec![secp, lastro_ix], &h.wallet_a);
        // ASSERT: Every field deviation fails atomically and creates no AnimalState.
        assert_failure(result);
        assert!(
            account_data(
                &h.svm,
                &animal_state_pda(&h.deployment_id, &flow.animal_id).0
            )
            .is_none()
        );
    }
    // FAILURE MEANS: A client could relocate signature/key bytes or reference another instruction.
}

#[test]
fn signature_over_other_276_bytes_fails() {
    // PURPOSE: The Secp descriptor must verify the same bytes processed by Lastro.
    // ARRANGE: Sign E1 while Lastro instruction carries same-sized, semantically valid E2.
    let mut h = Harness::new();
    h.initialize();
    let flow1 = h.flow([0xc4; 32]);
    let flow2 = h.flow([0xc5; 32]);
    let signature_e1 = low_s_signature(&h.station_signing_key, &flow1.origin.encode());
    let secp = build_secp_instruction_with_signature(
        signature_e1,
        &h.station_pubkey33,
        SecpDescriptor::default(),
    );
    let lastro_ix = build_lastro_instruction(&flow2.origin);
    // ACTION: Execute signature(E1) with Lastro(E2).
    let result = send_event_with_envelope(&mut h.svm, vec![secp, lastro_ix], &h.wallet_a);
    // ASSERT: The runtime precompile rejects the detached signature; E2 state is absent.
    assert_failure(result);
    assert!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow2.animal_id).0
        )
        .is_none()
    );
    // FAILURE MEANS: A detached signature could authorize a different event.
}

#[test]
fn message_offset_one_byte_wrong_fails() {
    // PURPOSE: Offset is part of envelope security.
    // ARRANGE: Correct ORIGIN transaction with descriptor offset changed by +1 and -1 in separate fresh harnesses.
    for offset in [
        STATION_EVENT_OFFSET_IN_ANCHOR_IX - 1,
        STATION_EVENT_OFFSET_IN_ANCHOR_IX + 1,
    ] {
        let mut h = Harness::new();
        h.initialize();
        let flow = h.flow([offset as u8; 32]);
        let descriptor = SecpDescriptor {
            message_offset: offset,
            ..SecpDescriptor::default()
        };
        let secp = build_secp_instruction(
            &flow.origin.encode(),
            &h.station_signing_key,
            &h.station_pubkey33,
            descriptor,
        );
        let lastro_ix = build_lastro_instruction(&flow.origin);
        // ACTION: Execute each malformed descriptor.
        let result = send_event_with_envelope(&mut h.svm, vec![secp, lastro_ix], &h.wallet_a);
        // ASSERT: Precompile/Lastro rejects it and no state is created.
        assert_failure(result);
        assert!(
            account_data(
                &h.svm,
                &animal_state_pda(&h.deployment_id, &flow.animal_id).0
            )
            .is_none()
        );
    }
    // FAILURE MEANS: The builder could point to an incorrect substring without detection.
}

#[test]
fn message_length_275_or_277_fails() {
    // PURPOSE: Signed length must be exactly 276.
    // ARRANGE: Descriptor points to the correct start but uses 275 and 277 in separate cases.
    for length in [275u16, 277u16] {
        let mut h = Harness::new();
        h.initialize();
        let flow = h.flow([length as u8; 32]);
        let descriptor = SecpDescriptor {
            message_length: length,
            ..SecpDescriptor::default()
        };
        let secp = build_secp_instruction(
            &flow.origin.encode(),
            &h.station_signing_key,
            &h.station_pubkey33,
            descriptor,
        );
        let lastro_ix = build_lastro_instruction(&flow.origin);
        // ACTION: Execute malformed-length envelope.
        let result = send_event_with_envelope(&mut h.svm, vec![secp, lastro_ix], &h.wallet_a);
        // ASSERT: Both malformed lengths fail and create no state.
        assert_failure(result);
        assert!(
            account_data(
                &h.svm,
                &animal_state_pda(&h.deployment_id, &flow.animal_id).0
            )
            .is_none()
        );
    }
    // FAILURE MEANS: A truncated/extended event could be accepted.
}

#[test]
fn wrong_instruction_index_fails() {
    // PURPOSE: The descriptor cannot point to another instruction.
    // ARRANGE: Correct envelope except message_instruction_index points at ix0 instead of Lastro ix1.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xc8; 32]);
    let descriptor = SecpDescriptor {
        message_instruction_index: 0,
        ..SecpDescriptor::default()
    };
    let secp = build_secp_instruction(
        &flow.origin.encode(),
        &h.station_signing_key,
        &h.station_pubkey33,
        descriptor,
    );
    let lastro_ix = build_lastro_instruction(&flow.origin);
    // ACTION: Execute.
    let result = send_event_with_envelope(&mut h.svm, vec![secp, lastro_ix], &h.wallet_a);
    // ASSERT: Runtime/Lastro rejects it and state remains absent.
    assert_failure(result);
    assert!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow.animal_id).0
        )
        .is_none()
    );
    // FAILURE MEANS: An attacker could supply a signed message outside the processed instruction.
}

#[test]
fn secp_after_lastro_fails_when_program_requires_expected_order() {
    // PURPOSE: Envelope order is frozen to reduce ambiguity.
    // ARRANGE: Build Lastro ix0 and Secp ix1.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xc9; 32]);
    let mut envelope = build_envelope(&flow.origin, &h.station_signing_key, &h.station_pubkey33);
    envelope.swap(0, 1);
    // ACTION: Execute reversed order.
    let result = send_event_with_envelope(&mut h.svm, envelope, &h.wallet_a);
    // ASSERT: It fails and does not create state.
    assert_failure(result);
    assert!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow.animal_id).0
        )
        .is_none()
    );
    // FAILURE MEANS: Different clients could create semantically divergent envelopes.
}

#[test]
fn high_s_signature_is_rejected_by_runtime_precompile() {
    // PURPOSE: Host and chain must agree on low-S.
    // ARRANGE: Create the mathematically equivalent high-S form of a valid Station signature.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xca; 32]);
    let low = low_s_signature(&h.station_signing_key, &flow.origin.encode());
    let high = high_s_from_low_s(low);
    let secp =
        build_secp_instruction_with_signature(high, &h.station_pubkey33, SecpDescriptor::default());
    let lastro_ix = build_lastro_instruction(&flow.origin);
    // ACTION: Execute Secp256r1 + Lastro.
    let result = send_event_with_envelope(&mut h.svm, vec![secp, lastro_ix], &h.wallet_a);
    // ASSERT: Runtime rejects high-S and Lastro does not advance state.
    assert_failure(result);
    assert!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow.animal_id).0
        )
        .is_none()
    );
    // FAILURE MEANS: Host could accept evidence that the chain will never accept.
}

#[test]
fn extra_instruction_fails_frozen_two_instruction_envelope() {
    // PURPOSE: The Station proof is bound to the frozen two-instruction transaction shape.
    // ARRANGE: Valid Secp+Lastro envelope plus an additional Lastro-shaped instruction at index 2.
    let mut h = Harness::new();
    h.initialize();
    let flow = h.flow([0xcb; 32]);
    let mut envelope = build_envelope(&flow.origin, &h.station_signing_key, &h.station_pubkey33);
    envelope.push(build_lastro_instruction(&flow.origin));
    // ACTION: Execute the three-instruction transaction.
    let result = send_event_with_envelope(&mut h.svm, envelope, &h.wallet_a);
    // ASSERT: Lastro rejects instruction 2 existence and no state is committed.
    assert_failure_contains(result, "InvalidStationProof");
    assert!(
        account_data(
            &h.svm,
            &animal_state_pda(&h.deployment_id, &flow.animal_id).0
        )
        .is_none()
    );
    // FAILURE MEANS: A client could alter the signed transaction envelope without detection.
}
