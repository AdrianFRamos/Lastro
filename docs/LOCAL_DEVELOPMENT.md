# Full local development

Lastro has a disposable, zero-cost software environment for the complete local protocol path:

```text
virtual RFID tag
  -> hardware simulator
  -> exact LSTR v1
  -> real Rust Agent
  -> real Rust API + PostgreSQL
  -> test Wallet A/B/C
  -> real Lastro Solana program
  -> solana-test-validator
  -> independent verifier
```

The local environment is infrastructure, not a second Lastro implementation. Production code does not import it and no success path bypasses Station evidence, wallet authorization, Solana execution, or verifier checks.

## Why the validator runs on the host

Solana documents `solana-test-validator` as the local development cluster and `http://localhost:8899` as the normal local RPC endpoint. Lastro uses that real validator directly instead of a fake RPC or an unofficial validator image.

The API normally runs in Docker for the local demo. Docker documents `host.docker.internal:host-gateway` as the host mapping for a container that must reach a host-local service. The local Compose override adds only that mapping to the API container. Browser and system tests continue to use `http://127.0.0.1:8899`.

Docker Compose also gives the invoking shell high precedence during variable interpolation. The local orchestrator therefore removes inherited `LASTRO_*`, `VITE_*`, and `COMPOSE_*` overrides before starting Compose, then injects only the generated local configuration. An exported Devnet/Mainnet RPC or Program ID cannot silently leak into a local run.

Primary references:

- Solana local program development: https://solana.com/docs/programs/rust
- Solana RPC endpoint reference: https://solana.com/docs/rpc/http
- Solana program deployment: https://solana.com/docs/programs/deploying
- Anchor local development: https://www.anchor-lang.com/docs/quickstart/local
- Agave changelog for the loopback-by-default test-validator change: https://github.com/anza-xyz/agave/blob/master/CHANGELOG.md
- Docker Compose host gateway: https://docs.docker.com/compose/how-tos/networking/
- Docker Compose environment precedence: https://docs.docker.com/compose/how-tos/environment-variables/envvars-precedence/

## Isolation boundary

All disposable state is under:

```text
.lastro-local/
```

It contains only development material:

```text
.lastro-local/
├── local.env
├── secrets.json
├── program-keypair.json
├── wallets/
│   ├── wallet-a-keypair.json
│   ├── wallet-b-keypair.json
│   └── wallet-c-keypair.json
├── ledger/
├── logs/
└── artifacts/
```

`.lastro-local/` is gitignored. The program keypair and Wallet A/B/C are test-only identities. Never fund them on Mainnet and never reuse them for a production Station, deployment authority, or custodian.

The local build temporarily injects the generated local Program ID into the Anchor source, runs `anchor keys sync` + `anchor build --ignore-keys`, and restores the tracked source and `Anchor.toml` byte-for-byte afterward. The generated local Program ID therefore never becomes the repository's production identity.

The normal deployment bootstrap remains separate:

```bash
make bootstrap-program-id
```

Do not run that production/deployment bootstrap merely to start the disposable local environment.

## Required toolchain

The local environment deliberately uses the same pinned toolchain as the repository:

```text
Python 3.13.x
Rust 1.98.1
Anchor CLI 1.2.0
Solana CLI / solana-test-validator 4.1.2
Node 24.21.0
npm 11.19.0
Docker + Docker Compose
```

For the full browser test, install repository dependencies once:

```bash
python3 -m pip install -r requirements-dev.txt
npm ci
npm --workspace @lastro/web run playwright:install
```

The Anchor/Solana versions must match `chain/Anchor.toml`. The repository CI installs the pinned Anchor release through AVM and then lets Anchor install the pinned Solana toolchain. Use the same versions locally.

Check the local software prerequisites with:

```bash
make local-demo-doctor
```

This local doctor intentionally does not require ESP-IDF because the software-only local path uses the hardware simulator. Physical Station validation remains a separate gate.

## Initialize once

Run:

```bash
make local-demo-init
```

This creates:

- one disposable local Program ID/keypair;
- Wallet A, Wallet B, and Wallet C test keypairs;
- a random local Agent token;
- `.lastro-local/local.env` with the local RPC/application configuration.

The command prints only public addresses and paths; it does not print the private key material.

## Start the interactive local stack

Run:

