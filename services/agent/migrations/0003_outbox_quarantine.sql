DROP TRIGGER IF EXISTS outbox_evidence_is_immutable;
DROP TRIGGER IF EXISTS outbox_state_is_monotonic;

CREATE TABLE outbox_v3 (
    event_hash BLOB PRIMARY KEY CHECK (length(event_hash) = 32),
    capture_id TEXT NOT NULL UNIQUE,
    event_bytes BLOB NOT NULL CHECK (length(event_bytes) = 276),
    observed_rfid BLOB NOT NULL CHECK (length(observed_rfid) = 8),
    station_pubkey BLOB NOT NULL CHECK (length(station_pubkey) = 33),
    station_signature BLOB NOT NULL CHECK (length(station_signature) = 64),
    state TEXT NOT NULL CHECK (state IN ('LOCAL', 'SERVER', 'FINALIZED', 'QUARANTINED')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO outbox_v3 (
    event_hash, capture_id, event_bytes, observed_rfid, station_pubkey,
    station_signature, state, attempts, last_error, created_at, updated_at
)
SELECT
    event_hash, capture_id, event_bytes, observed_rfid, station_pubkey,
    station_signature, state, attempts, last_error, created_at, updated_at
FROM outbox;

DROP TABLE outbox;
ALTER TABLE outbox_v3 RENAME TO outbox;

CREATE INDEX idx_outbox_state_created ON outbox(state, created_at);

CREATE TRIGGER outbox_evidence_is_immutable
BEFORE UPDATE OF event_hash, capture_id, event_bytes, observed_rfid, station_pubkey, station_signature
ON outbox
BEGIN
    SELECT RAISE(ABORT, 'outbox evidence is immutable');
END;

CREATE TRIGGER outbox_state_is_monotonic
BEFORE UPDATE OF state ON outbox
WHEN NOT (
    NEW.state = OLD.state
    OR (OLD.state = 'LOCAL' AND NEW.state = 'SERVER')
    OR (OLD.state = 'SERVER' AND NEW.state = 'FINALIZED')
    OR (OLD.state = 'LOCAL' AND NEW.state = 'QUARANTINED')
    OR (OLD.state = 'SERVER' AND NEW.state = 'QUARANTINED')
)
BEGIN
    SELECT RAISE(ABORT, 'outbox state transition is not monotonic');
END;
