BEGIN;

ALTER TABLE events
DROP CONSTRAINT events_animal_id_event_sequence_key;

CREATE UNIQUE INDEX events_one_non_rejected_sequence_per_animal
ON events (animal_id, event_sequence)
WHERE status <> 'REJECTED';

ALTER TABLE events
DROP CONSTRAINT events_status_signature_consistency;

ALTER TABLE events
ADD CONSTRAINT events_status_signature_consistency CHECK (
    (status IN ('EVIDENCE_ACCEPTED','REJECTED') AND tx_signature IS NULL)
    OR (status IN ('SUBMITTED','FINALIZED') AND tx_signature IS NOT NULL)
);

CREATE OR REPLACE FUNCTION enforce_capture_context_and_lifecycle() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.capture_id IS DISTINCT FROM OLD.capture_id
       OR NEW.station_id IS DISTINCT FROM OLD.station_id
       OR NEW.action IS DISTINCT FROM OLD.action
       OR NEW.animal_id IS DISTINCT FROM OLD.animal_id
       OR NEW.event_sequence IS DISTINCT FROM OLD.event_sequence
       OR NEW.identity_revision IS DISTINCT FROM OLD.identity_revision
       OR NEW.expected_old_rfid_hash IS DISTINCT FROM OLD.expected_old_rfid_hash
       OR NEW.from_custodian IS DISTINCT FROM OLD.from_custodian
       OR NEW.to_custodian IS DISTINCT FROM OLD.to_custodian
       OR NEW.previous_event_hash IS DISTINCT FROM OLD.previous_event_hash
       OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
        RAISE EXCEPTION 'capture context is immutable';
    END IF;

    IF NEW.expires_at IS DISTINCT FROM OLD.expires_at THEN
        IF OLD.status NOT IN ('PENDING','DISPATCHED') THEN
            RAISE EXCEPTION 'terminal capture expiration is immutable';
        END IF;
        IF NEW.expires_at > OLD.expires_at THEN
            RAISE EXCEPTION 'capture expiration cannot be extended';
        END IF;
    END IF;

    IF OLD.status = 'PENDING' AND NEW.status NOT IN ('PENDING','DISPATCHED','EXPIRED','CANCELLED') THEN
        RAISE EXCEPTION 'pending capture lifecycle is invalid';
    ELSIF OLD.status = 'DISPATCHED' AND NEW.status NOT IN ('DISPATCHED','EVIDENCE_ACCEPTED','EXPIRED','CANCELLED') THEN
        RAISE EXCEPTION 'dispatched capture lifecycle is invalid';
    ELSIF OLD.status = 'EVIDENCE_ACCEPTED' AND NEW.status NOT IN ('EVIDENCE_ACCEPTED','CANCELLED') THEN
        RAISE EXCEPTION 'accepted capture lifecycle is invalid';
    ELSIF OLD.status IN ('EXPIRED','CANCELLED') AND NEW.status <> OLD.status THEN
        RAISE EXCEPTION 'terminal capture lifecycle cannot change';
    END IF;

    RETURN NEW;
END;
$$;

COMMIT;
