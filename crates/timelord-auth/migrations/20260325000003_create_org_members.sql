CREATE TYPE org_role AS ENUM ('owner', 'admin', 'member');

CREATE TABLE org_members (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id      UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id     UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        org_role NOT NULL DEFAULT 'member',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (org_id, user_id)
);

CREATE INDEX idx_org_members_org_id ON org_members (org_id);
CREATE INDEX idx_org_members_user_id ON org_members (user_id);

ALTER TABLE org_members ENABLE ROW LEVEL SECURITY;

-- Org members are visible only within the same org
CREATE POLICY org_members_isolation ON org_members
    USING (org_id = NULLIF(current_setting('app.current_org_id', true), '')::uuid);
