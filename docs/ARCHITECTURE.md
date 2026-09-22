# Architecture

## Objective

The architecture exists to prove six properties:

1. Real RFID is observed by hardware.
2. The Station binds the observation to transition context and signs the event.
3. The current custodian authorizes the transition.
4. Solana maintains one canonical state per `AnimalID`.
5. `AnimalID` survives RFID replacement.
6. A third party verifies history without trusting the backend verdict.

## Flow

```text
FDX-B tag
  ↓
ESP32-C5 Station
  ↓ StationEvent[276] + P-256 signature
Rust Agent
  ↓ durable SQLite outbox
Rust API
  ↓ transaction instructions
Vue + Wallet Standard
  ↓ wallet signature
Secp256r1 precompile
  ↓
Lastro Anchor program
  ↓
ProtocolConfig + AnimalState + RfidBinding PDAs
  ↓
Browser verifier + Solana RPC
```

## Trust boundaries

**Station:** proves that its firmware received a valid RFID reading during a capture and signed the event. It does not prove biological identity.

**Agent:** transports and persists. It cannot modify `event_bytes`, observed RFID, key, or signature.

**API:** coordinates capture and prepares transactions. PostgreSQL is a projection; it is not the canonical authority.

**Wallet:** proves the custodian authority required by on-chain state.

**Solana:** decides whether the transition may advance the current state.

**Verifier:** recomputes bytes, hashes, signature, sequence, predecessor, revision, RFID/custody transitions, and compares the terminal state with RPC.

## On-chain state

`ProtocolConfig`: authority, deployment ID, and authorized Station P-256 key.

`AnimalState`: current canonical state per `AnimalID`.

`RfidBinding`: canonical index by `rfid_hash`, with `ACTIVE` or `RETIRED`, preventing two animals from sharing the same active RFID.

## Persistence

- Station: no persistent journal in the hackathon.
- Agent: SQLite outbox is the first durable boundary.
- API: PostgreSQL stores the projection and append-only evidence.
- Solana: canonical source of custody, revision, sequence, and current binding.
