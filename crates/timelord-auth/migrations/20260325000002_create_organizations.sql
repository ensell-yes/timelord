CREATE TABLE organizations (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT NOT NULL,
    -- URL-safe slug, e.g. "personal-abc123" for auto-created personal orgs
    slug        TEXT NOT NULL UNIQUE,
    -- personal = true for auto-created single-user orgs
    is_personal BOOLEAN NOT NULL DEFAULT false,
    settings    JSONB NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_organizations_slug ON organizations (slug);

-- Now add the FK from users to organizations (deferred to avoid circular dependency)
ALTER TABLE users
    ADD CONSTRAINT fk_users_last_active_org
    FOREIGN KEY (last_active_org_id) REFERENCES organizations(id)
    ON DELETE SET NULL;
