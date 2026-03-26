CREATE TABLE sync_state (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    calendar_id     UUID NOT NULL UNIQUE REFERENCES calendars(id) ON DELETE CASCADE,
    sync_token      TEXT,
    last_synced_at  TIMESTAMPTZ,
    last_error      TEXT,
    event_count     INTEGER NOT NULL DEFAULT 0,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- UNIQUE (calendar_id) already creates a unique index; only need org index.
CREATE INDEX idx_sync_state_org ON sync_state (org_id);

ALTER TABLE sync_state ENABLE ROW LEVEL SECURITY;

CREATE POLICY sync_state_isolation ON sync_state
    USING (org_id = NULLIF(current_setting('app.current_org_id', true), '')::uuid);

-- SECURITY DEFINER function for the sync worker to list all sync-enabled
-- calendars across orgs. Hardened with explicit search_path and qualified names.
CREATE OR REPLACE FUNCTION list_sync_work_items()
RETURNS TABLE (
    calendar_id     UUID,
    org_id          UUID,
    user_id         UUID,
    provider        TEXT,
    provider_calendar_id TEXT,
    sync_token      TEXT
) LANGUAGE sql SECURITY DEFINER STABLE
  SET search_path = public
AS $$
    SELECT c.id, c.org_id, c.user_id, c.provider, c.provider_calendar_id, s.sync_token
    FROM public.calendars c
    LEFT JOIN public.sync_state s ON s.calendar_id = c.id
    WHERE c.sync_enabled = true
$$;

REVOKE ALL ON FUNCTION list_sync_work_items() FROM PUBLIC;
-- In production, grant to a dedicated sync role:
--   GRANT EXECUTE ON FUNCTION list_sync_work_items() TO timelord_sync;
-- For dev (single shared DB user), the table owner already has EXECUTE.
