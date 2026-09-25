BEGIN;

CREATE TABLE v2_transformations (
    transformation_id        BYTEA PRIMARY KEY CHECK (octet_length(transformation_id) = 32),
    deployment_id            BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    facility_id              BYTEA NOT NULL CHECK (octet_length(facility_id) = 32),
    transformation_type      SMALLINT NOT NULL CHECK (transformation_type > 0),
    input_root               BYTEA NOT NULL CHECK (octet_length(input_root) = 32),
    output_root              BYTEA NOT NULL CHECK (octet_length(output_root) = 32),
    input_count              INTEGER NOT NULL CHECK (input_count > 0),
    output_count             INTEGER NOT NULL CHECK (output_count > 0),
    input_weight_grams       BIGINT NOT NULL CHECK (input_weight_grams > 0),
    output_weight_grams      BIGINT NOT NULL CHECK (output_weight_grams >= 0),
    byproduct_weight_grams   BIGINT NOT NULL CHECK (byproduct_weight_grams >= 0),
    loss_weight_grams        BIGINT NOT NULL CHECK (loss_weight_grams >= 0),
    tolerance_basis_points   INTEGER NOT NULL CHECK (tolerance_basis_points BETWEEN 0 AND 1000),
    manifest_nonce            BIGINT NOT NULL CHECK (manifest_nonce >= 0),
    manifest_hash            BYTEA NOT NULL CHECK (octet_length(manifest_hash) = 32),
    manifest_bytes            BYTEA NOT NULL CHECK (octet_length(manifest_bytes) = 188),
    status                   TEXT NOT NULL CHECK (status IN ('OPEN','FINALIZING','FINALIZED','ABORTED','EXPIRED')),
    sequence                 BIGINT NOT NULL DEFAULT 0 CHECK (sequence >= 0),
    expires_at               BIGINT NOT NULL CHECK (expires_at >= 0),
    tx_signature             TEXT UNIQUE,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX v2_transformations_deployment_idx
    ON v2_transformations(deployment_id, created_at, transformation_id);
CREATE INDEX v2_transformations_facility_idx
    ON v2_transformations(deployment_id, facility_id, created_at);

CREATE TABLE v2_lineage_edges (
    deployment_id       BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    transformation_id   BYTEA NOT NULL CHECK (octet_length(transformation_id) = 32),
    parent_asset_id     BYTEA NOT NULL CHECK (octet_length(parent_asset_id) = 32),
    child_asset_id      BYTEA NOT NULL CHECK (octet_length(child_asset_id) = 32),
    role                SMALLINT NOT NULL CHECK (role BETWEEN 1 AND 4),
    position            INTEGER NOT NULL CHECK (position >= 0),
    quantity            BIGINT NOT NULL CHECK (quantity >= 0),
    weight_grams        BIGINT NOT NULL CHECK (weight_grams >= 0),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (deployment_id, transformation_id, child_asset_id, position),
    FOREIGN KEY (transformation_id) REFERENCES v2_transformations(transformation_id)
);

CREATE INDEX v2_lineage_parent_idx
    ON v2_lineage_edges(deployment_id, parent_asset_id, position, child_asset_id);
CREATE INDEX v2_lineage_child_idx
    ON v2_lineage_edges(deployment_id, child_asset_id, position, parent_asset_id);

CREATE OR REPLACE FUNCTION prevent_v2_transformation_evidence_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.transformation_id <> OLD.transformation_id
       OR NEW.deployment_id <> OLD.deployment_id
       OR NEW.facility_id <> OLD.facility_id
       OR NEW.transformation_type <> OLD.transformation_type
       OR NEW.input_root <> OLD.input_root
       OR NEW.output_root <> OLD.output_root
       OR NEW.input_count <> OLD.input_count
       OR NEW.output_count <> OLD.output_count
       OR NEW.input_weight_grams <> OLD.input_weight_grams
       OR NEW.output_weight_grams <> OLD.output_weight_grams
       OR NEW.byproduct_weight_grams <> OLD.byproduct_weight_grams
       OR NEW.loss_weight_grams <> OLD.loss_weight_grams
       OR NEW.tolerance_basis_points <> OLD.tolerance_basis_points
       OR NEW.manifest_nonce <> OLD.manifest_nonce
       OR NEW.manifest_hash <> OLD.manifest_hash
       OR NEW.manifest_bytes <> OLD.manifest_bytes
       OR NEW.expires_at <> OLD.expires_at THEN
        RAISE EXCEPTION 'v2 transformation evidence is immutable';
    END IF;
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

CREATE TRIGGER v2_transformations_immutable_evidence
BEFORE UPDATE ON v2_transformations
FOR EACH ROW EXECUTE FUNCTION prevent_v2_transformation_evidence_mutation();

CREATE OR REPLACE FUNCTION prevent_v2_lineage_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'v2 lineage edges are append-only';
END;
$$;

CREATE TRIGGER v2_lineage_no_update
BEFORE UPDATE OR DELETE ON v2_lineage_edges
FOR EACH ROW EXECUTE FUNCTION prevent_v2_lineage_mutation();

COMMIT;
