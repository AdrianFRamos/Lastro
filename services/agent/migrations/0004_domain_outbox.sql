CREATE TABLE IF NOT EXISTS domain_outbox (
    event_hash BLOB PRIMARY KEY CHECK (length(event_hash) = 32),
    envelope_bytes BLOB NOT NULL CHECK (length(envelope_bytes) = 220),
    station_pubkey BLOB NOT NULL CHECK (length(station_pubkey) = 33),
    station_signature BLOB NOT NULL CHECK (length(station_signature) = 64),
    state TEXT NOT NULL CHECK (state IN ('LOCAL', 'SERVER', 'QUARANTINED')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_domain_outbox_state_created
    ON domain_outbox(state, created_at, event_hash);

CREATE TRIGGER IF NOT EXISTS domain_outbox_evidence_is_immutable
BEFORE UPDATE OF event_hash, envelope_bytes, station_pubkey, station_signature
ON domain_outbox
BEGIN
    SELECT RAISE(ABORT, 'domain outbox evidence is immutable');
END;

CREATE TRIGGER IF NOT EXISTS domain_outbox_state_is_monotonic
BEFORE UPDATE OF state ON domain_outbox
WHEN NOT (
    NEW.state = OLD.state
    OR (OLD.state = 'LOCAL' AND NEW.state = 'SERVER')
    OR (OLD.state = 'LOCAL' AND NEW.state = 'QUARANTINED')
    OR (OLD.state = 'SERVER' AND NEW.state = 'QUARANTINED')
)
BEGIN
    SELECT RAISE(ABORT, 'domain outbox state transition is not monotonic');
END;
