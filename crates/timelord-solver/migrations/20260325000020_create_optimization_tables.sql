CREATE TABLE optimization_runs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status          TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'completed', 'failed')),
    window_start    TIMESTAMPTZ NOT NULL,
    window_end      TIMESTAMPTZ NOT NULL,
    config          JSONB NOT NULL DEFAULT '{}',
    metrics         JSONB,
    error           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at    TIMESTAMPTZ
);

CREATE INDEX idx_optimization_runs_org_user ON optimization_runs (org_id, user_id);

ALTER TABLE optimization_runs ENABLE ROW LEVEL SECURITY;
CREATE POLICY optimization_runs_isolation ON optimization_runs
    USING (org_id = NULLIF(current_setting('app.current_org_id', true), '')::uuid);

CREATE TABLE optimization_suggestions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id          UUID NOT NULL REFERENCES optimization_runs(id) ON DELETE CASCADE,
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    event_id        UUID NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    original_start  TIMESTAMPTZ NOT NULL,
    original_end    TIMESTAMPTZ NOT NULL,
    suggested_start TIMESTAMPTZ NOT NULL,
    suggested_end   TIMESTAMPTZ NOT NULL,
    reason          TEXT,
    applied         BOOLEAN NOT NULL DEFAULT false,
    applied_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_optimization_suggestions_run ON optimization_suggestions (run_id);
CREATE INDEX idx_optimization_suggestions_event ON optimization_suggestions (event_id);

ALTER TABLE optimization_suggestions ENABLE ROW LEVEL SECURITY;
CREATE POLICY optimization_suggestions_isolation ON optimization_suggestions
    USING (org_id = NULLIF(current_setting('app.current_org_id', true), '')::uuid);
