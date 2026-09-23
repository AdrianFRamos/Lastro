BEGIN;

ALTER TABLE capture_authorization_challenges
ADD COLUMN supersede_capture_id UUID;

COMMIT;
