CREATE TABLE calendars (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id                  UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id                 UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider                TEXT NOT NULL CHECK (provider IN ('google', 'microsoft')),
    provider_calendar_id    TEXT NOT NULL,
    name                    TEXT NOT NULL,
    color                   TEXT,
    is_primary              BOOLEAN NOT NULL DEFAULT false,
    is_visible              BOOLEAN NOT NULL DEFAULT true,
    sync_enabled            BOOLEAN NOT NULL DEFAULT true,
    timezone                TEXT NOT NULL DEFAULT 'UTC',
    -- Sync display rule: "full_title" or "busy"
    display_mode            TEXT NOT NULL DEFAULT 'busy' CHECK (display_mode IN ('full_title', 'busy')),
    -- Which fields to sync when display_mode = 'full_title'
    sync_attendees          BOOLEAN NOT NULL DEFAULT false,
    sync_description        BOOLEAN NOT NULL DEFAULT false,
    sync_location           BOOLEAN NOT NULL DEFAULT false,
    sync_conference         BOOLEAN NOT NULL DEFAULT false,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (org_id, provider, provider_calendar_id)
);

CREATE INDEX idx_calendars_org_user ON calendars (org_id, user_id);
CREATE INDEX idx_calendars_provider ON calendars (provider, provider_calendar_id);

ALTER TABLE calendars ENABLE ROW LEVEL SECURITY;

CREATE POLICY calendars_isolation ON calendars
    USING (org_id = NULLIF(current_setting('app.current_org_id', true), '')::uuid);
