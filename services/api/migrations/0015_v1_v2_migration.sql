BEGIN;

CREATE TABLE v2_migration_runs (
    run_id              UUID PRIMARY KEY,
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    requested_by_party_id BYTEA REFERENCES v2_parties(party_id),
    source_name         TEXT NOT NULL CHECK (source_name = 'V1_ANIMALS'),
    status              TEXT NOT NULL CHECK (status IN ('PLANNED','REVIEW_REQUIRED','EXECUTING','COMPLETED','ABORTED')),
    source_count        BIGINT NOT NULL DEFAULT 0 CHECK (source_count >= 0),
    eligible_count      BIGINT NOT NULL DEFAULT 0 CHECK (eligible_count >= 0),
    promoted_count      BIGINT NOT NULL DEFAULT 0 CHECK (promoted_count >= 0),
    rejected_count      BIGINT NOT NULL DEFAULT 0 CHECK (rejected_count >= 0),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at        TIMESTAMPTZ
);

CREATE TABLE v2_migration_candidates (
    run_id              UUID NOT NULL REFERENCES v2_migration_runs(run_id),
    source_animal_id    BYTEA NOT NULL REFERENCES animals(animal_id),
    target_asset_id     BYTEA CHECK (target_asset_id IS NULL OR octet_length(target_asset_id) = 32),
    status              TEXT NOT NULL CHECK (status IN ('PENDING_REVIEW','ELIGIBLE','PROMOTED','REJECTED','QUARANTINED')),
    reason              TEXT NOT NULL CHECK (length(reason) BETWEEN 2 AND 1000),
    source_event_sequence BIGINT NOT NULL CHECK (source_event_sequence >= 0),
    source_last_event_hash BYTEA CHECK (source_last_event_hash IS NULL OR octet_length(source_last_event_hash) = 32),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    reviewed_at         TIMESTAMPTZ,
    promoted_at        TIMESTAMPTZ,
    PRIMARY KEY (run_id, source_animal_id),
    UNIQUE (run_id, target_asset_id)
);

CREATE INDEX v2_migration_candidates_status_idx
    ON v2_migration_candidates(run_id, status, source_animal_id);
CREATE INDEX v2_migration_source_idx
    ON v2_migration_candidates(source_animal_id, created_at);

CREATE OR REPLACE FUNCTION prevent_v2_migration_identity_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.run_id <> OLD.run_id
       OR NEW.source_animal_id <> OLD.source_animal_id
       OR NEW.target_asset_id IS DISTINCT FROM OLD.target_asset_id
       OR NEW.source_event_sequence <> OLD.source_event_sequence
       OR NEW.source_last_event_hash IS DISTINCT FROM OLD.source_last_event_hash THEN
        RAISE EXCEPTION 'v2 migration evidence is immutable';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER v2_migration_candidate_identity_immutable
BEFORE UPDATE ON v2_migration_candidates
FOR EACH ROW EXECUTE FUNCTION prevent_v2_migration_identity_mutation();

COMMIT;
