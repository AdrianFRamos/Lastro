-- Physical capture flow on protocol v2 (replaces the v1 animals/captures/events pipeline).
BEGIN;

-- IDENTIFIER_BOUND (18) and IDENTIFIER_REPLACED (19) are Station-signed identity events.
ALTER TABLE v2_event_anchors DROP CONSTRAINT IF EXISTS v2_event_anchors_event_type_check;
ALTER TABLE v2_event_anchors
    ADD CONSTRAINT v2_event_anchors_event_type_check CHECK (event_type BETWEEN 1 AND 19);

-- Captured evidence keeps its capture link and the RFID the Station reported reading.
ALTER TABLE v2_event_anchors
    ADD COLUMN capture_id UUID UNIQUE,
    ADD COLUMN observed_rfid BYTEA CHECK (observed_rfid IS NULL OR octet_length(observed_rfid) = 8);

-- Abandoned evidence (never signed by a wallet) may be superseded at the same state version.
-- The chain stays canonical: its predecessor/version guard lets at most one of them land.
DROP INDEX IF EXISTS v2_event_subject_version_idx;
CREATE UNIQUE INDEX v2_event_subject_version_idx
    ON v2_event_anchors(subject_id, state_version)
    WHERE status <> 'REJECTED';

ALTER TABLE v2_assets
    ADD COLUMN current_rfid_hash BYTEA
        CHECK (current_rfid_hash IS NULL OR octet_length(current_rfid_hash) = 32);
CREATE INDEX v2_assets_rfid_idx ON v2_assets(current_rfid_hash) WHERE current_rfid_hash IS NOT NULL;

CREATE TABLE v2_capture_challenges (
    challenge_id        UUID PRIMARY KEY,
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    asset_id            BYTEA NOT NULL CHECK (octet_length(asset_id) = 32),
    event_type          SMALLINT NOT NULL CHECK (event_type IN (2, 18, 19)),
    required_signer     BYTEA NOT NULL CHECK (octet_length(required_signer) = 32),
    message_bytes       BYTEA NOT NULL CHECK (octet_length(message_bytes) BETWEEN 1 AND 1024),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at          TIMESTAMPTZ NOT NULL CHECK (expires_at > created_at),
    used_at             TIMESTAMPTZ CHECK (used_at IS NULL OR used_at >= created_at)
);
CREATE INDEX v2_capture_challenges_recent_idx ON v2_capture_challenges(created_at DESC);
CREATE INDEX v2_capture_challenges_signer_idx ON v2_capture_challenges(required_signer, created_at DESC);
CREATE INDEX v2_capture_challenges_asset_idx ON v2_capture_challenges(asset_id, created_at DESC);

CREATE TABLE v2_captures (
    capture_id          UUID PRIMARY KEY,
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    station_id          BYTEA NOT NULL CHECK (octet_length(station_id) = 32),
    asset_id            BYTEA NOT NULL CHECK (octet_length(asset_id) = 32),
    event_type          SMALLINT NOT NULL CHECK (event_type IN (2, 18, 19)),
    event_id            BYTEA NOT NULL UNIQUE CHECK (octet_length(event_id) = 32),
    state_version       BIGINT NOT NULL CHECK (state_version > 0),
    previous_event_hash BYTEA NOT NULL CHECK (octet_length(previous_event_hash) = 32),
    expected_rfid_hash  BYTEA NOT NULL CHECK (octet_length(expected_rfid_hash) = 32),
    required_signer     BYTEA NOT NULL CHECK (octet_length(required_signer) = 32),
    observed_at         BIGINT NOT NULL CHECK (observed_at >= 0),
    expires_at          BIGINT NOT NULL CHECK (expires_at > observed_at),
    status              TEXT NOT NULL CHECK (status IN ('PENDING','DISPATCHED','EVIDENCE_ACCEPTED','EXPIRED','CANCELLED')),
    event_hash          BYTEA CHECK (event_hash IS NULL OR octet_length(event_hash) = 32),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Exactly one capture may own the next physical read of a Station.
CREATE UNIQUE INDEX v2_captures_one_active_per_station
    ON v2_captures(station_id) WHERE status IN ('PENDING','DISPATCHED');
CREATE INDEX v2_captures_asset_idx ON v2_captures(asset_id, created_at DESC);

COMMIT;
