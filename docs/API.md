# API — contract and boundaries

The API coordinates capture, persists immutable evidence, prepares instructions, and maintains a read projection. It **does not** decide canonical custody or identity. It never receives the private key of a browser wallet; since v2.1 it does hold the fee sponsor key and the sealed keys of managed organization wallets (see `docs/SECURITY.md`, "Keys the platform holds"). There is a single protocol: Lastro v2.

Machine-readable contract: `schemas/openapi.yaml`.

## Asset registration

`POST /api/v2/assets/transaction-data` builds the `register_asset` transaction for the deployment authority's wallet. Nothing is stored: the asset exists once that transaction is finalized. `GET /api/v2/assets/{assetId}` reads the canonical `AssetState`; `POST /api/v2/assets/{assetId}/sync` mirrors it into the local projection.

## Capture flow

Three physical operations exist: `BIND_IDENTIFIER` (first RFID of an asset), `REPLACE_IDENTIFIER` (lost/damaged tag) and `OBSERVE_PRESENCE` (the tagged animal was physically read).

1. Web requests `POST /api/captures/authorization-challenge` for the exact action and asset. The API reads the canonical `AssetState` and refuses transitions the chain would reject (BIND on a tagged asset, REPLACE/OBSERVE on an untagged one, any capture on a terminal asset).
2. The required wallet signs the deployment-bound, single-use challenge; Web supplies that signature to `POST /api/captures`, and the API creates an immutable capture context that expires in 5 minutes.
3. Agent polls authenticated `GET /api/agent/commands` and forwards the 202-byte COMMAND to the Station.
4. Station reads the RFID itself, applies the capture rule (BIND: no previous tag; REPLACE: observed tag differs from the expected one; OBSERVE: observed tag equals the expected one), builds the 220-byte domain envelope and signs it with its P-256 key.
5. Agent persists the evidence locally before `POST /api/agent/evidence`.
6. API rebuilds the envelope from its own capture context plus the observed RFID, requires byte equality with the signed envelope, and verifies the Station signature. Accepted evidence is append-only.
7. Web requests `GET /api/v2/events/{eventHash}/transaction-data`, validates program/order/signing wallet, signs, and broadcasts.
8. `submit` stores the transaction signature only after confirmed RPC proves the exact Lastro transaction. Projection state does not advance.
9. `confirm` accepts only that stored signature, requires the same exact transaction at finalized commitment, checks the on-chain `EventAnchor` (and, for identity events, that `AssetState.current_rfid_hash` is the observed tag) and only then marks the event `FINALIZED`.

## Capture authority boundary

`POST /api/captures` requires an Ed25519 signature over a two-minute, single-use challenge bound to the action, asset, state version, deployment and program. All three captures are signed by the asset's **current on-chain custodian** (the program also accepts the deployment authority for presence proofs, used only by capture-less Agent observations). The API consumes the challenge transactionally before reserving Station work. On-chain authority remains a separate signer check on the actual transaction.

## RFID lookup

`GET /api/v2/assets/by-rfid/{rfidHash}` resolves a canonical RFID hash through the on-chain `RfidBinding`. Tags are never reused: a `RETIRED` binding still resolves to the asset it once identified, which is how a replaced tag found in the field leads back to the animal.

## Evidence package

`GET /api/v2/assets/{assetId}/evidence-package` returns `lastro.evidence-package.v2`: the canonical asset state, every finalized Station event (envelope bytes, Station key and signature, observed RFID, transaction signature) and the accepted custody transfers with their transaction signatures. An independent verifier needs only this file and a Solana RPC.

## Idempotency

- Identical evidence may be resent and receives an idempotent result (`200` instead of `201`).
- Repeating `POST /api/captures` for the same action, state version and signer returns the existing open capture. A different transition for the same asset replaces the open capture only while no transaction signature was registered for it; otherwise `409`.
- Evidence whose bytes diverge from the capture context returns `409 Conflict`.
- Repeating `submit` with the same RPC-verified transaction signature is idempotent; a different signature cannot replace it.
- Repeating `confirm` for the same finalized transaction returns the same terminal state without duplicating the event.

## Resource and input bounds

