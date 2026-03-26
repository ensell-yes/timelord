use chrono::{DateTime, NaiveDateTime, Utc};
use chrono_tz::Tz;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use timelord_common::error::AppError;

use super::{SyncResult, UpsertEvent};

const GRAPH_CALENDAR_DELTA_URL: &str =
    "https://graph.microsoft.com/v1.0/me/calendars";

// ---------------------------------------------------------------------------
// Microsoft Graph API response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct DeltaResponse {
    value: Option<Vec<Value>>,
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
    #[serde(rename = "@odata.deltaLink")]
    delta_link: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphEvent {
    id: Option<String>,
    #[serde(rename = "@odata.etag")]
    odata_etag: Option<String>,
    subject: Option<String>,
    body_preview: Option<String>,
    location: Option<GraphLocation>,
    online_meeting: Option<Value>,
    start: Option<GraphDateTimeZone>,
    end: Option<GraphDateTimeZone>,
    is_all_day: Option<bool>,
    show_as: Option<String>,
    is_cancelled: Option<bool>,
    sensitivity: Option<String>,
    is_organizer: Option<bool>,
    organizer: Option<GraphOrganizer>,
    response_status: Option<GraphResponseStatus>,
    attendees: Option<Value>,
    recurrence: Option<Value>,
    series_master_id: Option<String>,
    #[serde(rename = "type")]
    event_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphLocation {
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphDateTimeZone {
    date_time: Option<String>,
    time_zone: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphOrganizer {
    email_address: Option<GraphEmailAddress>,
}

#[derive(Debug, Deserialize)]
struct GraphEmailAddress {
    address: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphResponseStatus {
    response: Option<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Fetch events from Microsoft Graph using the delta endpoint.
///
/// If `delta_link` is provided it is used as the full URL (contains the delta
/// token). Otherwise the base delta URL is constructed from `calendar_id`.
pub async fn fetch_microsoft_events(
    http: &Client,
    access_token: &str,
    calendar_id: &str,
    delta_link: Option<&str>,
) -> Result<SyncResult, AppError> {
    let base_url = format!(
        "{GRAPH_CALENDAR_DELTA_URL}/{calendar_id}/events/delta"
    );

    let mut all_events: Vec<UpsertEvent> = Vec::new();
    let mut next_url: Option<String> = delta_link.map(String::from);
    let mut final_delta_link: Option<String> = None;
    let mut is_first = true;

    loop {
        let url = match &next_url {
            Some(u) => u.clone(),
            None if is_first => base_url.clone(),
            None => break,
        };
        is_first = false;

        let resp = http
            .get(&url)
            .bearer_auth(access_token)
            .header("Prefer", "odata.maxpagesize=200")
            .send()
            .await
            .map_err(|e| AppError::internal(format!("Microsoft delta request: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::internal(format!(
                "Microsoft Graph API error ({status}): {body}"
            )));
        }

        let body: DeltaResponse = resp
            .json()
            .await
            .map_err(|e| AppError::internal(format!("Microsoft delta parse: {e}")))?;

        if let Some(items) = body.value {
            for item in items {
                // Check if this is a removed item
                if item.get("@removed").is_some() {
                    if let Some(ev) = convert_removed_item(&item) {
                        all_events.push(ev);
                    }
                    continue;
                }

                match convert_graph_event(&item) {
                    Ok(ev) => all_events.push(ev),
                    Err(e) => {
                        tracing::warn!(error = %e, "skipping unparseable Microsoft event");
                    }
                }
            }
        }

        if let Some(dl) = body.delta_link {
            final_delta_link = Some(dl);
        }

        next_url = body.next_link;
        if next_url.is_none() {
            break;
        }
    }

    Ok(SyncResult {
        events: all_events,
        next_sync_token: final_delta_link,
    })
}

// ---------------------------------------------------------------------------
// Mapping helpers
// ---------------------------------------------------------------------------

fn convert_removed_item(raw: &Value) -> Option<UpsertEvent> {
    let id = raw.get("id")?.as_str()?.to_string();

    Some(UpsertEvent {
        provider_event_id: id,
        provider_etag: None,
        title: "(Removed)".to_string(),
        description: None,
        location: None,
        conference_data: None,
        start_at: Utc::now(),
        end_at: Utc::now(),
        all_day: false,
        timezone: "UTC".to_string(),
        status: "cancelled".to_string(),
        visibility: "public".to_string(),
        is_organizer: false,
        organizer_email: None,
        self_rsvp_status: "needs_action".to_string(),
        attendees: Value::Array(vec![]),
        recurrence_rule: None,
        recurring_event_id: None,
        is_recurring_instance: false,
        reminders: Value::Object(Default::default()),
        extended_properties: Value::Object(Default::default()),
    })
}

fn convert_graph_event(raw: &Value) -> Result<UpsertEvent, AppError> {
    let ev: GraphEvent = serde_json::from_value(raw.clone())
        .map_err(|e| AppError::internal(format!("Microsoft event deserialize: {e}")))?;

    let provider_event_id = ev
        .id
        .ok_or_else(|| AppError::internal("Microsoft event missing id"))?;

    let start_tz_obj = ev
        .start
        .ok_or_else(|| AppError::internal("Microsoft event missing start"))?;
    let end_tz_obj = ev
        .end
        .ok_or_else(|| AppError::internal("Microsoft event missing end"))?;

    let timezone = start_tz_obj
        .time_zone
        .clone()
        .unwrap_or_else(|| "UTC".to_string());

    let start_at = parse_graph_datetime(&start_tz_obj)?;
    let end_at = parse_graph_datetime(&end_tz_obj)?;

    let all_day = ev.is_all_day.unwrap_or(false);

    let status = if ev.is_cancelled == Some(true) {
        "cancelled".to_string()
    } else {
        match ev.show_as.as_deref() {
            Some("tentative") => "tentative".to_string(),
            _ => "confirmed".to_string(),
        }
    };

    let visibility = match ev.sensitivity.as_deref() {
        Some("private") => "private".to_string(),
        Some("confidential") => "confidential".to_string(),
        // "normal" and anything else → public
        _ => "public".to_string(),
    };

    let is_organizer = ev.is_organizer.unwrap_or(false);

    let organizer_email = ev
        .organizer
        .and_then(|o| o.email_address)
        .and_then(|ea| ea.address);

    let self_rsvp_status = ev
        .response_status
        .and_then(|rs| rs.response)
        .map(|r| normalise_microsoft_rsvp(&r))
        .unwrap_or_else(|| "needs_action".to_string());

    let attendees = ev.attendees.unwrap_or(Value::Array(vec![]));

    let recurrence_rule = ev
        .recurrence
        .map(|r| serde_json::to_string(&r).unwrap_or_default());

    let is_recurring_instance = ev.event_type.as_deref() == Some("occurrence");

    Ok(UpsertEvent {
        provider_event_id,
        provider_etag: ev.odata_etag,
        title: ev.subject.unwrap_or_else(|| "(No title)".to_string()),
        description: ev.body_preview,
        location: ev.location.and_then(|l| l.display_name),
        conference_data: ev.online_meeting,
        start_at,
        end_at,
        all_day,
        timezone,
        status,
        visibility,
        is_organizer,
        organizer_email,
        self_rsvp_status,
        attendees,
        recurrence_rule,
        recurring_event_id: ev.series_master_id,
        is_recurring_instance,
        reminders: Value::Object(Default::default()),
        extended_properties: Value::Object(Default::default()),
    })
}

/// Parse a Microsoft Graph `dateTimeTimeZone` into a UTC `DateTime`.
///
/// Graph returns datetime as e.g. "2024-01-15T09:00:00.0000000" with a
/// separate `timeZone` field such as "Pacific Standard Time" or an IANA zone.
fn parse_graph_datetime(dt: &GraphDateTimeZone) -> Result<DateTime<Utc>, AppError> {
    let raw = dt
        .date_time
        .as_deref()
        .ok_or_else(|| AppError::internal("Microsoft event missing dateTime value"))?;

    let tz_name = dt.time_zone.as_deref().unwrap_or("UTC");

    // Strip sub-second precision beyond what chrono handles (7 fractional digits)
    let normalised = normalise_graph_datetime(raw);

    let naive = NaiveDateTime::parse_from_str(&normalised, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(&normalised, "%Y-%m-%dT%H:%M:%S"))
        .map_err(|e| {
            AppError::internal(format!(
                "Microsoft datetime parse error: {e} (input: {raw})"
            ))
        })?;

    // Try parsing as IANA timezone first, then fall back to Windows timezone mapping
    let tz = resolve_timezone(tz_name)?;
    let local = naive
        .and_local_timezone(tz)
        .earliest()
        .ok_or_else(|| {
            AppError::internal(format!(
                "Ambiguous or invalid local time: {raw} in {tz_name}"
            ))
        })?;

    Ok(local.with_timezone(&Utc))
}

/// Normalise Microsoft's datetime strings by truncating fractional seconds
/// to at most 6 digits (microsecond precision) for chrono compatibility.
fn normalise_graph_datetime(raw: &str) -> String {
    if let Some(dot_pos) = raw.find('.') {
        let frac = &raw[dot_pos + 1..];
        if frac.len() > 6 {
            format!("{}.{}", &raw[..dot_pos], &frac[..6])
        } else {
            raw.to_string()
        }
    } else {
        raw.to_string()
    }
}

/// Resolve a timezone name to a `chrono_tz::Tz`.
///
/// Microsoft Graph sometimes returns Windows timezone names (e.g.
/// "Pacific Standard Time") instead of IANA names. This function handles
/// the most common ones.
fn resolve_timezone(name: &str) -> Result<Tz, AppError> {
    // Try IANA name first
    if let Ok(tz) = name.parse::<Tz>() {
        return Ok(tz);
    }

    // Map common Windows timezone IDs to IANA
    let iana = match name {
        "UTC" => "UTC",
        "Dateline Standard Time" => "Etc/GMT+12",
        "Samoa Standard Time" => "Pacific/Apia",
        "Hawaiian Standard Time" => "Pacific/Honolulu",
        "Alaskan Standard Time" => "America/Anchorage",
        "Pacific Standard Time" => "America/Los_Angeles",
        "Mountain Standard Time" => "America/Denver",
        "US Mountain Standard Time" => "America/Phoenix",
        "Central Standard Time" => "America/Chicago",
        "Canada Central Standard Time" => "America/Regina",
        "Central America Standard Time" => "America/Guatemala",
        "Eastern Standard Time" => "America/New_York",
        "US Eastern Standard Time" => "America/Indianapolis",
        "SA Pacific Standard Time" => "America/Bogota",
        "Atlantic Standard Time" => "America/Halifax",
        "SA Western Standard Time" => "America/La_Paz",
        "Pacific SA Standard Time" => "America/Santiago",
        "Newfoundland Standard Time" => "America/St_Johns",
        "E. South America Standard Time" => "America/Sao_Paulo",
        "SA Eastern Standard Time" => "America/Cayenne",
        "Greenland Standard Time" => "America/Godthab",
        "Mid-Atlantic Standard Time" => "Atlantic/South_Georgia",
        "Azores Standard Time" => "Atlantic/Azores",
        "Cape Verde Standard Time" => "Atlantic/Cape_Verde",
        "GMT Standard Time" => "Europe/London",
        "Greenwich Standard Time" => "Atlantic/Reykjavik",
        "W. Europe Standard Time" => "Europe/Berlin",
        "Central Europe Standard Time" => "Europe/Budapest",
        "Romance Standard Time" => "Europe/Paris",
        "Central European Standard Time" => "Europe/Warsaw",
        "W. Central Africa Standard Time" => "Africa/Lagos",
        "Jordan Standard Time" => "Asia/Amman",
        "GTB Standard Time" => "Europe/Bucharest",
        "Middle East Standard Time" => "Asia/Beirut",
        "Egypt Standard Time" => "Africa/Cairo",
        "South Africa Standard Time" => "Africa/Johannesburg",
        "FLE Standard Time" => "Europe/Kiev",
        "Israel Standard Time" => "Asia/Jerusalem",
        "E. Europe Standard Time" => "Europe/Chisinau",
        "Arabic Standard Time" => "Asia/Baghdad",
        "Arab Standard Time" => "Asia/Riyadh",
        "Russian Standard Time" => "Europe/Moscow",
        "E. Africa Standard Time" => "Africa/Nairobi",
        "Iran Standard Time" => "Asia/Tehran",
        "Arabian Standard Time" => "Asia/Dubai",
        "Azerbaijan Standard Time" => "Asia/Baku",
        "Mauritius Standard Time" => "Indian/Mauritius",
        "Georgian Standard Time" => "Asia/Tbilisi",
        "Caucasus Standard Time" => "Asia/Yerevan",
        "Afghanistan Standard Time" => "Asia/Kabul",
        "West Asia Standard Time" => "Asia/Tashkent",
        "Ekaterinburg Standard Time" => "Asia/Yekaterinburg",
        "Pakistan Standard Time" => "Asia/Karachi",
        "India Standard Time" => "Asia/Kolkata",
        "Sri Lanka Standard Time" => "Asia/Colombo",
        "Nepal Standard Time" => "Asia/Kathmandu",
        "Central Asia Standard Time" => "Asia/Almaty",
        "Bangladesh Standard Time" => "Asia/Dhaka",
        "N. Central Asia Standard Time" => "Asia/Novosibirsk",
        "Myanmar Standard Time" => "Asia/Rangoon",
        "SE Asia Standard Time" => "Asia/Bangkok",
        "North Asia Standard Time" => "Asia/Krasnoyarsk",
        "China Standard Time" => "Asia/Shanghai",
        "North Asia East Standard Time" => "Asia/Irkutsk",
        "Singapore Standard Time" => "Asia/Singapore",
        "W. Australia Standard Time" => "Australia/Perth",
        "Taipei Standard Time" => "Asia/Taipei",
        "Tokyo Standard Time" => "Asia/Tokyo",
        "Korea Standard Time" => "Asia/Seoul",
        "Yakutsk Standard Time" => "Asia/Yakutsk",
        "Cen. Australia Standard Time" => "Australia/Adelaide",
        "AUS Central Standard Time" => "Australia/Darwin",
        "E. Australia Standard Time" => "Australia/Brisbane",
        "AUS Eastern Standard Time" => "Australia/Sydney",
        "West Pacific Standard Time" => "Pacific/Port_Moresby",
        "Tasmania Standard Time" => "Australia/Hobart",
        "Vladivostok Standard Time" => "Asia/Vladivostok",
        "Central Pacific Standard Time" => "Pacific/Guadalcanal",
        "New Zealand Standard Time" => "Pacific/Auckland",
        "Fiji Standard Time" => "Pacific/Fiji",
        "Tonga Standard Time" => "Pacific/Tongatapu",
        "Magadan Standard Time" => "Asia/Magadan",
        _ => {
            tracing::warn!(tz = name, "unknown Windows timezone, falling back to UTC");
            "UTC"
        }
    };

    iana.parse::<Tz>().map_err(|_| {
        AppError::internal(format!("Failed to parse mapped IANA timezone: {iana}"))
    })
}

fn normalise_microsoft_rsvp(response: &str) -> String {
    match response {
        "accepted" => "accepted",
        "declined" => "declined",
        "tentativelyAccepted" => "tentative",
        "none" | "notResponded" | "organizer" => "needs_action",
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
    fn test_parse_graph_datetime_utc() {
        let dt = GraphDateTimeZone {
            date_time: Some("2024-06-15T10:30:00.0000000".to_string()),
            time_zone: Some("UTC".to_string()),
        };
        let parsed = parse_graph_datetime(&dt).unwrap();
        assert_eq!(parsed.to_rfc3339(), "2024-06-15T10:30:00+00:00");
    }

    #[test]
    fn test_parse_graph_datetime_windows_tz() {
        let dt = GraphDateTimeZone {
            date_time: Some("2024-06-15T10:30:00.0000000".to_string()),
            time_zone: Some("Pacific Standard Time".to_string()),
        };
        let parsed = parse_graph_datetime(&dt).unwrap();
        // June = PDT = UTC-7
        assert_eq!(parsed.to_rfc3339(), "2024-06-15T17:30:00+00:00");
    }

    #[test]
    fn test_parse_graph_datetime_iana_tz() {
        let dt = GraphDateTimeZone {
            date_time: Some("2024-01-15T09:00:00.0000000".to_string()),
            time_zone: Some("America/New_York".to_string()),
        };
        let parsed = parse_graph_datetime(&dt).unwrap();
        // January = EST = UTC-5
        assert_eq!(parsed.to_rfc3339(), "2024-01-15T14:00:00+00:00");
    }

    #[test]
    fn test_normalise_graph_datetime_long_frac() {
        assert_eq!(
            normalise_graph_datetime("2024-01-15T09:00:00.0000000"),
            "2024-01-15T09:00:00.000000"
        );
    }

    #[test]
    fn test_normalise_graph_datetime_no_frac() {
        assert_eq!(
            normalise_graph_datetime("2024-01-15T09:00:00"),
            "2024-01-15T09:00:00"
        );
    }

    #[test]
    fn test_rsvp_normalisation() {
        assert_eq!(normalise_microsoft_rsvp("accepted"), "accepted");
        assert_eq!(normalise_microsoft_rsvp("declined"), "declined");
        assert_eq!(normalise_microsoft_rsvp("tentativelyAccepted"), "tentative");
        assert_eq!(normalise_microsoft_rsvp("none"), "needs_action");
        assert_eq!(normalise_microsoft_rsvp("notResponded"), "needs_action");
        assert_eq!(normalise_microsoft_rsvp("organizer"), "needs_action");
    }

    #[test]
    fn test_status_mapping_cancelled() {
        let raw = serde_json::json!({
            "id": "test-id",
            "subject": "Test",
            "isCancelled": true,
            "start": {
                "dateTime": "2024-01-15T09:00:00.0000000",
                "timeZone": "UTC"
            },
            "end": {
                "dateTime": "2024-01-15T10:00:00.0000000",
                "timeZone": "UTC"
            }
        });
        let ev = convert_graph_event(&raw).unwrap();
        assert_eq!(ev.status, "cancelled");
    }

    #[test]
    fn test_status_mapping_tentative() {
        let raw = serde_json::json!({
            "id": "test-id",
            "subject": "Test",
            "showAs": "tentative",
            "start": {
                "dateTime": "2024-01-15T09:00:00.0000000",
                "timeZone": "UTC"
            },
            "end": {
                "dateTime": "2024-01-15T10:00:00.0000000",
                "timeZone": "UTC"
            }
        });
        let ev = convert_graph_event(&raw).unwrap();
        assert_eq!(ev.status, "tentative");
    }

    #[test]
    fn test_sensitivity_mapping() {
        let raw = serde_json::json!({
            "id": "test-id",
            "subject": "Test",
            "sensitivity": "private",
            "start": {
                "dateTime": "2024-01-15T09:00:00.0000000",
                "timeZone": "UTC"
            },
            "end": {
                "dateTime": "2024-01-15T10:00:00.0000000",
                "timeZone": "UTC"
            }
        });
        let ev = convert_graph_event(&raw).unwrap();
        assert_eq!(ev.visibility, "private");
    }

    #[test]
    fn test_removed_item() {
        let raw = serde_json::json!({
            "id": "removed-event-id",
            "@removed": { "reason": "deleted" }
        });
        let ev = convert_removed_item(&raw).unwrap();
        assert_eq!(ev.provider_event_id, "removed-event-id");
        assert_eq!(ev.status, "cancelled");
    }

    #[test]
    fn test_recurring_instance_detection() {
        let raw = serde_json::json!({
            "id": "test-id",
            "subject": "Recurring",
            "type": "occurrence",
            "seriesMasterId": "master-id",
            "start": {
                "dateTime": "2024-01-15T09:00:00.0000000",
                "timeZone": "UTC"
            },
            "end": {
                "dateTime": "2024-01-15T10:00:00.0000000",
                "timeZone": "UTC"
            }
        });
        let ev = convert_graph_event(&raw).unwrap();
        assert!(ev.is_recurring_instance);
        assert_eq!(ev.recurring_event_id, Some("master-id".to_string()));
    }

    #[test]
    fn test_resolve_timezone_iana() {
        let tz = resolve_timezone("America/New_York").unwrap();
        assert_eq!(tz, chrono_tz::America::New_York);
    }

    #[test]
    fn test_resolve_timezone_windows() {
        let tz = resolve_timezone("Eastern Standard Time").unwrap();
        assert_eq!(tz, chrono_tz::America::New_York);
    }
}
