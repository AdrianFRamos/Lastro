BEGIN;

CREATE INDEX capture_authorizations_recent_idx
ON capture_authorization_challenges (created_at DESC);

CREATE INDEX capture_authorizations_signer_recent_idx
ON capture_authorization_challenges (required_signer, created_at DESC);

CREATE INDEX capture_authorizations_animal_recent_idx
ON capture_authorization_challenges (animal_id, created_at DESC);

CREATE INDEX events_finalized_history_idx
ON events (animal_id, event_sequence)
WHERE status = 'FINALIZED';

CREATE INDEX animals_unoriginated_created_idx
ON animals (created_at DESC)
WHERE event_sequence = 0;

COMMIT;