The application rejects JSON request bodies larger than **1024 bytes** on every protocol and public route before route processing. Only the operator-authenticated registry and processing routes accept up to **64 KiB**, the budget for lot admission with 512 asset references (`MAX_OPERATIONS_JSON_BODY_BYTES`). The largest protocol request is `AgentEvidenceRequest`: the 220-byte envelope is exactly 296 Base64 characters, plus one UUID and the fixed 8/33/64-byte evidence fields.

Fixed identifiers are decoded before cryptographic, database or RPC work: asset/event/RFID hashes are 64 lowercase hexadecimal characters, Station public keys 66, compact Station signatures 128, observed RFIDs 16, capture identifiers are UUIDs, and Solana transaction signatures are 64..88 Base58 characters that must decode to 64 bytes.

Backend Solana RPC calls have a 10-second request timeout. The independent browser verifier uses a 10-second per-request deadline and a 120-second total RPC budget (8 requests in flight), returning `NOT_CHECKED` on network timeout.

Timeline and EvidencePackage export reject histories longer than 1024 events instead of silently truncating them (only finalized events count toward the package). The browser verifier checks them with bounded parallel RPC within a 120-second budget.

The database serializes and limits anonymous challenge issuance to 8 per signer or asset and 240 globally per minute, retaining only a bounded period of expired challenges. External ingress should additionally enforce per-client limits for a public deployment.

## Priority fees

`LASTRO_PRIORITY_FEE_MICRO_LAMPORTS` (1..=1,000,000) makes every `transaction-data` response
append `set_compute_unit_limit(200000)` and `set_compute_unit_price(value)` after the protocol
instructions. Unset keeps the exact two-instruction envelope. The fee is at most 0.0002 SOL per
transaction and is paid by the transaction's fee payer.

## Fee sponsorship (v2.1, S1)

