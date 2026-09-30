# Testing strategy

## Main rule

A test is evidence only when it actually ran against the dependency boundary it claims to cover. Lastro uses two kinds of tests:

1. **normal executable tests**, which run whenever their toolchain/dependencies are present;
2. **explicit environment-gated tests**, whose bodies are fully implemented but skip unless real external infrastructure or hardware has been declared.

Never replace an assertion with a log, silently weaken a protocol invariant, or report an environment-gated test as passed when it was skipped.

## Pyramid applied to Lastro

### 1. Pure protocol

`crates/lastro-protocol/tests/`

Freezes the 220-byte v2 envelope, endianness, hash domains, capture rules and payload commitments, serial payloads, P-256 low-S behavior, lineage/transformation canonicalization, and cross-language vectors (`test-vectors/v2-capture.json`).

```bash
cargo test -p lastro-protocol
python3 scripts/check_vectors.py --verify-only
```

### 2. Station component

`firmware/station/components/lastro_station/test/`

Unity tests cover reader-independent canonicalization/hash helpers, byte-for-byte event construction, P-256 behavior, Station state machine, and serial framing. Tests that require a selected physical reader or eFuse remain hardware-gated because those facts cannot be simulated honestly.

```bash
idf.py -C firmware/station set-target esp32c5
idf.py -C firmware/station build
```

Target Unity execution requires the ESP32-C5 test app/hardware described in `docs/HARDWARE.md`.

### 3. Agent

`services/agent/tests/`

Covers fail-closed configuration, incremental serial framing/CRC, payload contracts, SQLite WAL durability, immutable outbox rows, restart recovery, retry/idempotency, and capture/event binding.

```bash
cargo test -p lastro-agent
```

### 4. API

`services/api/tests/`

Covers fail-closed configuration, resource bounds, authorization boundaries, and (with PostgreSQL) the v2 capture flow: wallet challenge, Station evidence admission and rejection, identity transaction data, confirmed/finalized registration and the v2 `EvidencePackage` (`tests/capture_v2.rs`).

```bash
LASTRO_DATABASE_URL=postgres://lastro:lastro@127.0.0.1:5432/lastro \
  cargo test -p lastro-api
```

### 5. Solana program

`chain/programs/lastro-v2/tests/`

LiteSVM contracts exercise the real Anchor instructions and the native Secp256r1 precompile: upgrade-authority initialization, asset/Station/facility/party registration, RFID bind/replace with retired bindings that can never be reused, custodian + Station authority for identity changes, two-phase custody transfer and stale proposals, config authority rotation, transformations and exact account layouts.

```bash
make bootstrap-program-id   # once for a real local identity
cd chain
anchor build --ignore-keys
cargo test --locked --workspace
```

These tests require the pinned Rust/Anchor/Solana toolchain and built SBF artifact. Their presence in source is not a substitute for running them.

### 6. Browser unit/component tests

`apps/web/tests/`

Covers public deployment configuration, v2 envelope/hashes/P-256, API DTO validation, Wallet Standard authority/capability handling, exact transaction preflight (every account and data byte derived in the browser for Station events, registration and custody), operator state machine, broadcast/SUBMITTED reload restoration without duplicate wallet signatures, local `EvidencePackage` v2 verification, and canonical RPC comparison of accounts and finalized transactions.

```bash
npm ci
npm --workspace @lastro/web run typecheck
npm --workspace @lastro/web run test
npm --workspace @lastro/web run build
```

Use Node 24.21.0 and npm 11.19.0 as pinned by `.nvmrc`/`package.json`. The committed `package-lock.json` is required; generate resolver-owned locks with `scripts/bootstrap_dependencies.sh` before running release CI on a fresh checkout.

### 7. Browser E2E

`apps/web/e2e/`

Playwright checks the public routes (storytelling and 16:9 layout guardrails) against a built web
app. The v1 full-stack browser harness (PTY Station, deterministic wallets, fault injection) was
removed with the v1 protocol.

```bash
npm --workspace @lastro/web run test:e2e
```

### 8. Full local system gate (G3)

`tests/system/`

The v2 harness drives the real stack exactly as an operator would. It plays only the roles
outside the product boundary: the Station (the firmware-equivalent simulator state machine behind
a PTY, which the real `lastro-agent` opens as its serial port), the wallets (Solana keypair files;
the harness signs exactly the transaction the API prepared after checking the signer, program and
measured size) and the operator (operator-token calls for parties, facility and custody proposal).
Agent, API, PostgreSQL, the v2 program and the RPC are real.

`test_full_local.py` runs, on fresh assets and tags:

```text
register (authority A, custodian B) → bind tag A (B) → presence proof (B) → replace with tag B (B)
→ custody B→C (B proposes, C accepts)
```

and asserts the finalized canonical state, that tag A still resolves to the asset as `RETIRED`,
that the exported EvidencePackage verifies independently (locally and against finalized accounts
and transactions) while a one-byte change is rejected, that B can no longer authorize identity
changes after the transfer (401 before any Station work), and that forged Station evidence is
rejected while the genuine capture finalizes.

Required environment (POSIX host; `make local-demo-test` and the e2e workflow provide it):

