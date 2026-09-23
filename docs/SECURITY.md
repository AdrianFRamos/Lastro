# Security model

## Protected properties

1. Station signature is bound to the exact bytes used by the program.
2. Current custodian is verified on-chain.
3. Sequence + predecessor make ordering explicit.
4. Revision advances exactly once on REIDENTIFY.
5. Current RFID binding is unique within one configured Solana deployment; other deployments can reuse the same RFID hash.
6. A retired RFID cannot silently create a second history.
7. PostgreSQL cannot rewrite canonical custody.

## Attacks that require tests

- missing precompile;
- wrong Station;
- valid signature over a different message;
- wrong offsets/indexes;
- old custodian;
- replay;
- sequence gap;
- wrong predecessor;
- stale revision;
- old RFID after REIDENTIFY;
- RFID already active for another AnimalID;
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
- fresh physical observation at a particular wall-clock time: StationEvent v1 has no signed timestamp or challenge;
- RFID reader integration on actual hardware until the documented reader gate has passed;
- eFuse signing and secure boot on a provisioned board until those hardware gates have passed.

The public `VITE_LASTRO_AUTHORITY` value must come from the operator's independently verified
deployment manifest, never from the EvidencePackage or an API response. The verifier compares it
with the authority stored in the finalized ProtocolConfig account and returns NOT_CHECKED when
it is missing. Initializing an arbitrary deployment remains permissionless, but an impostor
deployment cannot satisfy a separately pinned authority and deployment identifier.


## Development-only RustSec exceptions

The chain integration tests require LiteSVM with the `precompiles` feature so the suite executes the native Secp256r1 precompile instead of mocking the Station proof path. `litesvm` is declared only under `[dev-dependencies]` in `chain/programs/lastro/Cargo.toml`; it is not a dependency of the deployed Lastro program.

The current Agave precompile test stack pulls two legacy Dalek crates that RustSec reports as vulnerable:

- `RUSTSEC-2022-0093`: `ed25519-dalek 1.0.1`, transitively through `litesvm -> agave-precompiles`.
- `RUSTSEC-2024-0344`: `curve25519-dalek 3.2.0`, transitively through `ed25519-dalek 1.0.1`.

CI ignores only these two advisory IDs when auditing `chain/Cargo.lock`. The root workspace audit has no matching exception. Remove the exceptions as soon as the LiteSVM/Agave precompile dependency graph no longer requires the affected versions; do not broaden the ignore list to make unrelated advisories pass.
