//! ORIGIN semantic contracts.

mod common;

use lastro_protocol::{ProtocolError, StationEvent};

fn invalid(mut event: StationEvent) {
    assert_eq!(event.validate_semantics(), Err(ProtocolError::InvalidSemantics));
    let bytes = event.encode();
    assert_eq!(StationEvent::decode(&bytes), Err(ProtocolError::InvalidSemantics));
}

#[test]
fn origin_requires_sequence_one() {
    // PURPOSE: Force each history to start at sequence one.
    // ASSERT: ORIGIN with any later sequence is rejected.
    // FAILURE MEANS: A history can begin midstream.
    let mut event = common::event("origin");
    event.event_sequence = 2;
    invalid(event);
}

#[test]
fn origin_requires_revision_one() {
    // PURPOSE: Define the first identity revision unambiguously.
    // ASSERT: ORIGIN with revision other than one is rejected.
    // FAILURE MEANS: Identity revision loses monotonic meaning.
    let mut event = common::event("origin");
    event.identity_revision = 2;
    invalid(event);
}

#[test]
fn origin_requires_zero_predecessor_and_old_rfid() {
    // PURPOSE: Prove ORIGIN has no predecessor or prior RFID binding.
    // ASSERT: Nonzero predecessor or old RFID hash is rejected.
    // FAILURE MEANS: A forged prior history can be spliced into creation.
    let mut predecessor = common::event("origin");
    predecessor.previous_event_hash[0] = 1;
    invalid(predecessor);
    let mut old_rfid = common::event("origin");
    old_rfid.old_rfid_hash[0] = 1;
    invalid(old_rfid);
}

#[test]
fn origin_requires_zero_from_and_nonzero_to() {
    // PURPOSE: Define initial custody authority.
    // ASSERT: A nonzero source or zero destination custodian is rejected.
    // FAILURE MEANS: Initial custody can be ambiguous or ownerless.
    let mut source = common::event("origin");
    source.from_custodian[0] = 1;
    invalid(source);
    let mut destination = common::event("origin");
    destination.to_custodian = [0; 32];
    invalid(destination);
}