With `LASTRO_SPONSOR_KEYPAIR_PATH` set, every `transaction-data` response names the platform
sponsor as `feePayer` (and as the instruction's `payer`) while `requiredSigner` stays the wallet
that authorizes. The wallet signs only its own slot and posts the transaction to
`POST /api/v2/sponsor/submit` with the `purpose` it was prepared for. The API:

1. re-derives that exact transaction with the same code as the `transaction-data` routes and
   requires byte equality, ComputeBudget values included;
2. requires the sponsor as unsigned fee payer and a valid signature from every other signer;
3. refuses if the sponsor is the requiredSigner or the deployment authority (invariant I7);
4. bounds the cost (fees + created rent) by `LASTRO_SPONSOR_MAX_LAMPORTS_PER_TX` and the rolling
   24 h budgets `LASTRO_SPONSOR_DAILY_LAMPORTS_PER_SIGNER` / `..._TOTAL`, recorded in
   `v2_sponsor_ledger` under an advisory lock;
5. pays only when the requiredSigner is the deployment authority, an ACTIVE party or an
   organization wallet, so throwaway keys cannot spend the budget;
6. co-signs, broadcasts and returns the signature and the fully signed bytes. The reservation
   is released only when Solana definitely rejects the transaction in preflight; after a
   transport failure it is kept, because the transaction may still land.

`submit` and `confirm` accept the sponsored form and the self-paid form of the same transaction,
so a wallet can still pay for itself when sponsorship is unavailable. The browser accepts a
`feePayer` other than the signer only when it equals the build-time `VITE_LASTRO_SPONSOR`.

## Handling sessions (v2.1, S3/S4a/S8)

One custodian signature authorizes the configured Station to record presence proofs for at
most 12 h and 2 000 events; the proofs are anchored as Merkle batches of up to 1 024 leaves.

1. `POST /api/v2/sessions` `{custodian, maxEvents, durationSeconds}` prepares `open_session`
   (payer = sponsor when configured). It needs the operator token, for a custodian that is an
   ACTIVE party or an organization wallet, or a user of the custodian's own organization
   (`ORG_ADMIN`/`OPERADOR_CURRAL`); a custodian holds at most 16 live sessions. Preparing a
   batch also accepts the Agent token; revoking needs the operator or the organization. The wallet signs; `POST /api/v2/sessions/{id}/confirm`
   admits the finalized `SessionGrant` and **freezes the manifest**: every active, tagged asset
   of the custodian at its projected `(state_version, last_event_hash)`.
2. The Agent (agent token) reads `GET .../manifest` while it has signal, then works offline.
   Each presence proof is an ordinary OBSERVATION_RECORDED capture whose capture id is
   `SHA-256("LASTRO_V2_SESSION_CAPTURE\0" || session_id || asset_id)[..16]`, so existing
   Station firmware signs it unchanged and each asset has at most one proof per session.
3. `POST .../observations` admits one proof (append-only, idempotent per asset): the envelope
   must be the exact session capture of a manifest asset, observe its tag, be signed by the
   session's Station, fall inside the session window (or before a revocation) and still be
   anchorable (at most 7 days old).
4. `POST .../batches` prepares `anchor_observation_batch` for the oldest pending proofs that fit
   the session capacity (repeat returns the same prepared batch). `POST
   .../batches/{index}/confirm` admits the finalized `BatchAnchor` after comparing root, count,
   window, custodian and Station.
5. `POST .../revoke` prepares `revoke_session`; `POST .../sync` mirrors the chain.

The program does **not** verify each leaf's Station signature (the trade-off of batching). The
API verifies every leaf on admission, and the EvidencePackage carries `batchedObservations`
(envelope, signature, Merkle path, batch tx) so the independent verifier re-checks each one
against the finalized `BatchAnchor`.

## Facility boundaries (v2.1, S5)

Operator token; the resulting transactions are signed by the deployment authority.

1. `POST /api/v2/facilities/{id}/boundary` `{carCode, rings}` stores the CAR code and polygon
   (WGS84 `[lon, lat]`, ring 0 outer, further rings holes, every ring closed, at most 64 rings
   and 10 000 vertices). Coordinates are fixed to degrees × 10^7. The boundary is immutable:
   repeating it returns 200, a different one 409. The response carries
   `geoCommitment = SHA-256("LASTRO_V2_GEO\0" || car_code || 0x00 || u32 ring count ||
   per ring u32 vertex count + (i32 lon_e7, i32 lat_e7)*)`, all integers little endian. A
   fixed binary encoding is used instead of canonical GeoJSON so every language recomputes the
   same bytes.
2. `POST /api/v2/facilities/{id}/transaction-data` prepares `register_facility` carrying that
   commitment (zero when no boundary is stored).
3. `POST /api/v2/stations/{id}/facility/transaction-data` `{facilityId}` prepares
   `set_station_facility`. `open_session` requires a Station installed at an ACTIVE facility
   valid for the whole window; `SessionGrant` and every `BatchAnchor` copy its `facility_id`.

The EvidencePackage lists each batched observation's facility and carries the boundaries
(`facilities`); the verifier's `FACILITY_ORIGIN` layer recomputes each commitment and compares
it with the finalized `FacilityRecord`. The point-in-polygon result for GNSS coordinates is a
**calculation**, never a Station proof; per-event coordinates wait for the GNSS payload.

## Organization accounts (v2.1, S2)

People operate without a browser wallet. Each organization owns one custodian wallet per
deployment; its Ed25519 secret is generated by the API and stored only sealed with AES-256-GCM
under `LASTRO_ORG_KEY_ENCRYPTION_KEY_HEX` (held outside the database). The ciphertext's
associated data binds organization id and wallet, so a row swap cannot be opened, and a trigger
keeps wallet and sealed key immutable.

1. `POST /api/v2/organizations` (operator token) `{name, adminEmail}` returns `orgId`, `wallet`
   (register it as the party's wallet) and the first `ORG_ADMIN`.
2. `POST /api/v2/auth/otp/request` `{email}` always answers 202, so accounts cannot be
   enumerated. A code has six digits, lives 10 minutes, dies after 5 wrong attempts and is
   limited to 5 per user per 15 minutes; only a hash peppered with the
   master key is stored. With `LASTRO_OTP_DELIVERY=log` the code is written to the service log;
   this is refused when CORS origins are configured, so it only exists in development
   (production needs an e-mail or SMS provider, not wired yet).
3. `POST /api/v2/auth/otp/verify` `{email, code}` returns a 12 h bearer token (only its hash is
   stored) and the user; `GET /api/v2/me` describes the signed-in user.
4. `POST /api/v2/organizations/{orgId}/users` (`ORG_ADMIN`) `{email, role}` adds a person with
   `ORG_ADMIN`, `OPERADOR_CURRAL`, `AUDITOR` or `LEITURA`.
5. `POST /api/v2/org/sign` `{purpose}` — the same purposes as `/api/v2/sponsor/submit`. The
   API checks the role (`OPERADOR_CURRAL`: `SESSION_OPEN`, `SESSION_BATCH`, `SESSION_REVOKE`;
   `ORG_ADMIN`: all; `AUDITOR`/`LEITURA`: none), prepares the transaction with the same code
   as the `transaction-data` routes, requires the organization wallet as its `requiredSigner`
   and no signer outside the organization and the sponsor, signs, and broadcasts (through the
   sponsor path when the platform pays). The caller never supplies bytes, so the key cannot be
   made to sign anything else. Every signature is kept in `v2_org_signatures`. Confirm through
   the purpose's own confirm route.
6. `POST /api/v2/organizations/{orgId}/export` (`ORG_ADMIN`) `{confirmWallet, code}` needs a
   fresh one-time code, returns the keypair in Solana CLI format **once** so the organization
   can leave managed custody, is audited, and ends managed signing (409 afterwards).
7. `POST /api/v2/organizations/{orgId}/users/{userId}/disable` (`ORG_ADMIN`) disables a person
   and revokes their tokens; `POST /api/v2/auth/logout` revokes the caller's token.

Login codes: earlier live codes stay valid when a new one is requested (a stranger cannot lock
the owner out), and after 10 wrong codes in 24 h nothing verifies for that user. Role or
organization denials answer 403; a missing or dead token answers 401. Every request is bounded
to 30 s and at most 64 run at once.

Browser wallets keep working unchanged; an advanced user can still register a party with their
own wallet.

## v2 custody transfer (two-phase, on-chain)

1. `POST /api/v2/custody-transfers` (operator token) records the proposal off-chain.
2. `GET /api/v2/custody-transfers/{transferId}/transaction-data?phase=propose` returns the
   `create_intent` (type CUSTODY_TRANSFER) transaction for the **current on-chain custodian**.
3. `GET /api/v2/custody-transfers/{transferId}/transaction-data?phase=accept` returns
   `accept_custody_transfer` for the **recipient party's wallet**. The API reads the open
   intent's `rent_payer` from the chain and stores it, because acceptance closes the intent and
   refunds that account.
4. `POST /api/v2/custody-transfers/{transferId}/accept` with `{ "txSignature": "..." }` marks
   the transfer ACCEPTED only when that exact recipient-signed transaction is finalized and the
   canonical AssetState custodian is the recipient. No operator token is involved: acceptance
   is proven on-chain, not asserted by the operator.

## v2 transformations

- `POST /api/v2/transformations` (operator token) takes the manifest parameters plus the input
  and output lineage leaves. The API derives the Merkle roots, counts and mass totals from the
  leaves, validates the mass balance, stores the manifest and append-only lineage edges, and
  returns per-leaf Merkle proofs (`leafIndex`, `proof`) for the facility wallet's
  `reserve_transformation_input` / `create_transformation_output` instructions.
  All inputs must be reserved while the transformation is OPEN: the first consumption moves it
  to FINALIZING and no further reservation is accepted.
- `POST /api/v2/transformations/{transformationId}/sync` mirrors the finalized on-chain
  TransformationAnchor. Moving to FINALIZED requires `{ "txSignature": "..." }` of the facility
  owner's finalized `finalize_transformation`; processing operations can then be finalized.

## CORS

`LASTRO_CORS_ALLOWED_ORIGINS` takes comma-separated bare origins (`https://app.example.com`, no path or trailing slash). When unset the API allows any origin, which is intended only for local development; the production Compose file requires the variable.

## Errors

`400` invalid input/format; `401` missing/invalid authorization; `404` resource; `409` state/immutability conflict; `429` persistence or challenge rate limit; `5xx` internal/RPC/DB failure. Responses never include tokens, credential-bearing URLs, or secret material.
