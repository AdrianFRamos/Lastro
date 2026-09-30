-- Protocol v1 is removed: the product runs on v2 only (docs/MIGRACAO_V1_PARA_V2.md).
-- Earlier migrations stay untouched so existing databases keep a valid migration history.
BEGIN;

DROP TABLE IF EXISTS v2_migration_candidates;
DROP TABLE IF EXISTS v2_migration_runs;
DROP TABLE IF EXISTS events;
DROP TABLE IF EXISTS capture_authorization_challenges;
DROP TABLE IF EXISTS captures;
DROP TABLE IF EXISTS animals;

DROP FUNCTION IF EXISTS prevent_event_evidence_mutation();
DROP FUNCTION IF EXISTS enforce_capture_context_and_lifecycle();
DROP FUNCTION IF EXISTS enforce_event_lifecycle();
DROP FUNCTION IF EXISTS prevent_event_deletion();
DROP FUNCTION IF EXISTS prevent_v2_migration_identity_mutation();

DELETE FROM reconciliation_jobs WHERE kind = 'LEGACY_EVENT';
ALTER TABLE reconciliation_jobs DROP CONSTRAINT IF EXISTS reconciliation_jobs_kind_check;
ALTER TABLE reconciliation_jobs
    ADD CONSTRAINT reconciliation_jobs_kind_check CHECK (kind = 'DOMAIN_EVENT');

COMMIT;
