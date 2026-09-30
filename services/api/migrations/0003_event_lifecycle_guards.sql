BEGIN;

ALTER TABLE events
ADD CONSTRAINT events_status_signature_consistency CHECK (
    (status = 'EVIDENCE_ACCEPTED' AND tx_signature IS NULL)
    OR status = 'REJECTED'
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
    ELSIF OLD.status IN ('EVIDENCE_ACCEPTED','EXPIRED','CANCELLED') AND NEW.status <> OLD.status THEN
        RAISE EXCEPTION 'terminal capture lifecycle cannot change';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER captures_context_lifecycle_guard
BEFORE UPDATE ON captures
FOR EACH ROW EXECUTE FUNCTION enforce_capture_context_and_lifecycle();

CREATE OR REPLACE FUNCTION enforce_event_lifecycle() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.tx_signature IS NOT NULL AND NEW.tx_signature IS DISTINCT FROM OLD.tx_signature THEN
        RAISE EXCEPTION 'event transaction signature is immutable once set';
    END IF;

    IF OLD.status = 'EVIDENCE_ACCEPTED' AND NEW.status NOT IN ('EVIDENCE_ACCEPTED','SUBMITTED','REJECTED') THEN
        RAISE EXCEPTION 'event lifecycle must pass through SUBMITTED before FINALIZED';
    ELSIF OLD.status = 'SUBMITTED' AND NEW.status NOT IN ('SUBMITTED','FINALIZED') THEN
        RAISE EXCEPTION 'submitted event lifecycle cannot regress';
    ELSIF OLD.status IN ('FINALIZED','REJECTED') AND NEW.status <> OLD.status THEN
        RAISE EXCEPTION 'terminal event lifecycle cannot change';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER events_lifecycle_guard
BEFORE UPDATE ON events
FOR EACH ROW EXECUTE FUNCTION enforce_event_lifecycle();

CREATE OR REPLACE FUNCTION prevent_event_deletion() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'accepted event history is append-only';
END;
$$;

CREATE TRIGGER events_no_delete
BEFORE DELETE ON events
FOR EACH ROW EXECUTE FUNCTION prevent_event_deletion();

COMMIT;
