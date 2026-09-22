# API — contract and boundaries

The API coordinates capture, persists immutable evidence, prepares instructions, and maintains a read projection. It **does not** decide canonical custody and **never** receives a wallet private key.

Machine-readable contract: `schemas/openapi.yaml`.

## Capture flow

1. Web calls `POST /api/captures` with action/animal/next custodian when applicable.
2. API reads its projection, rejects obvious conflicts, and creates an immutable/expiring context.
3. Agent polls authenticated `GET /api/agent/commands`.
4. Station receives the expected context, observes RFID, computes the new hash when applicable, and signs `StationEvent`.
5. Agent persists `LOCAL` before `POST /api/agent/evidence`.
6. API recomputes the observed RFID hash, event hash, StationID, and P-256 verification, then compares every signed field with the capture context.
7. Accepted evidence is append-only.
8. Web requests `transaction-data`, validates program/order/signing wallet, signs, and broadcasts.
9. `submit` stores the transaction signature only after confirmed RPC proves the exact Lastro envelope. PostgreSQL projection state does not advance.
10. `confirm` accepts only that stored signature, requires the same exact transaction at finalized commitment, decodes `AnimalState`/`RfidBinding`, and only then updates `animals`.

## Capture authority boundary

`POST /api/captures` has no wallet authentication and does not claim custody authorization. The route only freezes the context the Station may sign. For ORIGIN, `nextCustodian` is required; for TRANSFER it is required and must differ from the current custodian; for REIDENTIFY it must be absent and the API derives `from==to` from current state. The required signer appears in `transaction-data`; definitive authority proof occurs through the wallet signature and on-chain validation.

## RFID lookup

`GET /api/animals/by-rfid/:rfidHash` accepts an already-known canonical hash, queries the projection, and confirms that the corresponding `RfidBinding` is `ACTIVE` on-chain. This route does not imply that DemoPage has a separate physical read mode without a capture. The hackathon core demonstrates REIDENTIFY recovery through `visual_recovery_id`; a dedicated standalone read UX for recovery may be added in a pilot without changing the three domain operations.

## Idempotency

- Identical evidence may be resent and receives an idempotent result.
- Repeating `POST /api/captures` for the same action/destination while that animal has `EVIDENCE_ACCEPTED` or `SUBMITTED` evidence returns the original capture; a different intent returns `409 Conflict`.
- The same capture/event identifier with divergent bytes returns `409 Conflict`.
- Repeating `submit` with the same RPC-verified transaction signature is idempotent; a different signature cannot replace it.
- Repeating `confirm` for the same finalized transaction returns the same terminal state without duplicating the event.

## Resource and input bounds

The application rejects JSON request bodies larger than **1024 bytes** before route processing. The largest request in the frozen hackathon protocol is `AgentEvidenceRequest`: a 276-byte StationEvent is exactly 368 Base64 characters, plus one UUID and the fixed 8/33/64-byte evidence fields; compact canonical JSON is about 720 bytes. The 1 KiB envelope leaves representation headroom without accepting Axum's generic multi-megabyte JSON buffering limit.

Fixed identifiers are validated before cryptographic or persistence work: AnimalID/event/RFID hashes are 64 lowercase hexadecimal characters, Station public keys are 66 lowercase hexadecimal characters, compact Station signatures are 128 lowercase hexadecimal characters, visual recovery identifiers are 1..64 characters, capture identifiers are UUIDs, and Solana transaction signatures are 64..88 Base58 characters and must decode canonically to 64 bytes before submission.

Backend Solana RPC calls have a 10-second request timeout. The independent browser verifier uses the same 10-second RPC deadline and reports RPC/network timeout as `NOT_CHECKED`, never as `VALID` or contradictory `INVALID` evidence.

EvidencePackage event history has no application-imposed count cap because the canonical protocol permits history to grow with successive valid transitions. The transport instead bounds every fixed-size event field before decoding. There are currently no client-supplied JSON arrays on mutation endpoints; if deployment-scale history requires pagination later, that must preserve complete verification semantics rather than silently truncate canonical history.

Application-level request-rate limiting is not part of the hackathon protocol. With fixed body sizes, a bounded PostgreSQL pool, and RPC deadlines, deployment-wide request frequency controls belong at the external ingress/reverse-proxy boundary where client identity and topology are known. That deployment boundary must be validated separately for any public exposure.

## Errors

`400` invalid input/format; `401` Agent token; `404` resource; `409` state/immutability conflict; `5xx` internal/RPC/DB failure. Responses never include tokens, credential-bearing URLs, or secret material.
