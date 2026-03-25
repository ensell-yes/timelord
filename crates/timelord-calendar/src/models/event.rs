use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::Type, Serialize, Deserialize, PartialEq)]
#[sqlx(type_name = "event_status", rename_all = "lowercase")]
pub enum EventStatus {
    Confirmed,
    Tentative,
    Cancelled,
}

#[derive(Debug, Clone, sqlx::Type, Serialize, Deserialize, PartialEq)]
#[sqlx(type_name = "event_visibility", rename_all = "lowercase")]
pub enum EventVisibility {
    Public,
    Private,
    Confidential,
}

#[derive(Debug, Clone, sqlx::Type, Serialize, Deserialize, PartialEq)]
#[sqlx(type_name = "rsvp_status", rename_all = "snake_case")]
pub enum RsvpStatus {
    Accepted,
    Declined,
    Tentative,
    NeedsAction,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Event {
    pub id: Uuid,
    pub org_id: Uuid,
    pub calendar_id: Uuid,
    pub provider_event_id: Option<String>,
    pub provider_etag: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub conference_data: Option<Value>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub all_day: bool,
    pub timezone: String,
    pub status: EventStatus,
    pub visibility: EventVisibility,
    pub is_organizer: bool,
    pub organizer_email: Option<String>,
    pub self_rsvp_status: RsvpStatus,
    pub attendees: Value,
    pub recurrence_rule: Option<String>,
    pub recurring_event_id: Option<String>,
    pub is_recurring_instance: bool,
    pub reminders: Value,
    pub extended_properties: Value,
    pub is_movable: bool,
    pub is_heads_down: bool,
    pub provider_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEventRequest {
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub all_day: Option<bool>,
    pub timezone: Option<String>,
    pub visibility: Option<String>,
    pub attendees: Option<Value>,
    pub recurrence_rule: Option<String>,
    pub is_movable: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UpdateEventRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub timezone: Option<String>,
    pub visibility: Option<String>,
    pub is_movable: Option<bool>,
    pub is_heads_down: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ListEventsQuery {
    pub time_min: Option<DateTime<Utc>>,
    pub time_max: Option<DateTime<Utc>>,
    pub page_size: Option<i64>,
    pub page_token: Option<String>,
}