```text
LASTRO_SYSTEM_TEST=1
LASTRO_SYSTEM_API_URL             API started with LASTRO_OPERATOR_TOKEN
LASTRO_SYSTEM_SOLANA_RPC_URL
LASTRO_PROGRAM_ID
LASTRO_DEPLOYMENT_ID_HEX
LASTRO_SYSTEM_AGENT_BIN           target/debug/lastro-agent
LASTRO_AGENT_TOKEN
LASTRO_OPERATOR_TOKEN
LASTRO_SYSTEM_STATION_PRIVATE_SCALAR_HEX   key registered by initialize_protocol_config.py
LASTRO_SYSTEM_WALLET_A_KEYPAIR    deployment authority (ProtocolConfigV2 authority)
LASTRO_SYSTEM_WALLET_B_KEYPAIR    funded custodian
LASTRO_SYSTEM_WALLET_C_KEYPAIR    funded recipient
```

```bash
LASTRO_SYSTEM_TEST=1 python3 -m pytest -q tests/system/test_full_local.py
```

Running the validator inside Docker requires `--security-opt seccomp=unconfined`:
`solana-test-validator` 4.2 uses `io_uring`, which the default seccomp profile blocks.

`test_support.py` checks the harness itself without a stack (independent package verifier;
PTY Station answering the frozen command with the frozen envelope) and runs in normal CI.

### 9. Devnet gate (G4)

```bash
LASTRO_DEVNET_TEST=1 python3 -m pytest -q tests/system/test_devnet.py
```

Same flow against the deployed Devnet program, with funded wallets and the registered Station key.
The manual workflow `.github/workflows/devnet.yml` validates the program identity and
`ProtocolConfigV2`/Station (`--validate-only`), starts the API against Devnet and runs G4. It fails
unless the repository contains exactly one real `declare_id!` matching `LASTRO_DEVNET_PROGRAM_ID`.
Configure the protected `devnet` GitHub Environment with:

```text
variables: LASTRO_DEVNET_PROGRAM_ID, LASTRO_DEVNET_DEPLOYMENT_ID_HEX, LASTRO_DEVNET_STATION_PUBKEY_HEX
secrets:   LASTRO_DEVNET_RPC_URL, LASTRO_AGENT_TOKEN, LASTRO_OPERATOR_TOKEN,
           LASTRO_DEVNET_STATION_PRIVATE_SCALAR_HEX,
           LASTRO_DEVNET_WALLET_A_KEYPAIR_JSON, LASTRO_DEVNET_WALLET_B_KEYPAIR_JSON,
           LASTRO_DEVNET_WALLET_C_KEYPAIR_JSON
```

Only public references (asset ID, transaction signatures) are written as artifacts.

### 10. Demo stability gate (G5)

```bash
LASTRO_DEMO_STABILITY_TEST=1 python3 -m pytest -q tests/system/test_demo_stability.py
```

Three consecutive complete runs with one Agent and Station, each package verified locally and
on-chain. The final physical demo repeats it with the real Station/reader and wallet extension.

### 11. Physical hardware and eFuse

`firmware/station/pytest/`

Run only after the reader gate in `docs/HARDWARE.md` is satisfied. The tests communicate with the real Station through the frozen Agent serial protocol; they do not synthesize a reader observation. Stop the Rust Agent first so the hardware test owns the serial port.

Install the hardware-test dependency and perform the machine-visible preflight:

```bash
python3 -m pip install -r requirements-dev.txt
LASTRO_AGENT_SERIAL_PORT=/dev/... ./scripts/hardware_preflight.sh
```

For G1, configure `LASTRO_DEPLOYMENT_ID_HEX` and the independently known 8-byte `LASTRO_HARDWARE_TAG_A_RFID_HEX`, then run (the host sends an `IDENTIFIER_BOUND` command and rebuilds the exact envelope the Station must sign):

```bash
LASTRO_HARDWARE_TEST=1 python3 -m pytest -s -q firmware/station/pytest/test_hardware.py
```

The two-tag stability test additionally requires interactive physical A/B swaps, `LASTRO_HARDWARE_TAG_B_RFID_HEX`, and retained raw reader-frame files. The reboot test requires `LASTRO_HARDWARE_REBOOT_COMMAND`; the command is executed directly without a shell.

For H1, provision the key manually as documented in `docs/HARDWARE.md`, save `espefuse summary --format json` to a file, and set `LASTRO_EFUSE_SUMMARY_JSON`, `LASTRO_EFUSE_BLOCK_FIELD`, and `LASTRO_EFUSE_PURPOSE_FIELD` to the exact fields for the selected block. H1 requires the block to report `readable: false` and an ECDSA key purpose, then repeats the same signed capture across reboot. The test never burns or changes eFuse state.

H1 permits the phrase `hardware-backed Station key` only after the manual eFuse-provisioned signer and physical end-to-end flow actually pass.

## Acceptance gates

- **G0 Protocol**: equivalent layouts/vectors across Rust, C, TypeScript, and Solana transaction construction.
- **G1 Physical crypto**: real RFID → ESP32-C5 → v2 envelope → P-256 → host verifies the same 220 bytes.
- **G2 Solana local**: register → bind → presence → replace → custody transfer, plus adversarial cases.
- **G3 Full local**: Station → Agent → API → wallet → local chain → independent verifier.
- **G4 Devnet**: same protocol flow against the deployed Devnet program/configuration.
- **G5 Demo stable**: three consecutive executions without manual state edits.
- **H1 eFuse**: optional hardware-backed Station key claim.
