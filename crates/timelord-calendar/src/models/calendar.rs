use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Calendar {
    pub id: Uuid,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub provider_calendar_id: String,
    pub name: String,
    pub color: Option<String>,
    pub is_primary: bool,
    pub is_visible: bool,
    pub sync_enabled: bool,
    pub timezone: String,
    pub display_mode: String,
    pub sync_attendees: bool,
    pub sync_description: bool,
    pub sync_location: bool,
    pub sync_conference: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCalendarRequest {
    pub provider: String,
    pub provider_calendar_id: String,
    pub name: String,
    pub color: Option<String>,
    pub is_primary: Option<bool>,
    pub timezone: Option<String>,
    pub display_mode: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UpdateCalendarRequest {
    pub name: Option<String>,
    pub color: Option<String>,
    pub is_visible: Option<bool>,
    pub sync_enabled: Option<bool>,
    pub display_mode: Option<String>,
    pub sync_attendees: Option<bool>,
    pub sync_description: Option<bool>,
    pub sync_location: Option<bool>,
    pub sync_conference: Option<bool>,
}

/// Provider calendar as returned from Google/Microsoft API.
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ProviderCalendar {
    pub provider_id: String,
    pub name: String,
    pub color: Option<String>,
    pub is_primary: bool,
    pub timezone: Option<String>,
}
