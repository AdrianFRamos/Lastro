BEGIN;

CREATE TABLE reconciliation_jobs (
    job_id          BIGSERIAL PRIMARY KEY,
    kind            TEXT NOT NULL CHECK (kind IN ('LEGACY_EVENT', 'DOMAIN_EVENT')),
    target_hash     BYTEA NOT NULL CHECK (octet_length(target_hash) = 32),
    tx_signature    TEXT NOT NULL CHECK (length(tx_signature) BETWEEN 80 AND 100),
    status          TEXT NOT NULL DEFAULT 'PENDING'
                    CHECK (status IN ('PENDING', 'CLAIMED', 'RETRY', 'DONE', 'QUARANTINED')),
    attempts        INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    available_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    locked_by       TEXT,
    locked_until    TIMESTAMPTZ,
    last_error      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at    TIMESTAMPTZ,
    UNIQUE (kind, target_hash)
);

CREATE INDEX reconciliation_jobs_claim_idx
    ON reconciliation_jobs(status, available_at, job_id)
    WHERE status IN ('PENDING', 'RETRY', 'CLAIMED');

CREATE INDEX reconciliation_jobs_target_idx
    ON reconciliation_jobs(kind, target_hash);

CREATE OR REPLACE FUNCTION enforce_reconciliation_job_lifecycle() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.kind IS DISTINCT FROM OLD.kind
       OR NEW.target_hash IS DISTINCT FROM OLD.target_hash
       OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
        RAISE EXCEPTION 'reconciliation job identity is immutable';
    END IF;

    IF OLD.status = 'DONE' AND NEW.status <> 'DONE' THEN
        RAISE EXCEPTION 'completed reconciliation job cannot regress';
    ELSIF OLD.status = 'QUARANTINED' AND NEW.status NOT IN ('QUARANTINED', 'PENDING') THEN
        RAISE EXCEPTION 'quarantined reconciliation job requires explicit requeue';
    END IF;

    IF NEW.status = 'DONE' AND NEW.completed_at IS NULL THEN
        RAISE EXCEPTION 'completed reconciliation job requires completed_at';
    END IF;
    IF NEW.status <> 'DONE' AND NEW.completed_at IS NOT NULL THEN
        RAISE EXCEPTION 'non-completed reconciliation job cannot have completed_at';
    END IF;

    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

CREATE TRIGGER reconciliation_job_lifecycle_guard
BEFORE UPDATE ON reconciliation_jobs
FOR EACH ROW EXECUTE FUNCTION enforce_reconciliation_job_lifecycle();

COMMIT;
