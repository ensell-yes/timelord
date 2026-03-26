CREATE TABLE analytics_snapshots (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    snapshot_date   DATE NOT NULL,
    health_score    INTEGER NOT NULL CHECK (health_score BETWEEN 0 AND 100),
    metrics         JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (org_id, user_id, snapshot_date)
);

CREATE INDEX idx_analytics_snapshots_user ON analytics_snapshots (org_id, user_id, snapshot_date DESC);

ALTER TABLE analytics_snapshots ENABLE ROW LEVEL SECURITY;
CREATE POLICY analytics_snapshots_isolation ON analytics_snapshots
    USING (org_id = NULLIF(current_setting('app.current_org_id', true), '')::uuid);
