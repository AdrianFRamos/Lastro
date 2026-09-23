//! REIDENTIFY semantic and successor contracts.

mod common;

use lastro_protocol::ProtocolError;

#[test]
fn reidentify_requires_new_rfid() {
    // PURPOSE: Ensure an identity revision corresponds to a physical RFID rebind.
    // ASSERT: REIDENTIFY with equal old and new RFID hashes is rejected.
    // FAILURE MEANS: Revision can advance without replacing the physical identifier.
    let mut event = common::event("reidentify");
    event.new_rfid_hash = event.old_rfid_hash;
    assert_eq!(
        event.validate_semantics(),
        Err(ProtocolError::InvalidSemantics)
    );
}

#[test]
fn reidentify_keeps_custodian() {
    // PURPOSE: Keep identity recovery separate from custody transfer.
    // ASSERT: REIDENTIFY with different source and destination custodians is rejected.
    // FAILURE MEANS: One event can combine two independent authority changes.
    let mut event = common::event("reidentify");
    event.to_custodian[0] ^= 1;
    assert_eq!(
        event.validate_semantics(),
        Err(ProtocolError::InvalidSemantics)
    );
}

#[test]
fn reidentify_advances_revision_exactly_once() {
    // PURPOSE: Enforce exact identity revision monotonicity.
    // ASSERT: The fixture REIDENTIFY succeeds after TRANSFER, while revision +2 fails successor validation.
    // FAILURE MEANS: Stale or skipped identity revisions can be accepted.
    let transfer = common::event("transfer");
    let reidentify = common::event("reidentify");
    assert!(reidentify.validate_successor(&transfer).is_ok());
    let mut skipped = reidentify;
    skipped.identity_revision += 1;
    assert_eq!(
        skipped.validate_successor(&transfer),
        Err(ProtocolError::InvalidSuccessor)
    );
}
