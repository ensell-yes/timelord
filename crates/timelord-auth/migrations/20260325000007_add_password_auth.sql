-- Add password hash column for local auth (nullable — OAuth users don't have one)
ALTER TABLE users ADD COLUMN password_hash TEXT;

-- Instance-level admin flag (separate from per-org roles)
ALTER TABLE users ADD COLUMN system_admin BOOLEAN NOT NULL DEFAULT false;

-- Expand provider CHECK constraint to include 'local'
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_provider_check;
ALTER TABLE users ADD CONSTRAINT users_provider_check CHECK (provider IN ('google', 'microsoft', 'local'));

-- System settings for first-run detection and runtime config
CREATE TABLE system_settings (
    key         TEXT PRIMARY KEY,
    value       JSONB NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
