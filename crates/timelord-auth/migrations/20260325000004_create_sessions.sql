CREATE TABLE sessions (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    org_id              UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    -- SHA-256 hash of the access token (never store raw tokens)
    token_hash          TEXT NOT NULL UNIQUE,
    -- SHA-256 hash of the refresh token
    refresh_hash        TEXT NOT NULL UNIQUE,
    user_agent          TEXT,
    ip_address          TEXT,
    expires_at          TIMESTAMPTZ NOT NULL,
    refresh_expires_at  TIMESTAMPTZ NOT NULL,
    revoked_at          TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_sessions_user_id ON sessions (user_id);
CREATE INDEX idx_sessions_token_hash ON sessions (token_hash) WHERE revoked_at IS NULL;
CREATE INDEX idx_sessions_refresh_hash ON sessions (refresh_hash) WHERE revoked_at IS NULL;
CREATE INDEX idx_sessions_expires ON sessions (expires_at) WHERE revoked_at IS NULL;