```bash
make local-demo-up
```

The command:

1. validates the pinned software toolchain;
2. starts the real `solana-test-validator` with its ledger under `.lastro-local/ledger`;
3. compiles the real Lastro Anchor program with the disposable local Program ID;
4. airdrops local-only SOL to Wallet A/B/C;
5. deploys the real Lastro program to the local validator;
6. initializes or validates the immutable `ProtocolConfig` using the repository's frozen Station test identity;
7. starts PostgreSQL, API, web, hardware simulator, and the simulator-owned Agent bridge through Docker Compose.

Open:

```text
Lastro web              http://127.0.0.1:8088
Hardware Simulator      http://127.0.0.1:8090
Solana RPC              http://127.0.0.1:8899
Lastro API              http://127.0.0.1:8080
```

The validator binds `0.0.0.0` only because the API container must reach the host RPC through Docker's host gateway. Agave changed the test-validator default to loopback for security; treat `make local-demo-up` as a development-only process on a trusted machine/network. Do not expose port 8899 through a public firewall/router.

## Wallet behavior

There are two distinct wallet gates.

### Automated zero-cost local gate

`make local-demo-test` loads the generated Wallet A/B/C keypairs into the existing deterministic Wallet Standard Playwright harness. The browser signs the real serialized Lastro transactions and submits them to `solana-test-validator`.

No browser extension and no real SOL are required.

### Real wallet-extension gate

A normal interactive browser session at `http://127.0.0.1:8088` continues to use the production Wallet Standard discovery path. Lastro does not add a hidden local-wallet mode to production frontend source.

Test Phantom or another installed wallet separately on a network that the wallet officially supports (for example Devnet). A passing deterministic local Wallet Standard test proves Lastro's wallet contract; it does not claim extension-specific interoperability.

## Run the complete local software test

Run:

```bash
make local-demo-test
```

This command is intentionally fresh and hermetic. It preserves the disposable identities but resets runtime data, then runs:

```text
G3 full-local system contracts
  Station harness -> Agent -> API -> Wallet -> local Solana -> verifier

G5 three-run stability
  three complete consecutive protocol runs

Playwright full-stack browser E2E
  deterministic Wallet Standard A/B/C actors against the same local API/program
```

The G3 path includes ORIGIN, A->B, stale-A rejection, REIDENTIFY, B->C, projection/canonical-state comparison, recovery, tamper checks, and evidence verification.

To watch the browser run:

```bash
make local-demo-test-headed
```

The test command stores useful local artifacts/logs under `.lastro-local/` and tears down the temporary test services when it finishes.

## Status, stop, and reset

Show public identities and service status:

```bash
make local-demo-status
```

Stop services while preserving PostgreSQL/Agent volumes and the local validator ledger:

```bash
make local-demo-down
```

Reset runtime state while preserving the local Program ID and Wallet A/B/C:

```bash
make local-demo-reset
```

Remove the disposable identities too:

```bash
python3 scripts/local_dev.py reset --identities
```

Then run `make local-demo-init` again.

## Switching environments later

The local stack does not introduce a local-only protocol branch. Environment changes remain configuration changes:

```text
Local:
  API -> solana-test-validator
  browser tests -> generated Wallet A/B/C
  Agent -> hardware simulator PTY

Devnet:
  API -> Devnet RPC
  browser -> installed Wallet Standard wallet
  Agent -> simulator or physical Station

Production:
  API -> production Solana RPC
  browser -> production custody wallet
  Agent -> physical Station
```

The following stay identical across those environments:

- `StationEvent[276]` bytes;
- RFID/Station/event hash domains;
- P-256 Station signatures;
- LSTR transport contract;
- API evidence admission;
- Wallet-required transaction bytes;
- Lastro Solana instructions and PDAs;
- custody/identity state transitions;
- independent verification rules.

There is deliberately no `if local: accept_fake_success` path.

## Physical hardware is still a separate gate

A successful local test is strong software evidence, but it does not prove:

- a real FDX-B reader protocol;
- antenna/read-range behavior;
- ESP32-C5 native USB behavior on the target board;
- physical power-loss behavior;
- eFuse provisioning/read protection;
- installed wallet-extension interoperability.

Those remain the explicit G1/H1/external gates in `docs/HARDWARE.md` and `docs/TESTING.md`.
