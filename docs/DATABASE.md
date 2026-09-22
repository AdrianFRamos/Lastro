# Persistence

## PostgreSQL

Canonical migrations: `services/api/migrations/0001_initial.sql` through `0003_event_lifecycle_guards.sql`.

### animals
Read/UI projection. `current_rfid_hash` is UNIQUE when present so the projection cannot contradict the on-chain index. Before ORIGIN, canonical fields may be null and sequence/revision remain zero.

### events
Append-only after acceptance. Database triggers reject deletion and changes to `event_hash`, animal, sequence, action, event bytes, observed RFID, public key, or Station signature. Lifecycle metadata is monotonic: `EVIDENCE_ACCEPTED → SUBMITTED → FINALIZED` (or `EVIDENCE_ACCEPTED → REJECTED`), and a transaction signature cannot change once attached. `SUBMITTED`/`FINALIZED` rows require a non-null transaction signature.

### captures
Temporary sessions. `station_id` participates in the partial index that guarantees at most one `PENDING/DISPATCHED` capture per Station. Expiration is mandatory; an expired capture does not accept evidence.

## SQLite in the Agent

`LOCAL → SERVER → FINALIZED`.

The SQLite transaction that creates `LOCAL` must commit before the POST. After restart, every item that is not FINALIZED is resumed. A divergent duplicate never replaces the original.
