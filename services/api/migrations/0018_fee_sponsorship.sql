-- SPEC v2.1 S1: platform fee sponsorship.

-- Custody intents now close on acceptance and refund their rent to the account that funded
-- them. The API reads that payer from the open on-chain intent when it prepares acceptance
-- and keeps it here, so the finalized acceptance can still be matched exactly afterwards.
ALTER TABLE v2_custody_transfers
    ADD COLUMN intent_rent_payer BYTEA
        CHECK (intent_rent_payer IS NULL OR octet_length(intent_rent_payer) = 32);

-- Every transaction the sponsor co-signed, with the upper bound of what it could cost.
-- Rolling 24 h sums over this table enforce the per-signer and deployment budgets.
CREATE TABLE v2_sponsor_ledger (
    tx_signature   TEXT PRIMARY KEY CHECK (length(tx_signature) BETWEEN 64 AND 88),
    deployment_id  BYTEA NOT NULL CHECK (octet_length(deployment_id) = 32),
    signer         BYTEA NOT NULL CHECK (octet_length(signer) = 32),
    purpose        TEXT NOT NULL CHECK (purpose IN ('STATION_EVENT','REGISTER_ASSET','CUSTODY_PROPOSE','CUSTODY_ACCEPT')),
    lamports       BIGINT NOT NULL CHECK (lamports > 0),
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX v2_sponsor_ledger_deployment_idx
    ON v2_sponsor_ledger(deployment_id, created_at);
CREATE INDEX v2_sponsor_ledger_signer_idx
    ON v2_sponsor_ledger(deployment_id, signer, created_at);
