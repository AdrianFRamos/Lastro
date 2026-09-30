# Technical sources and frozen versions

Reviewed on September 19, 2026. The project should prefer official documentation and primary artifacts; new versions do not enter during the hackathon without a concrete reason and regression testing.

## Toolchain

- Rust 1.98.1 — official September 3, 2026 release: https://blog.rust-lang.org/releases/latest/
- Anchor 1.2.0 — release notes: https://www.anchor-lang.com/docs/updates/release-notes/1-2-0
- Solana 4.1.2 — version recommended by Anchor 1.2.0 in the same release notes.
- Anchor modular structure: https://www.anchor-lang.com/docs/quickstart/local
- Anchor testing / LiteSVM: https://www.anchor-lang.com/docs/testing/litesvm
- LiteSVM 0.15.2 — uses the Agave 4.1.x runtime line, aligned with the Solana 4.1.2 toolchain pinned by Anchor 1.2.0: https://docs.rs/crate/litesvm/0.15.2
- Cargo workspaces: https://doc.rust-lang.org/cargo/reference/workspaces.html

## Local development infrastructure

- Solana local program development with `solana-test-validator` and localhost RPC: https://solana.com/docs/programs/rust
- Solana program deployment / localhost airdrop: https://solana.com/docs/programs/deploying
- Solana local RPC examples on `localhost:8899`: https://solana.com/docs/intro/quick-start/writing-to-network
- Anchor local development and externally managed local validator: https://www.anchor-lang.com/docs/quickstart/local
- Anchor localnet / `skip_local_validator` configuration: https://www.anchor-lang.com/docs/references/anchor-toml
- Agave changelog — `solana-test-validator` loopback-by-default security change and explicit `--bind-address 0.0.0.0`: https://github.com/anza-xyz/agave/blob/master/CHANGELOG.md
- Docker Compose host access with `host.docker.internal:host-gateway`: https://docs.docker.com/compose/how-tos/networking/
- Docker Compose environment-variable precedence: https://docs.docker.com/compose/how-tos/environment-variables/envvars-precedence/

## Solana client/web

- Solana frontend: https://solana.com/docs/frontend
- Solana Kit: https://www.npmjs.com/package/@solana/kit — 8.3.0
- Kit RPC plugin: https://www.npmjs.com/package/@solana/kit-plugin-rpc — 0.19.0
- Kit Wallet plugin / Wallet Standard (`walletSigner`): https://www.npmjs.com/package/@solana/kit-plugin-wallet — 0.20.0
- Secp256r1 precompile: https://solana.com/docs/core/programs/precompiles

## Web

- Node.js 24.21.0 LTS: https://nodejs.org/en/download
- Vue 3.5.43: https://www.npmjs.com/package/vue
- Vue Router 5.3.1: https://www.npmjs.com/package/vue-router
- Vite 8.2.2: https://www.npmjs.com/package/vite
- @vitejs/plugin-vue 6.0.9: https://www.npmjs.com/package/@vitejs/plugin-vue
- TypeScript 7.0.2: https://www.npmjs.com/package/typescript
- Vitest 5.0.1: https://www.npmjs.com/package/vitest
- Vue Test Utils 2.5.1: https://www.npmjs.com/package/@vue/test-utils
- Playwright Test 1.63.0: https://www.npmjs.com/package/@playwright/test
- @noble/curves 2.4.0 — P-256 verifier used by the browser: https://www.npmjs.com/package/@noble/curves
- @noble/curves P-256/compact signature behavior: https://github.com/paulmillr/noble-curves
- vue-tsc 3.3.11: https://www.npmjs.com/package/vue-tsc
- Prettier 3.9.8: https://www.npmjs.com/package/prettier
- @types/node 24.13.6 (matching the Node 24 line): https://www.npmjs.com/package/@types/node

## Backend and database

- Axum: https://docs.rs/axum/latest/axum/
- Tokio: https://docs.rs/tokio/latest/tokio/
- SQLx 0.9.0: https://docs.rs/sqlx/latest/sqlx/
- PostgreSQL 18.6 (stable patch on August 13, 2026): https://www.postgresql.org/docs/release/

## Hardware

- ESP32-C5 ECDSA peripheral: https://docs.espressif.com/projects/esp-idf/en/release-v6.1/esp32c5/api-reference/peripherals/ecdsa.html
- ESP-IDF 6.1 ESP ECDSA PSA opaque driver (key lifetime, opaque import, sign-hash, public-key export): https://github.com/espressif/esp-idf/blob/v6.1/components/mbedtls/port/psa_driver/include/psa_crypto_driver_esp_ecdsa.h
- ESP32-C5 eFuse block and ECDSA key-purpose definitions: https://github.com/espressif/esp-idf/blob/v6.1/components/efuse/esp32c5/include/esp_efuse_chip.h
- espefuse JSON summary/read-protection semantics: https://docs.espressif.com/projects/esptool/en/latest/esp32/espefuse/summary-cmd.html
- ESP-IDF Unity component tests: https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-guides/unit-tests.html
- ISO 11784:2024 — animal identification code structure: https://www.iso.org/standard/83944.html
- Microchip ATA5577C/FDX-B documentation — primary hardware reference for the 128-bit FDX-B telegram, 64-bit Identification Code, and LSB-first RF transmission behavior: https://ww1.microchip.com/downloads/aemDocuments/documents/WSG/ProductDocuments/DataSheets/ATA5577C-Read-Write-LF-RFID-IDIC-100-to-150-kHz-Data-Sheet-DS70005357.pdf

## Product context

- PNIB — official Brazilian government animal-identification program reference.
- SISBOV — official Brazilian cattle traceability system reference.
- EUDR: https://eur-lex.europa.eu/eli/reg/2023/1115
- Colosseum Hackathon: https://colosseum.com/hackathon

## Build and configuration

- Vite environment variables / `VITE_*` build-time exposure: https://vite.dev/guide/env-and-mode
- Anchor `Anchor.toml`, workspace/toolchain/package manager: https://www.anchor-lang.com/docs/references/anchor-toml
- Anchor account/PDA constraints: https://www.anchor-lang.com/docs/references/account-constraints
- Solana transaction limits: https://solana.com/docs/core/transactions
- Node.js release/LTS status: https://nodejs.org/en/about/previous-releases

## Python validation tooling

- pytest 9.1.1: https://pypi.org/project/pytest/
- PyYAML 6.0.3: https://pypi.org/project/PyYAML/
- jsonschema 4.26.0: https://pypi.org/project/jsonschema/
- cryptography 50.0.1: https://pypi.org/project/cryptography/
- pyserial 3.5: https://pypi.org/project/pyserial/
