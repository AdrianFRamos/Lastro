# API — contract and boundaries

The API coordinates capture, persists immutable evidence, prepares instructions, and maintains a read projection. It **does not** decide canonical custody or identity and **never** receives a wallet private key. There is a single protocol: Lastro v2.

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
transaction and is paid by the signing wallet.

## v2 custody transfer (two-phase, on-chain)

1. `POST /api/v2/custody-transfers` (operator token) records the proposal off-chain.
2. `GET /api/v2/custody-transfers/{transferId}/transaction-data?phase=propose` returns the
   `create_intent` (type CUSTODY_TRANSFER) transaction for the **current on-chain custodian**.
3. `GET /api/v2/custody-transfers/{transferId}/transaction-data?phase=accept` returns
   `accept_custody_transfer` for the **recipient party's wallet**.
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
