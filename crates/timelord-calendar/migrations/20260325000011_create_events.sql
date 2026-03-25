CREATE TYPE event_status AS ENUM ('confirmed', 'tentative', 'cancelled');
CREATE TYPE event_visibility AS ENUM ('public', 'private', 'confidential');
CREATE TYPE rsvp_status AS ENUM ('accepted', 'declined', 'tentative', 'needs_action');

CREATE TABLE events (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id                  UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    calendar_id             UUID NOT NULL REFERENCES calendars(id) ON DELETE CASCADE,
    -- Provider's event ID; NULL for locally-created events not yet synced
    provider_event_id       TEXT,
    -- ETag from provider for optimistic concurrency on sync
    provider_etag           TEXT,
    title                   TEXT NOT NULL DEFAULT '(No title)',
    description             TEXT,
    location                TEXT,
    -- Conference data: Zoom/Meet/Teams link + metadata (JSONB)
    conference_data         JSONB,
    start_at                TIMESTAMPTZ NOT NULL,
    end_at                  TIMESTAMPTZ NOT NULL,
    all_day                 BOOLEAN NOT NULL DEFAULT false,
    timezone                TEXT NOT NULL DEFAULT 'UTC',
    status                  event_status NOT NULL DEFAULT 'confirmed',
    visibility              event_visibility NOT NULL DEFAULT 'public',
    is_organizer            BOOLEAN NOT NULL DEFAULT false,
    organizer_email         TEXT,
    self_rsvp_status        rsvp_status NOT NULL DEFAULT 'needs_action',
    -- Attendee list: [{email, display_name, rsvp_status, is_optional}]
    attendees               JSONB NOT NULL DEFAULT '[]',
    -- iCal recurrence rule string, e.g. "RRULE:FREQ=WEEKLY;BYDAY=MO"
    recurrence_rule         TEXT,
    -- Provider's ID of the master recurring event
    recurring_event_id      TEXT,
    is_recurring_instance   BOOLEAN NOT NULL DEFAULT false,
    reminders               JSONB NOT NULL DEFAULT '[]',
    extended_properties     JSONB NOT NULL DEFAULT '{}',
    -- Optimization flags
    is_movable              BOOLEAN NOT NULL DEFAULT true,
    is_heads_down           BOOLEAN NOT NULL DEFAULT false,
    -- Last time this event was synced from the provider
    provider_synced_at      TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (calendar_id, provider_event_id)
);

CREATE INDEX idx_events_org_calendar ON events (org_id, calendar_id);
CREATE INDEX idx_events_time_range ON events (org_id, start_at, end_at);
CREATE INDEX idx_events_provider_id ON events (calendar_id, provider_event_id) WHERE provider_event_id IS NOT NULL;
CREATE INDEX idx_events_recurring ON events (recurring_event_id) WHERE recurring_event_id IS NOT NULL;
CREATE INDEX idx_events_movable ON events (org_id, is_movable) WHERE is_movable = true;

ALTER TABLE events ENABLE ROW LEVEL SECURITY;

CREATE POLICY events_isolation ON events
    USING (org_id = NULLIF(current_setting('app.current_org_id', true), '')::uuid);
