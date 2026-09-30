BEGIN;

CREATE TABLE v2_assets (
    asset_id                BYTEA PRIMARY KEY CHECK (octet_length(asset_id) = 32),
    deployment_id           BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    asset_type              SMALLINT NOT NULL CHECK (asset_type BETWEEN 1 AND 8),
    status                  SMALLINT NOT NULL CHECK (status BETWEEN 1 AND 7),
    custodian               BYTEA CHECK (custodian IS NULL OR octet_length(custodian) = 32),
    parent_root             BYTEA NOT NULL CHECK (octet_length(parent_root) = 32),
    lineage_root            BYTEA NOT NULL CHECK (octet_length(lineage_root) = 32),
    current_lot_id          BYTEA NOT NULL CHECK (octet_length(current_lot_id) = 32),
    available_weight_grams  BIGINT NOT NULL CHECK (available_weight_grams >= 0),
    event_sequence          BIGINT NOT NULL DEFAULT 0 CHECK (event_sequence >= 0),
    state_version           BIGINT NOT NULL DEFAULT 0 CHECK (state_version >= 0),
    last_event_hash         BYTEA CHECK (last_event_hash IS NULL OR octet_length(last_event_hash) = 32),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX v2_assets_deployment_idx ON v2_assets(deployment_id);

CREATE TABLE v2_event_anchors (
    event_id                 BYTEA PRIMARY KEY CHECK (octet_length(event_id) = 32),
    event_hash               BYTEA NOT NULL UNIQUE CHECK (octet_length(event_hash) = 32),
    deployment_id            BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    subject_id               BYTEA NOT NULL CHECK (octet_length(subject_id) = 32),
    source_id                BYTEA NOT NULL CHECK (octet_length(source_id) = 32),
    event_type               SMALLINT NOT NULL CHECK (event_type BETWEEN 1 AND 17),
    state_version            BIGINT NOT NULL CHECK (state_version >= 0),
    observed_at              BIGINT NOT NULL CHECK (observed_at >= 0),
    expires_at               BIGINT NOT NULL CHECK (expires_at >= observed_at),
    expected_previous_hash  BYTEA NOT NULL CHECK (octet_length(expected_previous_hash) = 32),
    payload_hash             BYTEA NOT NULL CHECK (octet_length(payload_hash) = 32),
    envelope_bytes           BYTEA NOT NULL CHECK (octet_length(envelope_bytes) = 220),
    station_pubkey           BYTEA NOT NULL CHECK (octet_length(station_pubkey) = 33),
    station_signature        BYTEA NOT NULL CHECK (octet_length(station_signature) = 64),
    status                   TEXT NOT NULL CHECK (status IN ('EVIDENCE_ACCEPTED','SUBMITTED','FINALIZED','REJECTED')),
    tx_signature             TEXT UNIQUE,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX v2_event_subject_version_idx
    ON v2_event_anchors(subject_id, state_version);
CREATE INDEX v2_event_subject_observed_idx
    ON v2_event_anchors(subject_id, observed_at, event_id);
CREATE INDEX v2_event_deployment_idx
    ON v2_event_anchors(deployment_id, created_at);

CREATE OR REPLACE FUNCTION prevent_v2_event_evidence_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.event_id <> OLD.event_id
       OR NEW.event_hash <> OLD.event_hash
       OR NEW.deployment_id <> OLD.deployment_id
       OR NEW.subject_id <> OLD.subject_id
       OR NEW.source_id <> OLD.source_id
       OR NEW.event_type <> OLD.event_type
       OR NEW.state_version <> OLD.state_version
       OR NEW.observed_at <> OLD.observed_at
       OR NEW.expires_at <> OLD.expires_at
       OR NEW.expected_previous_hash <> OLD.expected_previous_hash
       OR NEW.payload_hash <> OLD.payload_hash
       OR NEW.envelope_bytes <> OLD.envelope_bytes
       OR NEW.station_pubkey <> OLD.station_pubkey
       OR NEW.station_signature <> OLD.station_signature THEN
        RAISE EXCEPTION 'v2 event evidence is immutable';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER v2_events_immutable_evidence
BEFORE UPDATE ON v2_event_anchors
FOR EACH ROW EXECUTE FUNCTION prevent_v2_event_evidence_mutation();

COMMIT;
