use chrono::{DateTime, NaiveDate, Utc};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use timelord_common::error::AppError;

use super::{SyncResult, UpsertEvent};

const GOOGLE_CALENDAR_EVENTS_URL: &str =
    "https://www.googleapis.com/calendar/v3/calendars";

// ---------------------------------------------------------------------------
// Google Calendar API response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventsListResponse {
    items: Option<Vec<GoogleEvent>>,
    next_page_token: Option<String>,
    next_sync_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleEvent {
    id: Option<String>,
    etag: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    location: Option<String>,
    conference_data: Option<Value>,
    start: Option<GoogleDateTime>,
    end: Option<GoogleDateTime>,
    status: Option<String>,
    visibility: Option<String>,
    organizer: Option<GoogleOrganizer>,
    attendees: Option<Vec<GoogleAttendee>>,
    recurrence: Option<Vec<String>>,
    recurring_event_id: Option<String>,
    reminders: Option<Value>,
    extended_properties: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleDateTime {
    date_time: Option<String>,
    date: Option<String>,
    time_zone: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleOrganizer {
    email: Option<String>,
    #[serde(rename = "self")]
    is_self: Option<bool>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleAttendee {
    #[serde(rename = "self")]
    is_self: Option<bool>,
    response_status: Option<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Fetch events from Google Calendar, handling pagination.
///
/// If `sync_token` is provided an incremental sync is performed.
/// Returns `AppError::Internal` with message containing "SYNC_TOKEN_INVALID"
/// when the server returns 410 Gone, signalling the caller should clear the
/// token and perform a full re-sync.
pub async fn fetch_google_events(
    http: &Client,
    access_token: &str,
    calendar_id: &str,
    sync_token: Option<&str>,
) -> Result<SyncResult, AppError> {
    let base_url = format!(
        "{GOOGLE_CALENDAR_EVENTS_URL}/{calendar_id}/events"
    );

    let mut all_events: Vec<UpsertEvent> = Vec::new();
    let mut page_token: Option<String> = None;
    let mut final_sync_token: Option<String> = None;

    loop {
        let mut request = http
            .get(&base_url)
            .bearer_auth(access_token)
            .query(&[("maxResults", "2500"), ("singleEvents", "true")]);

        if let Some(token) = sync_token {
            // Only set syncToken on the first page; subsequent pages use pageToken.
            if page_token.is_none() {
                request = request.query(&[("syncToken", token)]);
            }
        }

        if let Some(ref pt) = page_token {
            request = request.query(&[("pageToken", pt.as_str())]);
        }

        let resp = request
            .send()
            .await
            .map_err(|e| AppError::internal(format!("Google events request: {e}")))?;

        let status = resp.status();

        if status == reqwest::StatusCode::GONE {
            return Err(AppError::internal(
                "SYNC_TOKEN_INVALID: Google returned 410 Gone — sync token expired",
            ));
        }

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::internal(format!(
                "Google events API error ({status}): {body}"
            )));
        }

        let body: EventsListResponse = resp
            .json()
            .await
            .map_err(|e| AppError::internal(format!("Google events parse: {e}")))?;

        if let Some(items) = body.items {
            for item in items {
                match convert_google_event(item) {
                    Ok(ev) => all_events.push(ev),
                    Err(e) => {
                        tracing::warn!(error = %e, "skipping unparseable Google event");
                    }
                }
            }
        }

        if let Some(nst) = body.next_sync_token {
            final_sync_token = Some(nst);
        }

        match body.next_page_token {
            Some(npt) => page_token = Some(npt),
            None => break,
        }
    }

    Ok(SyncResult {
        events: all_events,
        next_sync_token: final_sync_token,
    })
}

// ---------------------------------------------------------------------------
// Mapping helpers
// ---------------------------------------------------------------------------

fn convert_google_event(ev: GoogleEvent) -> Result<UpsertEvent, AppError> {
    let provider_event_id = ev
        .id
        .ok_or_else(|| AppError::internal("Google event missing id"))?;

    let start = ev
        .start
        .ok_or_else(|| AppError::internal("Google event missing start"))?;
    let end = ev
        .end
        .ok_or_else(|| AppError::internal("Google event missing end"))?;

    let (start_at, all_day) = parse_google_datetime(&start)?;
    let (end_at, _) = parse_google_datetime(&end)?;

    let timezone = start
        .time_zone
        .unwrap_or_else(|| "UTC".to_string());

    let visibility = match ev.visibility.as_deref() {
        Some("private") => "private".to_string(),
        Some("confidential") => "confidential".to_string(),
        // Google uses "default" to mean public visibility
        Some("public") | Some("default") | None => "public".to_string(),
        Some(other) => other.to_string(),
    };

    let is_organizer = ev
        .organizer
        .as_ref()
        .and_then(|o| o.is_self)
        .unwrap_or(false);

    let organizer_email = ev.organizer.as_ref().and_then(|o| o.email.clone());

    let self_rsvp_status = ev
        .attendees
        .as_ref()
        .and_then(|attendees| {
            attendees
                .iter()
                .find(|a| a.is_self == Some(true))
                .and_then(|a| a.response_status.clone())
        })
        .unwrap_or_else(|| "needs_action".to_string());

    // Normalise Google's responseStatus values to our enum
    let self_rsvp_status = normalise_google_rsvp(&self_rsvp_status);

    let attendees = ev
        .attendees
        .map(|a| serde_json::to_value(a).unwrap_or(Value::Array(vec![])))
        .unwrap_or(Value::Array(vec![]));

    let recurrence_rule = ev.recurrence.and_then(|r| r.into_iter().next());

    let is_recurring_instance = ev.recurring_event_id.is_some();

    Ok(UpsertEvent {
        provider_event_id,
        provider_etag: ev.etag,
        title: ev.summary.unwrap_or_else(|| "(No title)".to_string()),
        description: ev.description,
        location: ev.location,
        conference_data: ev.conference_data,
        start_at,
        end_at,
        all_day,
        timezone,
        status: ev.status.unwrap_or_else(|| "confirmed".to_string()),
        visibility,
        is_organizer,
        organizer_email,
        self_rsvp_status,
        attendees,
        recurrence_rule,
        recurring_event_id: ev.recurring_event_id,
        is_recurring_instance,
        reminders: ev.reminders.unwrap_or(Value::Object(Default::default())),
        extended_properties: ev
            .extended_properties
            .unwrap_or(Value::Object(Default::default())),
    })
}

/// Parse a Google `start`/`end` object into a UTC `DateTime` and an `all_day` flag.
fn parse_google_datetime(dt: &GoogleDateTime) -> Result<(DateTime<Utc>, bool), AppError> {
    if let Some(ref date_time) = dt.date_time {
        // RFC 3339 datetime — Google always includes the offset
        let parsed = DateTime::parse_from_rfc3339(date_time)
            .map_err(|e| {
                AppError::internal(format!("Google datetime parse error: {e} (input: {date_time})"))
            })?
            .with_timezone(&Utc);
        Ok((parsed, false))
    } else if let Some(ref date) = dt.date {
        // All-day event — date only (YYYY-MM-DD)
        let naive = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|e| {
                AppError::internal(format!("Google date parse error: {e} (input: {date})"))
            })?;
        let datetime = naive
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| AppError::internal("Failed to build midnight datetime"))?;
        Ok((datetime.and_utc(), true))
    } else {
        Err(AppError::internal(
            "Google event has neither dateTime nor date",
        ))
    }
}

