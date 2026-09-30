# Current architectural decisions

1. **The hackathon proves the primitive**: physical RFID, P-256 evidence, custody, canonical state, and independent verification. Compliance, GNSS, and finance stay outside the critical path.
2. **The v2 domain envelope is 220 bytes** and contains no GNSS, livestock-management data or free text. `observed_at` is host time, not attested by the Station.
3. **state_version and the event hash chain belong to the AssetID**, not to the Station.
4. **Lost tags are recovered through RFID history, not a side identifier**: a replaced tag stays RETIRED and still resolves to its AssetID.
5. **RfidBinding is canonical on-chain and never closed**. An RFID can identify only one AssetID, ever; a retired binding remains RETIRED.
6. **Secp256r1 validates the raw 220 envelope bytes** referenced by the Lastro instruction. `event_hash` is a domain-separated hash of those bytes and never replaces the precompile message.
7. **Secp offsets are derived from the serialized instruction**, never hard-coded from assumptions about the discriminator.
8. **Station does not persist a complex journal in the hackathon**. Agent/SQLite is the first durable boundary.
9. **PostgreSQL is a projection**. `AssetState`/`RfidBinding`/`EventAnchor` decide canonical state.
10. **eFuse is hardening**. The product only uses the claim `hardware-backed` if H1 passes with real evidence.
11. **Modular Anchor + LiteSVM** follows the current Anchor 1.2.0 template/recommendation.
12. **Solana Kit + Wallet Standard** replaces `wallet-adapter` in new code.
13. **Web uses Vue + Vitest/VTU + Playwright** to separate unit/component tests from E2E.
14. **No premature state-compression strategy**. On-chain scaling will be redesigned only after pilot volume/cost/audit measurements.
15. **Canonical RFID is the logical 64-bit FDX-B identification value serialized by Lastro as 8 big-endian bytes**. The reader may deliver text/decimal/hex or bytes in its own framing; the adapter validates and decodes that framing according to the actual hardware manual, then serializes the logical integer into canonical format. Big-endian is a Lastro convention, not a statement about RF transmission order.
16. **LiteSVM 0.15.2 is pinned for on-chain tests** because it uses the Agave 4.1.x runtime line, avoiding deliberate mixing with runtime 4.2.x while Anchor 1.2.0 recommends Solana 4.1.2.
17. **The web verifier uses `@noble/curves` 2.4.0 for P-256**. The protocol uses a 33-byte compressed SEC1 key and compact `r||s` signature; this library accepts those formats directly and applies SHA-256/low-S semantics for P-256. The code keeps an explicit low-S check so the protocol rule remains visible.
18. **Deployment configuration fails closed**. API, Agent, and Web do not invent a missing endpoint, program ID, deployment ID, Station key, or serial port. `.env.example` documents names, but deployment-specific values remain mandatory.
19. **Serial uses fixed payloads and manual codecs**: COMMAND=202 bytes, EVENT_READY=341, ACK=48, ERROR=20. C and Rust never serialize structs/serde directly onto the wire; shared binary fixtures freeze offsets and endianness.
20. **RFID lookup and physical observation are different capabilities**. `GET /api/v2/assets/by-rfid/:hash` resolves a known hash through the on-chain `RfidBinding` (ACTIVE or RETIRED). A physical presence proof is its own Station operation (`OBSERVE_PRESENCE`).
21. **Capture creation requires a wallet-signed, single-use challenge** from the wallet the chain will require; the Solana program remains the final authority.
22. **The root Cargo workspace and Anchor are separate workspaces**. `chain/Cargo.toml` is the program workspace; the root Rust workspace contains only protocol, Agent, and API. This prevents one package from belonging to two workspaces simultaneously.
23. **`VITE_*` is public build configuration**. Vite loads the root `.env` through `envDir`; static images receive API/RPC/chain/program ID as Docker build args. Tokens, database credentials, and private keys never enter the web bundle.
24. **v2 is the only protocol**. The v1 program (`ORIGIN/TRANSFER/REIDENTIFY`, 276-byte StationEvent) was removed after every guarantee was ported; see `docs/MIGRACAO_V1_PARA_V2.md`.
