-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE users (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email               TEXT NOT NULL,
    display_name        TEXT NOT NULL DEFAULT '',
    avatar_url          TEXT,
    provider            TEXT NOT NULL CHECK (provider IN ('google', 'microsoft')),
    provider_sub        TEXT NOT NULL,
    is_active           BOOLEAN NOT NULL DEFAULT true,
    -- Tracks the last org this user was active in; used to mint the default JWT on login/refresh.
    last_active_org_id  UUID,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (provider, provider_sub)
);

CREATE INDEX idx_users_email ON users (email);
CREATE INDEX idx_users_provider ON users (provider, provider_sub);
