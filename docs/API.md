# API — contract and boundaries

The API coordinates capture, persists immutable evidence, prepares instructions, and maintains a read projection. It **does not** decide canonical custody and **never** receives a wallet private key.

Machine-readable contract: `schemas/openapi.yaml`.

## Capture flow

1. Web requests `POST /api/captures/authorization-challenge` for the exact action, animal and optional destination or recapture ID.
2. The required custodian signs the deployment-bound, single-use challenge; Web supplies that signature to `POST /api/captures`, and the API creates an immutable/expiring context.
3. Agent polls authenticated `GET /api/agent/commands`.
4. Station receives the expected context, observes RFID, computes the new hash when applicable, and signs `StationEvent`.
5. Agent persists `LOCAL` before `POST /api/agent/evidence`.
6. API recomputes the observed RFID hash, event hash, StationID, and P-256 verification, then compares every signed field with the capture context.
7. Accepted evidence is append-only.
8. Web requests `transaction-data`, validates program/order/signing wallet, signs, and broadcasts.
9. `submit` stores the transaction signature only after confirmed RPC proves the exact Lastro envelope. PostgreSQL projection state does not advance.
10. `confirm` accepts only that stored signature, requires the same exact transaction at finalized commitment, decodes `AnimalState`/`RfidBinding`, and only then updates `animals`.

## Capture authority boundary

`POST /api/captures` requires a current-custodian Ed25519 signature over a two-minute, single-use capture challenge. For ORIGIN, the intended initial custodian signs. The challenge binds the action, animal, destination, deployment, program, expiry, and (for explicit same-action recapture) the prior capture ID. The API consumes it transactionally before reserving Station work. For TRANSFER, `nextCustodian` must differ from the current custodian; REIDENTIFY derives `from==to` from canonical state. On-chain custody authority remains a separate wallet-signature check on the actual transaction.

## RFID lookup

`GET /api/animals/by-rfid/:rfidHash` accepts an already-known canonical hash, queries the projection, and confirms that the corresponding `RfidBinding` is `ACTIVE` on-chain. This route does not imply that DemoPage has a separate physical read mode without a capture. The hackathon core demonstrates REIDENTIFY recovery through `visual_recovery_id`; a dedicated standalone read UX for recovery may be added in a pilot without changing the three domain operations.

## Idempotency

- Identical evidence may be resent and receives an idempotent result.
- Repeating `POST /api/captures` for the same action/destination normally returns the existing accepted/submitted capture. An explicitly signed `supersedeCaptureId` can request a fresh REIDENTIFY observation only before a transaction signature is registered. This is a demo recovery path: an old transaction signed elsewhere but not reported to the API can still race on-chain. Operators must reconcile canonical Solana state before relying on the replacement as final.
- The same capture/event identifier with divergent bytes returns `409 Conflict`.
- Repeating `submit` with the same RPC-verified transaction signature is idempotent; a different signature cannot replace it.
- Repeating `confirm` for the same finalized transaction returns the same terminal state without duplicating the event.

## Resource and input bounds

The application rejects JSON request bodies larger than **1024 bytes** before route processing. The largest request in the frozen hackathon protocol is `AgentEvidenceRequest`: a 276-byte StationEvent is exactly 368 Base64 characters, plus one UUID and the fixed 8/33/64-byte evidence fields; compact canonical JSON is about 720 bytes. The 1 KiB envelope leaves representation headroom without accepting Axum's generic multi-megabyte JSON buffering limit.

Fixed identifiers are validated before cryptographic or persistence work: AnimalID/event/RFID hashes are 64 lowercase hexadecimal characters, Station public keys are 66 lowercase hexadecimal characters, compact Station signatures are 128 lowercase hexadecimal characters, visual recovery identifiers are 1..64 characters, capture identifiers are UUIDs, and Solana transaction signatures are 64..88 Base58 characters and must decode canonically to 64 bytes before submission.

Backend Solana RPC calls have a 10-second request timeout. The independent browser verifier uses a 10-second per-request deadline and a 30-second total RPC budget, returning `NOT_CHECKED` on network timeout.

EvidencePackage export and browser parsing reject histories longer than 128 events. The API loads at most 129 records to detect overflow and never silently truncates a valid history. Deployments that expect longer animal lifetimes need a versioned streaming proof format before increasing the limit.

The database serializes and limits anonymous challenge issuance to 8 per signer and 240 globally per minute, retaining only a bounded period of expired challenges. Public animal registration permits at most 60 new unoriginated animals per minute and 5,000 pending ORIGIN. External ingress should additionally enforce per-client limits for a public deployment.

## Errors

`400` invalid input/format; `401` missing/invalid authorization; `404` resource; `409` state/immutability conflict; `429` persistence or challenge rate limit; `5xx` internal/RPC/DB failure. Responses never include tokens, credential-bearing URLs, or secret material.
