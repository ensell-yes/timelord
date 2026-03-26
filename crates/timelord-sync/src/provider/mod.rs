pub mod google;
pub mod microsoft;

use chrono::{DateTime, Utc};
use serde_json::Value;

/// Represents an event to upsert from a provider sync.
pub struct UpsertEvent {
    pub provider_event_id: String,
    pub provider_etag: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub conference_data: Option<Value>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub all_day: bool,
    pub timezone: String,
    pub status: String,
    pub visibility: String,
    pub is_organizer: bool,
    pub organizer_email: Option<String>,
    pub self_rsvp_status: String,
    pub attendees: Value,
    pub recurrence_rule: Option<String>,
    pub recurring_event_id: Option<String>,
    pub is_recurring_instance: bool,
    pub reminders: Value,
    pub extended_properties: Value,
}

pub struct SyncResult {
    pub events: Vec<UpsertEvent>,
    pub next_sync_token: Option<String>,
}
