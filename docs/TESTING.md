# Testing strategy

## Main rule

A test is evidence only when it actually ran against the dependency boundary it claims to cover. Lastro uses two kinds of tests:

1. **normal executable tests**, which run whenever their toolchain/dependencies are present;
2. **explicit environment-gated tests**, whose bodies are fully implemented but skip unless real external infrastructure or hardware has been declared.

Never replace an assertion with a log, silently weaken a protocol invariant, or report an environment-gated test as passed when it was skipped.

## Pyramid applied to Lastro

### 1. Pure protocol

`crates/lastro-protocol/tests/`

Freezes the 276-byte layout, endianness, hash domains, actions, sequence/revision, evidence package, P-256 low-S behavior, and cross-language vectors.

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

Covers PostgreSQL constraints, registration/recovery, capture lifecycle, cryptographic evidence admission, append-only/monotonic events, transaction preparation, Secp256r1 descriptor construction, confirmed transaction registration, finalized RPC confirmation, reload recovery metadata, projection updates, concurrency, and `EvidencePackage` export.

```bash
LASTRO_DATABASE_URL=postgres://lastro:lastro@127.0.0.1:5432/lastro \
  cargo test -p lastro-api
```

### 5. Solana program

`chain/programs/lastro/tests/`

LiteSVM contracts exercise the real Anchor instructions and standard precompiles for initialization, ORIGIN, TRANSFER, REIDENTIFY, stale custodian, replay, sequence/predecessor/revision errors, RFID binding history/reuse, account/PDA constraints, exact Secp256r1 descriptor offsets, wrong key/message/index/length/order, and transaction size.

```bash
make bootstrap-program-id   # once for a real local identity
cd chain
anchor build --ignore-keys
cargo test --locked --workspace
```

These tests require the pinned Rust/Anchor/Solana toolchain and built SBF artifact. Their presence in source is not a substitute for running them.

### 6. Browser unit/component tests

`apps/web/tests/`

Covers public deployment configuration, protocol decoding/hashes/P-256, API DTO validation, Wallet Standard authority/capability handling, exact transaction preflight, operator state machine, broadcast/SUBMITTED reload restoration without duplicate wallet signatures, evidence verification, finalized txSignature-to-envelope binding, incomplete transaction-reference rejection/non-success semantics, and direct canonical RPC comparison.

```bash
npm ci
npm --workspace @lastro/web run typecheck
npm --workspace @lastro/web run test
npm --workspace @lastro/web run build
```

Use Node 24.21.0 and npm 11.19.0 as pinned by `.nvmrc`/`package.json`. The committed `package-lock.json` is required; generate resolver-owned locks with `scripts/bootstrap_dependencies.sh` before running release CI on a fresh checkout.

### 7. Full browser E2E

`apps/web/e2e/`

The Playwright bodies are implemented and gated by `LASTRO_E2E_SYSTEM=1`. The fixture starts the real Rust Agent against a PTY-backed Station harness, registers three deterministic Wallet Standard test wallets, keeps their private keys in the Node test process, signs the production transaction bytes, and talks to the real API/PostgreSQL/local Solana stack. It also supports Agent-response fault injection without bypassing the API or chain.

Required environment:

```text
LASTRO_E2E_SYSTEM=1
LASTRO_SYSTEM_API_URL
LASTRO_SYSTEM_SOLANA_RPC_URL
LASTRO_PROGRAM_ID
LASTRO_DEPLOYMENT_ID_HEX
LASTRO_SYSTEM_AGENT_BIN
LASTRO_AGENT_TOKEN
LASTRO_SYSTEM_STATION_PRIVATE_SCALAR_HEX
LASTRO_SYSTEM_WALLET_A_KEYPAIR
LASTRO_SYSTEM_WALLET_B_KEYPAIR
LASTRO_SYSTEM_WALLET_C_KEYPAIR
```

Optional browser settings:

```text
LASTRO_E2E_SOLANA_CHAIN=solana:localnet
LASTRO_E2E_EXTERNAL=1        # use an already running web build
LASTRO_E2E_BASE_URL=http://127.0.0.1:4173
```

Run:

```bash
LASTRO_E2E_SYSTEM=1 npm --workspace @lastro/web run test:e2e
```

The E2E suite covers the complete four-event flow, stale Wallet A rejection, visual recovery, RFID retirement/activation, lost API response + Agent retry, browser reload reconstruction, wallet-signature rejection, independent `VALID`, one-byte `INVALID`, and verification from a local file while the evidence API is unavailable.

These deterministic Wallet Standard actors are test wallets, not a browser extension. Before the final demo, manually execute the same authorization flow with the chosen real extension and confirm connect, account selection, signature approval/rejection, broadcast, reload-after-broadcast recovery, and stale-custodian rejection. CI must not claim that external interoperability gate.

### 8. Full local system gate (G3)

`tests/system/`

The pure harness contracts in `test_support.py` and `test_e2e_controller.py` run in normal CI without external services. They validate the Python serial/P-256/PDA/message helpers and the E2E controller's observation/fault-injection behavior.

The G3 integration harness uses real wire contracts: PTY Station framing/CRC32C, real P-256 event signatures, the actual Agent executable/SQLite outbox/API, real Ed25519 wallet signatures, Solana RPC submission/finality, independent PDA/account decoding, exact finalized txSignature-to-envelope verification, and independent evidence verification.

