//! Capture-context state machine.
//!
//! A capture freezes state context that the Station may bind to one physical RFID
//! observation. Capture creation is **not** wallet authentication. The API has no wallet
//! session and must not pretend to know who will sign later. Authority is enforced when
//! transaction-data names the required signer and, decisively, by the Lastro program.
//!
//! Exactly one capture may be PENDING/DISPATCHED for the registered Station. The database
//! partial unique index is the race-safe final defense.
//!
//! Action derivation rules:
//! - ORIGIN: AnimalState/projection not originated; next_custodian is required/non-zero;
//!   seq=1, revision=1, predecessor/old RFID/from all zero, to=next_custodian.
//! - TRANSFER: current canonical/projection state must exist; next_custodian required,
//!   non-zero and different from current; seq=current+1, revision=current,
//!   predecessor/current RFID/from copied from current state, to=next_custodian.
//! - REIDENTIFY: next_custodian must be absent; current state must exist;
//!   seq=current+1, revision=current+1, predecessor/current old RFID copied from state,
//!   from=to=current custodian. The API never supplies new_rfid_hash.
//!
//! Expired/cancelled/consumed captures never accept evidence.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureStatus {
    Pending,
    Dispatched,
    EvidenceAccepted,
    Expired,
    Cancelled,
}
