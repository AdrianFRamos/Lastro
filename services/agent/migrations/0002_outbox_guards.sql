CREATE TRIGGER IF NOT EXISTS outbox_evidence_is_immutable
BEFORE UPDATE OF event_hash, capture_id, event_bytes, observed_rfid, station_pubkey, station_signature
ON outbox
BEGIN
    SELECT RAISE(ABORT, 'outbox evidence is immutable');
END;

CREATE TRIGGER IF NOT EXISTS outbox_state_is_monotonic
BEFORE UPDATE OF state ON outbox
WHEN NOT (
    NEW.state = OLD.state
    OR (OLD.state = 'LOCAL' AND NEW.state = 'SERVER')
    OR (OLD.state = 'SERVER' AND NEW.state = 'FINALIZED')
)
BEGIN
    SELECT RAISE(ABORT, 'outbox state transition is not monotonic');
END;
