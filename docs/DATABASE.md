# Persistence

## PostgreSQL

Migrations: `services/api/migrations/` (applied in order; `0017_drop_v1.sql` removed the v1 tables).

### v2_assets
Read projection mirrored from finalized `AssetState` (custodian, state version, last event hash,
current RFID hash). It never decides canonical state.

### v2_event_anchors
Append-only Station evidence (220-byte envelope, Station key and signature, observed RFID).
Lifecycle is monotonic: `EVIDENCE_ACCEPTED → SUBMITTED → FINALIZED` (or `→ REJECTED`), and a
transaction signature cannot change once attached. At most one non-rejected event exists per
asset and state version.

### v2_capture_challenges / v2_captures
Single-use wallet challenges (rate-limited) and temporary capture sessions. At most one active
capture exists per Station; an expired capture does not accept evidence.

## SQLite in the Agent

`LOCAL → SERVER → FINALIZED`.

The SQLite transaction that creates `LOCAL` must commit before the POST. After restart, every item that is not FINALIZED is resumed. A divergent duplicate never replaces the original.
