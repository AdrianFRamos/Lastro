# Security model

## Protected properties

1. Station signature is bound to the exact bytes used by the program.
2. Identity changes require the current custodian's signature on-chain; presence proofs require the deployment authority.
3. `state_version` + `expected_previous_hash` make ordering explicit; each event id is anchored once.
4. RFID replacement retires the old binding and keeps the AssetID.
5. An RFID hash identifies at most one asset per deployment, forever (bindings are never closed); other deployments can reuse the same RFID hash.
6. Custody moves only when the named recipient accepts a live, state-bound proposal.
7. PostgreSQL cannot rewrite canonical custody or identity.

## Attacks that require tests

- missing precompile;
- wrong Station;
- valid signature over a different message;
- wrong offsets/indexes;
- old custodian;
- replay of an event id;
- wrong predecessor or stale state version;
- replaced RFID reused for any asset;
- RFID already bound to another AssetID;
- custody proposal accepted by someone other than the recipient, or after the state changed;
- altered/omitted/forked evidence package;
- backend projection different from RPC.

## What we do not prove

- biological identity;
- impossibility of physically removing a tag;
- legal ownership;
- regulatory compliance;
- absolute sensor truth;
- bilateral commercial acceptance.
- trusted deployment identity when the web verifier has no independently provisioned `VITE_LASTRO_AUTHORITY`;
- fresh physical observation at a particular wall-clock time: `observed_at` comes from the host and the Station has no trusted clock;
- RFID reader integration on actual hardware until the documented reader gate has passed;
- eFuse signing and secure boot on a provisioned board until those hardware gates have passed.

The public `VITE_LASTRO_AUTHORITY` value must come from the operator's independently verified
deployment manifest, never from the EvidencePackage or an API response. The verifier compares it
with the authority stored in the finalized ProtocolConfigV2 account and returns NOT_CHECKED when
it is missing. Initializing an arbitrary deployment remains permissionless, but an impostor
deployment cannot satisfy a separately pinned authority and deployment identifier.


## Development-only RustSec exceptions

The chain integration tests require LiteSVM with the `precompiles` feature so the suite executes the native Secp256r1 precompile instead of mocking the Station proof path. `litesvm` is declared only under `[dev-dependencies]` in `chain/programs/lastro-v2/Cargo.toml`; it is not a dependency of the deployed Lastro program.

The current Agave precompile test stack pulls two legacy Dalek crates that RustSec reports as vulnerable:

- `RUSTSEC-2022-0093`: `ed25519-dalek 1.0.1`, transitively through `litesvm -> agave-precompiles`.
- `RUSTSEC-2024-0344`: `curve25519-dalek 3.2.0`, transitively through `ed25519-dalek 1.0.1`.

CI ignores only these two advisory IDs when auditing `chain/Cargo.lock`. The root workspace audit has no matching exception. Remove the exceptions as soon as the LiteSVM/Agave precompile dependency graph no longer requires the affected versions; do not broaden the ignore list to make unrelated advisories pass.
