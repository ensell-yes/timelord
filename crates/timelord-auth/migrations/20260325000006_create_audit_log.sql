-- Audit log: append-only, never updated or deleted from application code.
CREATE TABLE audit_log (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    org_id      UUID NOT NULL,
    user_id     UUID,
    action      TEXT NOT NULL,       -- "create", "update", "delete", "login", "logout", "org_switch"
    entity_type TEXT NOT NULL,       -- "user", "calendar", "event", "session", "org"
    entity_id   UUID,
    metadata    JSONB NOT NULL DEFAULT '{}',
    ip_address  INET,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_log_org_created ON audit_log (org_id, created_at DESC);
CREATE INDEX idx_audit_log_user ON audit_log (user_id, created_at DESC) WHERE user_id IS NOT NULL;
CREATE INDEX idx_audit_log_entity ON audit_log (entity_type, entity_id) WHERE entity_id IS NOT NULL;

-- RLS: queries must supply org_id context
ALTER TABLE audit_log ENABLE ROW LEVEL SECURITY;

CREATE POLICY audit_log_isolation ON audit_log
    USING (org_id = NULLIF(current_setting('app.current_org_id', true), '')::uuid);
