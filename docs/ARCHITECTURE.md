# Architecture

## Objective

The architecture exists to prove six properties:

1. Real RFID is observed by hardware.
2. The Station binds the observation to the asset's canonical context and signs it.
3. The right wallet authorizes the transition (current custodian for identity, deployment
   authority for presence proofs, the recipient for custody acceptance).
4. Solana maintains one canonical state per AssetID.
5. The AssetID survives RFID replacement; a retired tag still leads back to its animal.
6. A third party verifies history without trusting the backend verdict.

## Flow

```text
FDX-B tag
  ↓
ESP32-C5 Station
  ↓ v2 domain envelope[220] + P-256 signature
Rust Agent
  ↓ durable SQLite outbox
Rust API
  ↓ transaction instructions
Vue + Wallet Standard
  ↓ wallet signature
Secp256r1 precompile
  ↓
Lastro v2 Anchor program
  ↓
ProtocolConfigV2 + AssetState + RfidBinding + EventAnchor PDAs
  ↓
Browser verifier + Solana RPC
```

## Trust boundaries

**Station:** proves that its firmware read a valid RFID during a capture and signed the envelope
for the context it was given. It does not prove biological identity or the time of the read.

**Agent:** transports and persists. It cannot modify the envelope, observed RFID, key or signature.

**API:** coordinates capture and prepares transactions. PostgreSQL is a projection; it is not the
canonical authority.

**Wallet:** proves the authority required by on-chain state.

**Solana:** decides whether the transition may advance the current state.

**Verifier:** recomputes envelope bytes, hashes, signatures, the hash chain and RFID transitions,
and compares every anchor, binding and transaction with finalized RPC state.

## On-chain state

`ProtocolConfigV2`: authority, deployment ID, registries and limits.

`StationRecord`: authorized Station key per StationId.

`AssetState`: canonical state per AssetID, including custodian, `state_version`,
`last_event_hash` and `current_rfid_hash`.

`RfidBinding`: canonical index by `rfid_hash`, `ACTIVE` or `RETIRED`, never closed — one tag can
never identify two assets.

`EventAnchor`: one per anchored Station event.

## Persistence

- Station: no persistent journal.
- Agent: SQLite outbox is the first durable boundary.
- API: PostgreSQL stores the projection and append-only evidence.
- Solana: canonical source of custody, identity and event history.
