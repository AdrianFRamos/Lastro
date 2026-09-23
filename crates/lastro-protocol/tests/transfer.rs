//! TRANSFER semantic and successor contracts.

mod common;

use lastro_protocol::ProtocolError;

#[test]
fn transfer_keeps_same_rfid() {
    // PURPOSE: Keep custody changes separate from RFID replacement.
    // ASSERT: A TRANSFER whose old and new RFID hashes differ is rejected.
    // FAILURE MEANS: RFID can change without a REIDENTIFY revision.
    let mut event = common::event("transfer");
    event.new_rfid_hash[0] ^= 1;
    assert_eq!(
        event.validate_semantics(),
        Err(ProtocolError::InvalidSemantics)
    );
}

#[test]
fn transfer_keeps_revision() {
    // PURPOSE: Freeze identity revision across custody-only transitions.
    // ASSERT: The fixture TRANSFER succeeds after ORIGIN, while a changed revision fails successor validation.
    // FAILURE MEANS: TRANSFER can impersonate a REIDENTIFY operation.
    let origin = common::event("origin");
    let transfer = common::event("transfer");
    assert!(transfer.validate_successor(&origin).is_ok());
    let mut changed = transfer;
    changed.identity_revision += 1;
    assert_eq!(
        changed.validate_successor(&origin),
        Err(ProtocolError::InvalidSuccessor)
    );
}

#[test]
fn transfer_rejects_self_destination() {
    // PURPOSE: Prevent meaningless custody sequence advances.
    // ASSERT: TRANSFER with identical source and destination custodian is rejected.
    // FAILURE MEANS: Event sequence can advance without changing custody.
    let mut event = common::event("transfer");
    event.to_custodian = event.from_custodian;
    assert_eq!(
        event.validate_semantics(),
        Err(ProtocolError::InvalidSemantics)
    );
}
