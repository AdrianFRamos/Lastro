BEGIN;

CREATE TABLE v2_parties (
    party_id            BYTEA PRIMARY KEY CHECK (octet_length(party_id) = 32),
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    legal_name          TEXT NOT NULL CHECK (length(legal_name) BETWEEN 2 AND 200),
    tax_id_hash         BYTEA CHECK (tax_id_hash IS NULL OR octet_length(tax_id_hash) = 32),
    wallet              BYTEA NOT NULL CHECK (octet_length(wallet) = 32),
    role                SMALLINT NOT NULL CHECK (role BETWEEN 1 AND 11),
    status              TEXT NOT NULL CHECK (status IN ('ACTIVE','SUSPENDED','REVOKED')),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX v2_parties_deployment_tax_idx
    ON v2_parties(deployment_id, tax_id_hash)
    WHERE tax_id_hash IS NOT NULL;
CREATE INDEX v2_parties_deployment_status_idx
    ON v2_parties(deployment_id, status, legal_name);

CREATE TABLE v2_facilities (
    facility_id         BYTEA PRIMARY KEY CHECK (octet_length(facility_id) = 32),
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    owner_party_id      BYTEA NOT NULL REFERENCES v2_parties(party_id),
    facility_type       SMALLINT NOT NULL CHECK (facility_type BETWEEN 1 AND 7),
    display_name        TEXT NOT NULL CHECK (length(display_name) BETWEEN 2 AND 200),
    credential_hash     BYTEA NOT NULL CHECK (octet_length(credential_hash) = 32),
    valid_from          BIGINT NOT NULL CHECK (valid_from >= 0),
    valid_until         BIGINT NOT NULL CHECK (valid_until > valid_from),
    status              TEXT NOT NULL CHECK (status IN ('ACTIVE','SUSPENDED','REVOKED','EXPIRED')),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX v2_facilities_deployment_status_idx
    ON v2_facilities(deployment_id, status, display_name);
CREATE INDEX v2_facilities_owner_idx
    ON v2_facilities(deployment_id, owner_party_id);

CREATE TABLE v2_facility_parties (
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    facility_id         BYTEA NOT NULL REFERENCES v2_facilities(facility_id),
    party_id            BYTEA NOT NULL REFERENCES v2_parties(party_id),
    relationship        TEXT NOT NULL CHECK (relationship IN ('OWNER','OPERATOR','AUDITOR','CUSTODIAN')),
    status              TEXT NOT NULL CHECK (status IN ('ACTIVE','SUSPENDED','REVOKED')),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (deployment_id, facility_id, party_id, relationship)
);

CREATE INDEX v2_facility_parties_party_idx
    ON v2_facility_parties(deployment_id, party_id, status);

CREATE TABLE v2_lots (
    lot_id              BYTEA PRIMARY KEY CHECK (octet_length(lot_id) = 32),
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    facility_id         BYTEA NOT NULL REFERENCES v2_facilities(facility_id),
    owner_party_id      BYTEA NOT NULL REFERENCES v2_parties(party_id),
    external_reference  TEXT CHECK (external_reference IS NULL OR length(external_reference) BETWEEN 1 AND 120),
    head_count          INTEGER NOT NULL CHECK (head_count > 0),
    live_weight_grams   BIGINT NOT NULL CHECK (live_weight_grams > 0),
    status              TEXT NOT NULL CHECK (status IN ('ACTIVE','IN_TRANSIT','QUALITY_HOLD','CLOSED','RECALLED')),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX v2_lots_external_reference_idx
    ON v2_lots(deployment_id, external_reference)
    WHERE external_reference IS NOT NULL;
CREATE INDEX v2_lots_facility_status_idx
    ON v2_lots(deployment_id, facility_id, status, created_at);
CREATE INDEX v2_lots_owner_idx
    ON v2_lots(deployment_id, owner_party_id, created_at);

CREATE TABLE v2_lot_assets (
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    lot_id              BYTEA NOT NULL REFERENCES v2_lots(lot_id),
    asset_id            BYTEA NOT NULL REFERENCES v2_assets(asset_id),
    quantity            BIGINT NOT NULL CHECK (quantity > 0),
    weight_grams        BIGINT NOT NULL CHECK (weight_grams > 0),
    role                TEXT NOT NULL CHECK (role IN ('ANIMAL','CARCASS','PRODUCT','BYPRODUCT')),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (deployment_id, lot_id, asset_id)
);

CREATE INDEX v2_lot_assets_asset_idx
    ON v2_lot_assets(deployment_id, asset_id, lot_id);

CREATE TABLE v2_custody_transfers (
    transfer_id         UUID PRIMARY KEY,
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    asset_id            BYTEA NOT NULL REFERENCES v2_assets(asset_id),
    from_party_id       BYTEA REFERENCES v2_parties(party_id),
    to_party_id         BYTEA NOT NULL REFERENCES v2_parties(party_id),
    from_facility_id    BYTEA REFERENCES v2_facilities(facility_id),
    to_facility_id      BYTEA NOT NULL REFERENCES v2_facilities(facility_id),
    reason              TEXT NOT NULL CHECK (length(reason) BETWEEN 2 AND 500),
    status              TEXT NOT NULL CHECK (status IN ('PROPOSED','ACCEPTED','REJECTED','CANCELLED')),
    event_id            BYTEA CHECK (event_id IS NULL OR octet_length(event_id) = 32),
    tx_signature        TEXT UNIQUE,
    created_by_party_id BYTEA REFERENCES v2_parties(party_id),
    accepted_by_party_id BYTEA REFERENCES v2_parties(party_id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    accepted_at         TIMESTAMPTZ
);

CREATE INDEX v2_custody_asset_idx
    ON v2_custody_transfers(deployment_id, asset_id, created_at);
CREATE INDEX v2_custody_recipient_idx
    ON v2_custody_transfers(deployment_id, to_party_id, status, created_at);

CREATE TABLE v2_audit_log (
    audit_id            BIGSERIAL PRIMARY KEY,
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    actor_party_id      BYTEA REFERENCES v2_parties(party_id),
    actor_wallet        BYTEA CHECK (actor_wallet IS NULL OR octet_length(actor_wallet) = 32),
    action              TEXT NOT NULL CHECK (length(action) BETWEEN 2 AND 120),
    resource_type       TEXT NOT NULL CHECK (length(resource_type) BETWEEN 2 AND 80),
    resource_id         BYTEA CHECK (resource_id IS NULL OR octet_length(resource_id) = 32),
    payload             JSONB NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX v2_audit_resource_idx
    ON v2_audit_log(deployment_id, resource_type, resource_id, created_at);
CREATE INDEX v2_audit_actor_idx
    ON v2_audit_log(deployment_id, actor_party_id, created_at);

CREATE OR REPLACE FUNCTION prevent_v2_audit_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'v2 audit log is append-only';
END;
$$;

CREATE TRIGGER v2_audit_no_update_delete
BEFORE UPDATE OR DELETE ON v2_audit_log
FOR EACH ROW EXECUTE FUNCTION prevent_v2_audit_mutation();

CREATE OR REPLACE FUNCTION prevent_v2_party_identity_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.party_id <> OLD.party_id
       OR NEW.deployment_id <> OLD.deployment_id
       OR NEW.wallet <> OLD.wallet
       OR NEW.role <> OLD.role THEN
        RAISE EXCEPTION 'v2 party identity is immutable';
    END IF;
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

CREATE TRIGGER v2_party_identity_immutable
BEFORE UPDATE ON v2_parties
FOR EACH ROW EXECUTE FUNCTION prevent_v2_party_identity_mutation();

CREATE OR REPLACE FUNCTION prevent_v2_facility_identity_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.facility_id <> OLD.facility_id
       OR NEW.deployment_id <> OLD.deployment_id
       OR NEW.owner_party_id <> OLD.owner_party_id
       OR NEW.facility_type <> OLD.facility_type
       OR NEW.credential_hash <> OLD.credential_hash THEN
        RAISE EXCEPTION 'v2 facility identity is immutable';
    END IF;
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

CREATE TRIGGER v2_facility_identity_immutable
BEFORE UPDATE ON v2_facilities
FOR EACH ROW EXECUTE FUNCTION prevent_v2_facility_identity_mutation();

COMMIT;
