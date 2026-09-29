BEGIN;

CREATE TABLE v2_authority_grants (
    grant_id            UUID PRIMARY KEY,
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    party_id            BYTEA NOT NULL REFERENCES v2_parties(party_id),
    facility_id         BYTEA REFERENCES v2_facilities(facility_id),
    capability          TEXT NOT NULL CHECK (capability IN ('REGISTER_PARTY','REGISTER_FACILITY','REGISTER_LOT','ADMIT_PROCESSING','FINALIZE_PROCESSING','CREATE_SHIPMENT','ACCEPT_SHIPMENT','OPEN_RECALL','CLOSE_RECALL','READ_AUDIT')),
    status              TEXT NOT NULL CHECK (status IN ('ACTIVE','SUSPENDED','REVOKED')),
    granted_by_party_id BYTEA REFERENCES v2_parties(party_id),
    valid_from          BIGINT NOT NULL CHECK (valid_from >= 0),
    valid_until         BIGINT NOT NULL CHECK (valid_until > valid_from),
    reason              TEXT CHECK (reason IS NULL OR length(reason) <= 1000),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX v2_authority_active_unique_idx
    ON v2_authority_grants(deployment_id, party_id, facility_id, capability)
    WHERE status = 'ACTIVE';
CREATE INDEX v2_authority_party_idx
    ON v2_authority_grants(deployment_id, party_id, status, capability);
CREATE INDEX v2_authority_facility_idx
    ON v2_authority_grants(deployment_id, facility_id, status, capability);

CREATE OR REPLACE FUNCTION prevent_v2_grant_identity_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.grant_id <> OLD.grant_id
       OR NEW.deployment_id <> OLD.deployment_id
       OR NEW.party_id <> OLD.party_id
       OR NEW.facility_id IS DISTINCT FROM OLD.facility_id
       OR NEW.capability <> OLD.capability
       OR NEW.valid_from <> OLD.valid_from
       OR NEW.valid_until <> OLD.valid_until THEN
        RAISE EXCEPTION 'v2 authority grant identity is immutable';
    END IF;
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

CREATE TRIGGER v2_grant_identity_immutable
BEFORE UPDATE ON v2_authority_grants
FOR EACH ROW EXECUTE FUNCTION prevent_v2_grant_identity_mutation();

COMMIT;
