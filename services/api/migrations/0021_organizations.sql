-- SPEC v2.1 S2: organization accounts without browser wallets.
-- Each organization owns one custodian wallet whose secret is stored only encrypted
-- (AES-256-GCM under a deployment master key held outside the database).

CREATE TABLE v2_organizations (
    org_id        UUID PRIMARY KEY,
    deployment_id BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    name          TEXT NOT NULL CHECK (length(name) BETWEEN 2 AND 200),
    wallet        BYTEA NOT NULL UNIQUE CHECK (octet_length(wallet) = 32),
    -- 12-byte nonce || ciphertext of the 32-byte Ed25519 secret || 16-byte tag.
    sealed_secret BYTEA NOT NULL CHECK (octet_length(sealed_secret) = 60),
    key_version   SMALLINT NOT NULL CHECK (key_version >= 1),
    exported_at   TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE v2_org_users (
    user_id    UUID PRIMARY KEY,
    org_id     UUID NOT NULL REFERENCES v2_organizations(org_id),
    email      TEXT NOT NULL UNIQUE CHECK (email = lower(email) AND length(email) BETWEEN 3 AND 254),
    role       TEXT NOT NULL CHECK (role IN ('ORG_ADMIN','OPERADOR_CURRAL','AUDITOR','LEITURA')),
    status     TEXT NOT NULL CHECK (status IN ('ACTIVE','DISABLED')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX v2_org_users_org_idx ON v2_org_users(org_id, role);

-- One-time login codes: only a keyed hash is stored; bounded attempts and lifetime.
CREATE TABLE v2_login_codes (
    code_id    UUID PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES v2_org_users(user_id),
    code_hash  BYTEA NOT NULL CHECK (octet_length(code_hash) = 32),
    expires_at TIMESTAMPTZ NOT NULL,
    attempts   SMALLINT NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 5),
    consumed   BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX v2_login_codes_user_idx ON v2_login_codes(user_id, created_at);

-- Bearer sessions: only the token hash is stored.
CREATE TABLE v2_user_tokens (
    token_hash BYTEA PRIMARY KEY CHECK (octet_length(token_hash) = 32),
    user_id    UUID NOT NULL REFERENCES v2_org_users(user_id),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked    BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX v2_user_tokens_user_idx ON v2_user_tokens(user_id);

-- Every transaction an organization key signed, for audit.
CREATE TABLE v2_org_signatures (
    tx_signature TEXT PRIMARY KEY,
    org_id       UUID NOT NULL REFERENCES v2_organizations(org_id),
    user_id      UUID NOT NULL REFERENCES v2_org_users(user_id),
    purpose      TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- An organization's wallet and sealed key never change; only `exported_at` may be set.
CREATE OR REPLACE FUNCTION prevent_v2_org_key_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE'
       OR NEW.org_id <> OLD.org_id
       OR NEW.deployment_id <> OLD.deployment_id
       OR NEW.wallet <> OLD.wallet
       OR NEW.sealed_secret <> OLD.sealed_secret
       OR NEW.key_version <> OLD.key_version
       OR (OLD.exported_at IS NOT NULL AND NEW.exported_at IS DISTINCT FROM OLD.exported_at) THEN
        RAISE EXCEPTION 'v2 organization key is immutable';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER v2_organizations_key_immutable
BEFORE UPDATE OR DELETE ON v2_organizations
FOR EACH ROW EXECUTE FUNCTION prevent_v2_org_key_mutation();

CREATE OR REPLACE FUNCTION prevent_v2_org_signature_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'v2 organization signature log is append-only';
END;
$$;

CREATE TRIGGER v2_org_signatures_append_only
BEFORE UPDATE OR DELETE ON v2_org_signatures
FOR EACH ROW EXECUTE FUNCTION prevent_v2_org_signature_mutation();
