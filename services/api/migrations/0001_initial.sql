BEGIN;

CREATE TABLE animals (
    animal_id           BYTEA PRIMARY KEY CHECK (octet_length(animal_id) = 32),
    visual_recovery_id  TEXT NOT NULL UNIQUE CHECK (length(visual_recovery_id) BETWEEN 1 AND 64 AND visual_recovery_id = btrim(visual_recovery_id)),
    current_rfid_hash   BYTEA UNIQUE CHECK (current_rfid_hash IS NULL OR octet_length(current_rfid_hash) = 32),
    current_custodian   BYTEA CHECK (current_custodian IS NULL OR octet_length(current_custodian) = 32),
    identity_revision   INTEGER NOT NULL DEFAULT 0 CHECK (identity_revision >= 0),
    event_sequence      BIGINT NOT NULL DEFAULT 0 CHECK (event_sequence >= 0),
    last_event_hash     BYTEA CHECK (last_event_hash IS NULL OR octet_length(last_event_hash) = 32),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE events (
    event_hash          BYTEA PRIMARY KEY CHECK (octet_length(event_hash) = 32),
    animal_id           BYTEA NOT NULL REFERENCES animals(animal_id) ON DELETE RESTRICT CHECK (octet_length(animal_id) = 32),
    event_sequence      BIGINT NOT NULL CHECK (event_sequence > 0),
    action              SMALLINT NOT NULL CHECK (action IN (1,2,3)),
    event_bytes         BYTEA NOT NULL CHECK (octet_length(event_bytes) = 276),
    observed_rfid       BYTEA NOT NULL CHECK (octet_length(observed_rfid) = 8),
    station_pubkey      BYTEA NOT NULL CHECK (octet_length(station_pubkey) = 33),
    station_signature   BYTEA NOT NULL CHECK (octet_length(station_signature) = 64),
    tx_signature        TEXT UNIQUE,
    status              TEXT NOT NULL CHECK (status IN ('EVIDENCE_ACCEPTED','SUBMITTED','FINALIZED','REJECTED')),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (animal_id, event_sequence)
);

CREATE TABLE captures (
    capture_id              UUID PRIMARY KEY,
    station_id              BYTEA NOT NULL CHECK (octet_length(station_id) = 32),
    action                  SMALLINT NOT NULL CHECK (action IN (1,2,3)),
    animal_id               BYTEA NOT NULL REFERENCES animals(animal_id) ON DELETE RESTRICT CHECK (octet_length(animal_id) = 32),
    event_sequence          BIGINT NOT NULL CHECK (event_sequence > 0),
    identity_revision       INTEGER NOT NULL CHECK (identity_revision > 0),
    expected_old_rfid_hash  BYTEA NOT NULL CHECK (octet_length(expected_old_rfid_hash) = 32),
    from_custodian          BYTEA NOT NULL CHECK (octet_length(from_custodian) = 32),
    to_custodian            BYTEA NOT NULL CHECK (octet_length(to_custodian) = 32),
    previous_event_hash     BYTEA NOT NULL CHECK (octet_length(previous_event_hash) = 32),
    status                  TEXT NOT NULL CHECK (status IN ('PENDING','DISPATCHED','EVIDENCE_ACCEPTED','EXPIRED','CANCELLED')),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at              TIMESTAMPTZ NOT NULL CHECK (expires_at > created_at)
);

-- PostgreSQL does not support a partial UNIQUE constraint directly; this index enforces one active capture per Station.
CREATE UNIQUE INDEX captures_one_active_per_station
ON captures (station_id)
WHERE status IN ('PENDING','DISPATCHED');

CREATE INDEX events_animal_sequence_idx ON events(animal_id, event_sequence);
CREATE INDEX captures_status_created_idx ON captures(status, created_at);

-- Cryptographic event immutability: the application may attach tx_signature/status,
-- while event_bytes/observed_rfid/pubkey/signature/hash/animal/sequence/action never change.
CREATE OR REPLACE FUNCTION prevent_event_evidence_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.event_hash <> OLD.event_hash
       OR NEW.animal_id <> OLD.animal_id
       OR NEW.event_sequence <> OLD.event_sequence
       OR NEW.action <> OLD.action
       OR NEW.event_bytes <> OLD.event_bytes
       OR NEW.observed_rfid <> OLD.observed_rfid
       OR NEW.station_pubkey <> OLD.station_pubkey
       OR NEW.station_signature <> OLD.station_signature THEN
        RAISE EXCEPTION 'cryptographic event evidence is immutable';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER events_immutable_evidence
BEFORE UPDATE ON events
FOR EACH ROW EXECUTE FUNCTION prevent_event_evidence_mutation();

COMMIT;
