-- SPEC v2.1 S5: facility boundaries. The on-chain FacilityRecord holds only geo_commitment;
-- the canonical boundary bytes it commits to live here and travel in the EvidencePackage.

CREATE TABLE v2_facility_boundaries (
    facility_id    BYTEA PRIMARY KEY REFERENCES v2_facilities(facility_id),
    car_code       TEXT NOT NULL CHECK (length(car_code) BETWEEN 1 AND 100),
    -- Canonical encoding (crates/lastro-protocol/src/v2/geo.rs): CAR || 0x00 || rings.
    boundary       BYTEA NOT NULL CHECK (octet_length(boundary) BETWEEN 7 AND 80361),
    geo_commitment BYTEA NOT NULL UNIQUE CHECK (octet_length(geo_commitment) = 32),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- A registered boundary never changes: past evidence keeps the meaning it was anchored with.
CREATE OR REPLACE FUNCTION prevent_v2_facility_boundary_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'facility boundaries are immutable';
END;
$$;

CREATE TRIGGER v2_facility_boundaries_immutable
    BEFORE UPDATE OR DELETE ON v2_facility_boundaries
    FOR EACH ROW EXECUTE FUNCTION prevent_v2_facility_boundary_mutation();
