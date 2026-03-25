#![allow(dead_code)]
use reqwest::Client;
use serde::Deserialize;

use crate::models::calendar::ProviderCalendar;
use timelord_common::error::AppError;

/// Fetch the list of calendars from Google Calendar API.
pub async fn list_google_calendars(
    http: &Client,
    access_token: &str,
) -> Result<Vec<ProviderCalendar>, AppError> {
    #[derive(Deserialize)]
    struct GoogleCalendarList {
        items: Vec<GoogleCalendarItem>,
    }

    #[derive(Deserialize)]
    struct GoogleCalendarItem {
        id: String,
        summary: Option<String>,
        #[serde(rename = "backgroundColor")]
        background_color: Option<String>,
        primary: Option<bool>,
        #[serde(rename = "timeZone")]
        time_zone: Option<String>,
    }

    let list: GoogleCalendarList = http
        .get("https://www.googleapis.com/calendar/v3/users/me/calendarList")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| AppError::internal(format!("Google calendar list request: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::internal(format!("Google calendar list parse: {e}")))?;

    Ok(list
        .items
        .into_iter()
        .map(|item| ProviderCalendar {
            provider_id: item.id,
            name: item.summary.unwrap_or_default(),
            color: item.background_color,
            is_primary: item.primary.unwrap_or(false),
            timezone: item.time_zone,
        })
        .collect())
}

/// Fetch the list of calendars from Microsoft Graph API.
pub async fn list_microsoft_calendars(
    http: &Client,
    access_token: &str,
) -> Result<Vec<ProviderCalendar>, AppError> {
    #[derive(Deserialize)]
    struct GraphResponse {
        value: Vec<GraphCalendar>,
    }

    #[derive(Deserialize)]
    struct GraphCalendar {
        id: String,
        name: String,
        color: Option<String>,
        #[serde(rename = "isDefaultCalendar")]
        is_default: Option<bool>,
    }

    let resp: GraphResponse = http
        .get("https://graph.microsoft.com/v1.0/me/calendars")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| AppError::internal(format!("Microsoft calendar list request: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::internal(format!("Microsoft calendar list parse: {e}")))?;

    Ok(resp
        .value
        .into_iter()
        .map(|cal| ProviderCalendar {
            provider_id: cal.id,
            name: cal.name,
            color: cal.color,
            is_primary: cal.is_default.unwrap_or(false),
            timezone: None,
        })
        .collect())
}
