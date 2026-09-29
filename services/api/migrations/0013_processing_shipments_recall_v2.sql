BEGIN;

CREATE TABLE v2_processing_operations (
    operation_id        BYTEA PRIMARY KEY CHECK (octet_length(operation_id) = 32),
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    facility_id         BYTEA NOT NULL REFERENCES v2_facilities(facility_id),
    lot_id              BYTEA REFERENCES v2_lots(lot_id),
    transformation_id   BYTEA REFERENCES v2_transformations(transformation_id),
    operator_party_id   BYTEA NOT NULL REFERENCES v2_parties(party_id),
    operation_kind      TEXT NOT NULL CHECK (operation_kind IN ('SLAUGHTER','BUTCHERY','PROCESSING')),
    status               TEXT NOT NULL CHECK (status IN ('REGISTERED','IN_PROGRESS','READY_FOR_CHAIN','SUBMITTED','FINALIZED','REJECTED')),
    notes                TEXT CHECK (notes IS NULL OR length(notes) <= 2000),
    started_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    finalized_at         TIMESTAMPTZ,
    tx_signature         TEXT UNIQUE,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX v2_processing_facility_idx
    ON v2_processing_operations(deployment_id, facility_id, status, created_at);
CREATE INDEX v2_processing_lot_idx
    ON v2_processing_operations(deployment_id, lot_id, created_at);
CREATE INDEX v2_processing_transformation_idx
    ON v2_processing_operations(deployment_id, transformation_id);

CREATE TABLE v2_processing_items (
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    operation_id        BYTEA NOT NULL REFERENCES v2_processing_operations(operation_id),
    position             INTEGER NOT NULL CHECK (position >= 0),
    asset_id             BYTEA REFERENCES v2_assets(asset_id) CHECK (asset_id IS NULL OR octet_length(asset_id) = 32),
    direction            TEXT NOT NULL CHECK (direction IN ('INPUT','OUTPUT','BYPRODUCT','LOSS')),
    quantity             BIGINT NOT NULL CHECK (quantity > 0),
    weight_grams         BIGINT NOT NULL CHECK (weight_grams > 0),
    PRIMARY KEY (deployment_id, operation_id, position)
);

ALTER TABLE v2_processing_items
    ADD CONSTRAINT v2_processing_item_asset_for_non_loss
    CHECK (asset_id IS NOT NULL OR direction = 'LOSS');

CREATE INDEX v2_processing_items_asset_idx
    ON v2_processing_items(deployment_id, asset_id, operation_id);
CREATE INDEX v2_processing_items_direction_idx
    ON v2_processing_items(deployment_id, operation_id, direction, position);

CREATE TABLE v2_shipments (
    shipment_id         BYTEA PRIMARY KEY CHECK (octet_length(shipment_id) = 32),
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    origin_facility_id  BYTEA NOT NULL REFERENCES v2_facilities(facility_id),
    destination_facility_id BYTEA NOT NULL REFERENCES v2_facilities(facility_id),
    carrier_party_id    BYTEA NOT NULL REFERENCES v2_parties(party_id),
    created_by_party_id BYTEA NOT NULL REFERENCES v2_parties(party_id),
    status               TEXT NOT NULL CHECK (status IN ('DRAFT','DISPATCHED','IN_TRANSIT','DELIVERED','REJECTED','RECALLED')),
    planned_departure   TIMESTAMPTZ,
    departed_at         TIMESTAMPTZ,
    delivered_at        TIMESTAMPTZ,
    notes                TEXT CHECK (notes IS NULL OR length(notes) <= 2000),
    tx_signature         TEXT UNIQUE,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX v2_shipments_origin_idx
    ON v2_shipments(deployment_id, origin_facility_id, status, created_at);
CREATE INDEX v2_shipments_destination_idx
    ON v2_shipments(deployment_id, destination_facility_id, status, created_at);

CREATE TABLE v2_shipment_items (
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    shipment_id         BYTEA NOT NULL REFERENCES v2_shipments(shipment_id),
    position             INTEGER NOT NULL CHECK (position >= 0),
    asset_id             BYTEA NOT NULL REFERENCES v2_assets(asset_id),
    quantity             BIGINT NOT NULL CHECK (quantity > 0),
    weight_grams         BIGINT NOT NULL CHECK (weight_grams > 0),
    PRIMARY KEY (deployment_id, shipment_id, position)
);

CREATE INDEX v2_shipment_items_asset_idx
    ON v2_shipment_items(deployment_id, asset_id, shipment_id);

CREATE TABLE v2_recalls (
    recall_id            BYTEA PRIMARY KEY CHECK (octet_length(recall_id) = 32),
    deployment_id        BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    opened_by_party_id   BYTEA NOT NULL REFERENCES v2_parties(party_id),
    scope_type           TEXT NOT NULL CHECK (scope_type IN ('LOT','ANIMAL','TRANSFORMATION','PRODUCT','ASSET')),
    scope_id             BYTEA NOT NULL CHECK (octet_length(scope_id) = 32),
    reason               TEXT NOT NULL CHECK (length(reason) BETWEEN 5 AND 2000),
    status               TEXT NOT NULL CHECK (status IN ('OPEN','CLOSED','CANCELLED')),
    snapshot_root        BYTEA NOT NULL CHECK (octet_length(snapshot_root) = 32),
    opened_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    closed_at            TIMESTAMPTZ
);

CREATE INDEX v2_recalls_scope_idx
    ON v2_recalls(deployment_id, scope_type, scope_id, opened_at);
CREATE INDEX v2_recalls_status_idx
    ON v2_recalls(deployment_id, status, opened_at);

CREATE TABLE v2_recall_members (
    deployment_id        BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    recall_id            BYTEA NOT NULL REFERENCES v2_recalls(recall_id),
    asset_id             BYTEA NOT NULL REFERENCES v2_assets(asset_id),
    traversal_depth      INTEGER NOT NULL CHECK (traversal_depth >= 0),
    relation             TEXT NOT NULL CHECK (relation IN ('ROOT','UPSTREAM','DOWNSTREAM')),
    first_seen_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (deployment_id, recall_id, asset_id)
);

CREATE INDEX v2_recall_members_asset_idx
    ON v2_recall_members(deployment_id, asset_id, recall_id);

CREATE OR REPLACE FUNCTION prevent_v2_processing_item_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'v2 processing items are append-only';
END;
$$;

CREATE TRIGGER v2_processing_items_no_update_delete
BEFORE UPDATE OR DELETE ON v2_processing_items
FOR EACH ROW EXECUTE FUNCTION prevent_v2_processing_item_mutation();

CREATE OR REPLACE FUNCTION prevent_v2_shipment_item_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'v2 shipment items are append-only';
END;
$$;

CREATE TRIGGER v2_shipment_items_no_update_delete
BEFORE UPDATE OR DELETE ON v2_shipment_items
FOR EACH ROW EXECUTE FUNCTION prevent_v2_shipment_item_mutation();

CREATE OR REPLACE FUNCTION prevent_v2_recall_snapshot_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.recall_id <> OLD.recall_id
       OR NEW.deployment_id <> OLD.deployment_id
       OR NEW.scope_type <> OLD.scope_type
       OR NEW.scope_id <> OLD.scope_id
       OR NEW.snapshot_root <> OLD.snapshot_root THEN
        RAISE EXCEPTION 'v2 recall snapshot identity is immutable';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER v2_recall_snapshot_immutable
BEFORE UPDATE ON v2_recalls
FOR EACH ROW EXECUTE FUNCTION prevent_v2_recall_snapshot_mutation();

CREATE OR REPLACE FUNCTION prevent_v2_recall_member_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'v2 recall members are append-only';
END;
$$;

CREATE TRIGGER v2_recall_members_no_update_delete
BEFORE UPDATE OR DELETE ON v2_recall_members
FOR EACH ROW EXECUTE FUNCTION prevent_v2_recall_member_mutation();

COMMIT;
