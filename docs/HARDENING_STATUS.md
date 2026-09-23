# Hardening implementation status

## Implemented in code

- Browser type errors were corrected, and signed transactions now retain their last valid block
  height. When an unconfirmed transaction has expired and RPC history has no signature, the UI
  keeps accepted physical evidence while allowing a fresh wallet signature.
- Capture challenges have atomic PostgreSQL issuance budgets, a cheap preflight before Solana RPC,
  and bounded retention. Anonymous animal registration has an atomic deployment-wide budget.
- A custodian can explicitly authorize replacing one accepted REIDENTIFY capture by naming its
  capture ID in the wallet-signed challenge. Ordinary retries remain idempotent. PostgreSQL uses
  conditional updates so a concurrently submitted event cannot be superseded.
- Station WAIT_RFID has a monotonic deadline. The Agent treats Station capture errors and serial
  timeouts as retryable without discarding signed evidence.
- Evidence export rejects more than 128 events before materializing the complete history. The
  browser enforces the same cap and a 30-second total Solana RPC budget.
- The independent verifier compares the package deployment and finalized ProtocolConfig authority
  with separately configured browser trust anchors. Without the authority it reports NOT_CHECKED.
- ProtocolConfig initialization waits for the fee payer's funding at finalized commitment.
- Public copy now describes software verification and physical hardware gates separately.

## Remaining protocol and hardware constraints

1. The repository has no selected RFID reader model, pinout, raw frames, or integrity rule. A
   real reader driver cannot be safely written or claimed complete until that evidence exists.
2. Secure boot and eFuse signing need provisioning and physical validation. Firmware code cannot
   establish that a specific board has the intended irreversible fuse settings. ProtocolConfig
   stores one immutable Station key; key rotation and revocation require a versioned on-chain
   registry and verifier support for historical keys.
3. StationEvent v1 has no signed nonce or time. Its sequence and predecessor prevent replay of an
   already finalized state, but do not prove that an observation was made at a particular time.
   End-to-end freshness requires a versioned StationEvent/on-chain challenge design and new
   cross-language fixtures.
4. A client can sign and broadcast the old accepted transition without reporting the signature
   before requesting a replacement. PostgreSQL cannot revoke an already signed Solana transaction.
   An on-chain cancellation/intent nonce is needed before unconditional safe supersession can be
   claimed. The explicit rescan is a demo recovery path and must be reconciled against canonical
   Solana finality before presenting a replacement as final.
5. A confirmed transaction can be rolled back before finalization. The persisted SUBMITTED
   signature is immutable; resetting it safely requires a server-verified signed envelope lifetime
   and proof that the old signature cannot land. The browser only replaces an unsubmitted,
   expired transaction after checking RPC history. Submitted rollback requires manual reconciliation.
6. A quarantined Agent outbox row remains available for operator inspection but cannot be
   re-admitted after capture expiry. Signing a new capture cannot turn an old physical observation
   into fresh evidence. Operators must recapture or use a future protocol version that records
   bounded, signed capture freshness.

Run the full verification gates and regenerate the derived test index and tracked-file manifest
in the real Git checkout before merging these changes. Do not infer a PASS from source review.
