BEGIN;

CREATE TABLE capture_authorization_challenges (
    challenge_id        UUID PRIMARY KEY,
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    action              SMALLINT NOT NULL CHECK (action IN (1,2,3)),
    animal_id           BYTEA NOT NULL REFERENCES animals(animal_id) ON DELETE RESTRICT CHECK (octet_length(animal_id) = 32),
    next_custodian      BYTEA CHECK (next_custodian IS NULL OR octet_length(next_custodian) = 32),
    required_signer     BYTEA NOT NULL CHECK (octet_length(required_signer) = 32),
    message_bytes       BYTEA NOT NULL CHECK (octet_length(message_bytes) BETWEEN 1 AND 1024),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at          TIMESTAMPTZ NOT NULL CHECK (expires_at > created_at),
    used_at             TIMESTAMPTZ,
    CHECK (used_at IS NULL OR used_at >= created_at)
);

CREATE INDEX capture_authorization_expiry_idx
ON capture_authorization_challenges (expires_at)
WHERE used_at IS NULL;

COMMIT;
