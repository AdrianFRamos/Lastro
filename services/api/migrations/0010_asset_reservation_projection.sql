BEGIN;

ALTER TABLE v2_assets
    ADD COLUMN reserved_weight_grams BIGINT NOT NULL DEFAULT 0
        CHECK (reserved_weight_grams >= 0),
    ADD COLUMN reserved_by BYTEA NOT NULL DEFAULT decode(repeat('00', 32), 'hex')
        CHECK (octet_length(reserved_by) = 32),
    ADD COLUMN reserved_until BIGINT NOT NULL DEFAULT 0
        CHECK (reserved_until >= 0),
    ADD COLUMN flags INTEGER NOT NULL DEFAULT 0
        CHECK (flags >= 0);

CREATE INDEX v2_assets_reserved_idx
    ON v2_assets(deployment_id, reserved_by)
    WHERE reserved_by <> decode(repeat('00', 32), 'hex');

COMMIT;
