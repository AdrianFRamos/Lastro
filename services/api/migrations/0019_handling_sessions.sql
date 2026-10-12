-- SPEC v2.1 S3/S4a/S8: handling sessions and batched presence proofs.
-- The on-chain SessionGrant and BatchAnchor are canonical; these tables hold the off-chain
-- evidence (every Station-signed leaf) and the projection of what the chain admitted.

CREATE TABLE v2_sessions (
    session_id        BYTEA PRIMARY KEY CHECK (octet_length(session_id) = 32),
    deployment_id     BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    custodian         BYTEA NOT NULL CHECK (octet_length(custodian) = 32),
    station_id        BYTEA NOT NULL CHECK (octet_length(station_id) = 32),
    -- Facility the Station was installed at when the session was prepared (S5).
    facility_id       BYTEA NOT NULL CHECK (octet_length(facility_id) = 32),
    max_events        INTEGER NOT NULL CHECK (max_events BETWEEN 1 AND 2000),
    valid_from        BIGINT NOT NULL,
    expires_at        BIGINT NOT NULL CHECK (expires_at > valid_from AND expires_at - valid_from <= 43200),
    revoked_at        BIGINT,
    status            TEXT NOT NULL CHECK (status IN ('PREPARED','OPEN','REVOKED')),
    open_tx_signature TEXT UNIQUE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX v2_sessions_station_idx ON v2_sessions(deployment_id, station_id, status);
-- Bounds live sessions per custodian, and the manifest's lookup of the custodian's assets.
CREATE INDEX v2_sessions_custodian_idx ON v2_sessions(deployment_id, custodian, status);
CREATE INDEX v2_assets_custodian_idx ON v2_assets(deployment_id, custodian) WHERE status = 1;

-- What the Agent was told when the session opened: the canonical head each observation
-- must extend. Frozen for the session, because a later chain change must not make an honest
-- offline observation look forged.
CREATE TABLE v2_session_manifest (
    session_id      BYTEA NOT NULL REFERENCES v2_sessions(session_id),
    asset_id        BYTEA NOT NULL CHECK (octet_length(asset_id) = 32),
    rfid_hash       BYTEA NOT NULL CHECK (octet_length(rfid_hash) = 32),
    state_version   BIGINT NOT NULL CHECK (state_version >= 1),
    last_event_hash BYTEA NOT NULL CHECK (octet_length(last_event_hash) = 32),
    PRIMARY KEY (session_id, asset_id),
    UNIQUE (session_id, rfid_hash)
);

CREATE TABLE v2_session_batches (
    session_id        BYTEA NOT NULL REFERENCES v2_sessions(session_id),
    batch_index       INTEGER NOT NULL CHECK (batch_index >= 0),
    merkle_root       BYTEA NOT NULL CHECK (octet_length(merkle_root) = 32),
    leaf_count        INTEGER NOT NULL CHECK (leaf_count BETWEEN 1 AND 1024),
    first_observed_at BIGINT NOT NULL,
    last_observed_at  BIGINT NOT NULL CHECK (last_observed_at >= first_observed_at),
    status            TEXT NOT NULL CHECK (status IN ('PREPARED','ANCHORED')),
    tx_signature      TEXT UNIQUE,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (session_id, batch_index)
);

-- One presence proof per asset per session (the event id is derived from both).
CREATE TABLE v2_session_observations (
    event_hash          BYTEA PRIMARY KEY CHECK (octet_length(event_hash) = 32),
    session_id          BYTEA NOT NULL REFERENCES v2_sessions(session_id),
    asset_id            BYTEA NOT NULL CHECK (octet_length(asset_id) = 32),
    envelope            BYTEA NOT NULL CHECK (octet_length(envelope) = 220),
    station_pubkey      BYTEA NOT NULL CHECK (octet_length(station_pubkey) = 33),
    station_signature   BYTEA NOT NULL CHECK (octet_length(station_signature) = 64),
    observed_rfid       BYTEA NOT NULL CHECK (octet_length(observed_rfid) = 8),
    observed_at         BIGINT NOT NULL,
    batch_index         INTEGER,
    leaf_index          INTEGER CHECK (leaf_index IS NULL OR leaf_index BETWEEN 0 AND 1023),
    status              TEXT NOT NULL CHECK (status IN ('PENDING','BATCHED','ANCHORED')),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (session_id, asset_id),
    FOREIGN KEY (session_id, batch_index) REFERENCES v2_session_batches(session_id, batch_index),
    CHECK ((status = 'PENDING') = (batch_index IS NULL)),
    CHECK ((batch_index IS NULL) = (leaf_index IS NULL))
);

CREATE INDEX v2_session_observations_pending_idx
    ON v2_session_observations(session_id, status, observed_at, event_hash);
CREATE INDEX v2_session_observations_asset_idx
    ON v2_session_observations(asset_id, status);

-- Evidence is append-only: the signed bytes of an observation never change.
CREATE OR REPLACE FUNCTION prevent_v2_session_evidence_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'session observations are append-only';
    END IF;
    IF NEW.event_hash <> OLD.event_hash OR NEW.session_id <> OLD.session_id
        OR NEW.asset_id <> OLD.asset_id OR NEW.envelope <> OLD.envelope
        OR NEW.station_pubkey <> OLD.station_pubkey OR NEW.station_signature <> OLD.station_signature
        OR NEW.observed_rfid <> OLD.observed_rfid OR NEW.observed_at <> OLD.observed_at THEN
        RAISE EXCEPTION 'session observation evidence is immutable';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER v2_session_observations_immutable
    BEFORE UPDATE OR DELETE ON v2_session_observations
    FOR EACH ROW EXECUTE FUNCTION prevent_v2_session_evidence_mutation();

-- Sponsored session transactions.
ALTER TABLE v2_sponsor_ledger DROP CONSTRAINT v2_sponsor_ledger_purpose_check;
ALTER TABLE v2_sponsor_ledger ADD CONSTRAINT v2_sponsor_ledger_purpose_check
    CHECK (purpose IN ('STATION_EVENT','REGISTER_ASSET','CUSTODY_PROPOSE','CUSTODY_ACCEPT',
                       'SESSION_OPEN','SESSION_BATCH','SESSION_REVOKE'));
