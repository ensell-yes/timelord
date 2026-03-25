CREATE TABLE provider_tokens (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    org_id              UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    provider            TEXT NOT NULL CHECK (provider IN ('google', 'microsoft')),
    -- AES-256-GCM encrypted; nonce stored alongside ciphertext
    access_token_enc    BYTEA NOT NULL,
    refresh_token_enc   BYTEA NOT NULL,
    token_nonce         BYTEA NOT NULL,
    scopes              TEXT[] NOT NULL DEFAULT '{}',
    expires_at          TIMESTAMPTZ NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, provider)
);

CREATE INDEX idx_provider_tokens_user_provider ON provider_tokens (user_id, provider);
CREATE INDEX idx_provider_tokens_expires ON provider_tokens (expires_at);

ALTER TABLE provider_tokens ENABLE ROW LEVEL SECURITY;

CREATE POLICY provider_tokens_isolation ON provider_tokens
    USING (org_id = NULLIF(current_setting('app.current_org_id', true), '')::uuid);
