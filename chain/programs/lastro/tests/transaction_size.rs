//! Real serialized-size contracts for the wallet transaction envelope.

mod common;

use common::*;
use solana_signer::Signer;

fn measured_len(
    h: &Harness,
    event: &lastro_protocol::StationEvent,
    signer: &solana_keypair::Keypair,
) -> usize {
    let tx = build_transaction(
        &h.svm,
        build_envelope(event, &h.station_signing_key, &h.station_pubkey33),
        signer,
    );
    serialized_transaction_len(&tx)
}

#[test]
fn origin_serialized_transaction_fits_selected_format_limit() {
    // PURPOSE: Measure the complete ORIGIN envelope in the selected legacy format.
    // ARRANGE: Real transaction with fee payer, signature, all accounts, blockhash, Secp and Lastro.
    let h = Harness::new();
    let flow = h.flow([0xd1; 32]);
    // ACTION: Serialize with the same Solana transaction library used by LiteSVM.
    let len = measured_len(&h, &flow.origin, &h.wallet_a);
    eprintln!("ORIGIN serialized transaction bytes: {len}");
    // ASSERT: Complete wire transaction stays within Solana's 1232-byte packet limit.
    assert!(
        len <= TRANSACTION_LIMIT,
        "ORIGIN transaction is {len} bytes"
    );
    // FAILURE MEANS: ORIGIN could fail on the network because of size.
}

#[test]
fn transfer_serialized_transaction_fits_selected_format_limit() {
    // PURPOSE: Measure a complete TRANSFER, not an estimate.
    // ARRANGE: Real TRANSFER envelope including current-custodian fee payer/signature.
    let h = Harness::new();
    let flow = h.flow([0xd2; 32]);
    // ACTION: Serialize the signed legacy transaction.
    let len = measured_len(&h, &flow.transfer_ab, &h.wallet_a);
    eprintln!("TRANSFER serialized transaction bytes: {len}");
    // ASSERT: Serialized size is at most 1232 bytes.
    assert!(
        len <= TRANSACTION_LIMIT,
        "TRANSFER transaction is {len} bytes"
    );
    // FAILURE MEANS: TRANSFER could exceed the network limit because of its full account set.
}

#[test]
fn reidentify_serialized_transaction_fits_selected_format_limit() {
    // PURPOSE: Measure REIDENTIFY with old/new RfidBinding accounts.
    // ARRANGE: Real envelope with the largest Lastro account set.
    let h = Harness::new();
    let flow = h.flow([0xd3; 32]);
    // ACTION: Serialize the complete signed legacy transaction.
    let len = measured_len(&h, &flow.reidentify, &h.wallet_b);
    eprintln!("REIDENTIFY serialized transaction bytes: {len}");
    // ASSERT: Serialized size is at most 1232 bytes.
    assert!(
        len <= TRANSACTION_LIMIT,
        "REIDENTIFY transaction is {len} bytes"
    );
    // FAILURE MEANS: The heaviest action could fail only at runtime.
}

#[test]
fn size_test_includes_wallet_signature_accounts_and_both_instructions() {
    // PURPOSE: Prevent a false positive from partial measurement.
    // ARRANGE: Complete signed ORIGIN transaction.
    let h = Harness::new();
    let flow = h.flow([0xd4; 32]);
    let envelope = build_envelope(&flow.origin, &h.station_signing_key, &h.station_pubkey33);
    let tx = build_transaction(&h.svm, envelope.clone(), &h.wallet_a);
    // ACTION: Inspect structure before measuring and serialize the complete transaction.
    let len = serialized_transaction_len(&tx);
    // ASSERT: Wallet signature, fee payer, both instructions, Secp/Lastro program IDs, and all Lastro accounts are represented.
    assert_eq!(tx.signatures.len(), 1);
    assert_eq!(tx.message.instructions.len(), 2);
    assert_eq!(envelope[0].program_id, secp256r1_program_id());
    assert_eq!(envelope[1].program_id, program_id());
    assert_eq!(envelope[1].accounts.len(), 6);
    assert_eq!(tx.message.account_keys[0], h.wallet_a.pubkey());
    assert!(
        len > envelope[0].data.len() + envelope[1].data.len(),
        "measurement must include signatures/message/accounts"
    );
    assert!(len <= TRANSACTION_LIMIT);
    // FAILURE MEANS: The size metric would not represent the submitted payload.
}
