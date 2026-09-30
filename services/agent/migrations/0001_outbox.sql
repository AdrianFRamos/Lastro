CREATE TABLE IF NOT EXISTS outbox (
    event_hash BLOB PRIMARY KEY CHECK (length(event_hash) = 32),
    capture_id TEXT NOT NULL UNIQUE,
    event_bytes BLOB NOT NULL CHECK (length(event_bytes) = 276),
    observed_rfid BLOB NOT NULL CHECK (length(observed_rfid) = 8),
    station_pubkey BLOB NOT NULL CHECK (length(station_pubkey) = 33),
    station_signature BLOB NOT NULL CHECK (length(station_signature) = 64),
    state TEXT NOT NULL CHECK (state IN ('LOCAL', 'SERVER', 'FINALIZED')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_outbox_state_created ON outbox(state, created_at);