With the same `LASTRO_SYSTEM_*` variables above:

```bash
LASTRO_SYSTEM_TEST=1 python3 -m pytest -q tests/system/test_full_local.py tests/system/test_recovery.py tests/system/test_tamper.py
```

The preferred developer entrypoint provisions a disposable Program ID, Wallet A/B/C, a real `solana-test-validator`, deploys the real Lastro program, initializes `ProtocolConfig`, starts the required application services, and runs G3 with a fresh runtime:

```bash
make local-demo-init   # once per disposable identity set
make local-demo-test
```

This orchestration does not weaken the G3 boundary; it supplies the same environment that the existing harness already requires. See `docs/LOCAL_DEVELOPMENT.md`.

### 9. Devnet gate (G4)

```bash
LASTRO_DEVNET_TEST=1 python3 -m pytest -q tests/system/test_devnet.py
```

Run only with the declared deployed program, registered Station configuration, funded test wallets, and Devnet RPC. Preserve the emitted transaction/evidence artifacts.

The manual GitHub Actions workflow `.github/workflows/devnet.yml` runs the same G4 contracts without committing private material. It fails before network validation unless the repository contains exactly one real `declare_id!` and that public identity matches `LASTRO_DEVNET_PROGRAM_ID`. Configure the protected `devnet` GitHub Environment with these variables:

```text
LASTRO_DEVNET_PROGRAM_ID
LASTRO_DEVNET_DEPLOYMENT_ID_HEX
LASTRO_DEVNET_STATION_PUBKEY_HEX
```

Configure these GitHub Environment secrets:

```text
LASTRO_DEVNET_RPC_URL
LASTRO_AGENT_TOKEN
LASTRO_DEVNET_STATION_PRIVATE_SCALAR_HEX
LASTRO_DEVNET_WALLET_A_KEYPAIR_JSON
LASTRO_DEVNET_WALLET_B_KEYPAIR_JSON
LASTRO_DEVNET_WALLET_C_KEYPAIR_JSON
```

Each wallet secret is the normal 64-byte Solana keypair JSON array. The workflow writes temporary copies only under `RUNNER_TEMP`, validates that the deployed program and immutable `ProtocolConfig` match the configured deployment/Station, executes the complete Devnet flow plus stale-custodian rejection, and uploads the resulting public transaction/evidence references. It does not deploy a new program or provision Station/eFuse keys.

### 10. Demo stability gate (G5)

```bash
LASTRO_DEMO_STABILITY_TEST=1 python3 -m pytest -q tests/system/test_demo_stability.py
```

The full-stack GitHub Actions fixture runs this three-pass stability contract against the deterministic local software stack. The final physical demo must still repeat the same gate with the real Station/reader and real wallet-extension interoperability; CI does not prove those external boundaries.

### 11. Physical hardware and eFuse

`firmware/station/pytest/`

Run only after the reader gate in `docs/HARDWARE.md` is satisfied. The tests communicate with the real Station through the frozen Agent serial protocol; they do not synthesize a reader observation. Stop the Rust Agent first so the hardware test owns the serial port.

Install the hardware-test dependency and perform the machine-visible preflight:

```bash
python3 -m pip install -r requirements-dev.txt
LASTRO_AGENT_SERIAL_PORT=/dev/... ./scripts/hardware_preflight.sh
```

For G1, configure `LASTRO_DEPLOYMENT_ID_HEX`, `LASTRO_HARDWARE_CUSTODIAN_HEX`, and the independently known 8-byte `LASTRO_HARDWARE_TAG_A_RFID_HEX`, then run:

```bash
LASTRO_HARDWARE_TEST=1 python3 -m pytest -s -q firmware/station/pytest/test_hardware.py
```

The two-tag stability test additionally requires interactive physical A/B swaps, `LASTRO_HARDWARE_TAG_B_RFID_HEX`, and retained raw reader-frame files. The reboot test requires `LASTRO_HARDWARE_REBOOT_COMMAND`; the command is executed directly without a shell.

For H1, provision the key manually as documented in `docs/HARDWARE.md`, save `espefuse summary --format json` to a file, and set `LASTRO_EFUSE_SUMMARY_JSON`, `LASTRO_EFUSE_BLOCK_FIELD`, and `LASTRO_EFUSE_PURPOSE_FIELD` to the exact fields for the selected block. H1 requires the block to report `readable: false` and an ECDSA key purpose, then repeats the same signed StationEvent across reboot. The test never burns or changes eFuse state.

H1 permits the phrase `hardware-backed Station key` only after the manual eFuse-provisioned signer and physical end-to-end flow actually pass.

## Acceptance gates

- **G0 Protocol**: equivalent layouts/vectors across Rust, C, TypeScript, and Solana transaction construction.
- **G1 Physical crypto**: real RFID → ESP32-C5 → `StationEvent` → P-256 → host verifies the same 276 bytes.
- **G2 Solana local**: ORIGIN/A→B/REIDENTIFY/B→C plus adversarial cases.
- **G3 Full local**: Station → Agent → API → wallet → local chain → independent verifier.
- **G4 Devnet**: same protocol flow against the deployed Devnet program/configuration.
- **G5 Demo stable**: three consecutive executions without manual state edits.
- **H1 eFuse**: optional hardware-backed Station key claim.