fn normalise_google_rsvp(status: &str) -> String {
    match status {
        "accepted" => "accepted",
        "declined" => "declined",
        "tentative" => "tentative",
        _ => "needs_action",
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rfc3339_datetime() {
        let dt = GoogleDateTime {
            date_time: Some("2024-06-15T10:30:00-07:00".to_string()),
            date: None,
            time_zone: Some("America/Los_Angeles".to_string()),
        };
        let (parsed, all_day) = parse_google_datetime(&dt).unwrap();
        assert!(!all_day);
        assert_eq!(parsed.to_rfc3339(), "2024-06-15T17:30:00+00:00");
    }

    #[test]
    fn test_parse_all_day_date() {
        let dt = GoogleDateTime {
            date_time: None,
            date: Some("2024-06-15".to_string()),
            time_zone: None,
        };
        let (parsed, all_day) = parse_google_datetime(&dt).unwrap();
        assert!(all_day);
        assert_eq!(parsed.to_rfc3339(), "2024-06-15T00:00:00+00:00");
    }

    #[test]
    fn test_visibility_mapping() {
        assert_eq!(
            convert_test_event(Some("default".to_string())).visibility,
            "public"
        );
        assert_eq!(
            convert_test_event(Some("private".to_string())).visibility,
            "private"
        );
        assert_eq!(
            convert_test_event(None).visibility,
            "public"
        );
    }

    fn convert_test_event(visibility: Option<String>) -> UpsertEvent {
        let ev = GoogleEvent {
            id: Some("test-id".to_string()),
            etag: None,
            summary: Some("Test".to_string()),
            description: None,
            location: None,
            conference_data: None,
            start: Some(GoogleDateTime {
                date_time: Some("2024-01-01T00:00:00Z".to_string()),
                date: None,
                time_zone: None,
            }),
            end: Some(GoogleDateTime {
                date_time: Some("2024-01-01T01:00:00Z".to_string()),
                date: None,
                time_zone: None,
            }),
            status: Some("confirmed".to_string()),
            visibility,
            organizer: None,
            attendees: None,
            recurrence: None,
            recurring_event_id: None,
            reminders: None,
            extended_properties: None,
        };
        convert_google_event(ev).unwrap()
    }

    #[test]
    fn test_rsvp_normalisation() {
        assert_eq!(normalise_google_rsvp("needsAction"), "needs_action");
        assert_eq!(normalise_google_rsvp("accepted"), "accepted");
        assert_eq!(normalise_google_rsvp("declined"), "declined");
        assert_eq!(normalise_google_rsvp("tentative"), "tentative");
    }
}
