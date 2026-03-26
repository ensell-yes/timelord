use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use timelord_common::error::AppError;

#[derive(Debug, Serialize)]
pub struct CalendarInfo {
    pub id: Uuid,
    pub provider: String,
    pub name: String,
    pub timezone: String,
    pub sync_enabled: bool,
    pub last_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct EventInfo {
    pub id: Uuid,
    pub calendar_name: String,
    pub title: String,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub all_day: bool,
    pub is_movable: bool,
    pub is_heads_down: bool,
    pub status: String,
    pub attendee_count: i64,
}

#[derive(Debug, Serialize)]
pub struct SuggestionInfo {
    pub id: Uuid,
    pub event_title: String,
    pub original_start: DateTime<Utc>,
    pub suggested_start: DateTime<Utc>,
    pub reason: Option<String>,
    pub applied: bool,
}

pub async fn list_calendars(
    pool: &PgPool,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<CalendarInfo>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT c.id, c.provider, c.name, c.timezone, c.sync_enabled,
               s.last_synced_at
        FROM calendars c
        LEFT JOIN sync_state s ON s.calendar_id = c.id
        WHERE c.org_id = $1 AND c.user_id = $2
        ORDER BY c.is_primary DESC, c.name ASC
        "#,
        org_id,
        user_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| CalendarInfo {
        id: r.id,
        provider: r.provider,
        name: r.name,
        timezone: r.timezone,
        sync_enabled: r.sync_enabled,
        last_synced_at: r.last_synced_at,
    }).collect())
}

pub async fn list_events(
    pool: &PgPool,
    org_id: Uuid,
    user_id: Uuid,
    time_min: DateTime<Utc>,
    time_max: DateTime<Utc>,
    calendar_id: Option<Uuid>,
) -> Result<Vec<EventInfo>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT e.id, c.name AS calendar_name, e.title,
               e.start_at, e.end_at, e.all_day,
               e.is_movable, e.is_heads_down,
               e.status::text AS "status!",
               COALESCE(jsonb_array_length(e.attendees), 0)::bigint AS "attendee_count!"
        FROM events e
        JOIN calendars c ON c.id = e.calendar_id
        WHERE e.org_id = $1 AND c.user_id = $2
          AND e.status != 'cancelled'
          AND e.end_at > $3 AND e.start_at < $4
          AND ($5::uuid IS NULL OR e.calendar_id = $5)
        ORDER BY e.start_at ASC
        LIMIT 100
        "#,
        org_id,
        user_id,
        time_min,
        time_max,
        calendar_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| EventInfo {
        id: r.id,
        calendar_name: r.calendar_name,
        title: r.title,
        start_at: r.start_at,
        end_at: r.end_at,
        all_day: r.all_day,
        is_movable: r.is_movable,
        is_heads_down: r.is_heads_down,
        status: r.status,
        attendee_count: r.attendee_count,
    }).collect())
}

pub async fn search_events(
    pool: &PgPool,
    org_id: Uuid,
    user_id: Uuid,
    query: &str,
    time_min: Option<DateTime<Utc>>,
    time_max: Option<DateTime<Utc>>,
) -> Result<Vec<EventInfo>, AppError> {
    let pattern = format!("%{query}%");
    let rows = sqlx::query!(
        r#"
        SELECT e.id, c.name AS calendar_name, e.title,
               e.start_at, e.end_at, e.all_day,
               e.is_movable, e.is_heads_down,
               e.status::text AS "status!",
               COALESCE(jsonb_array_length(e.attendees), 0)::bigint AS "attendee_count!"
        FROM events e
        JOIN calendars c ON c.id = e.calendar_id
        WHERE e.org_id = $1 AND c.user_id = $2
          AND e.status != 'cancelled'
          AND e.title ILIKE $3
          AND ($4::timestamptz IS NULL OR e.end_at > $4)
          AND ($5::timestamptz IS NULL OR e.start_at < $5)
        ORDER BY e.start_at ASC
        LIMIT 50
        "#,
        org_id,
        user_id,
        pattern,
        time_min,
        time_max,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| EventInfo {
        id: r.id,
        calendar_name: r.calendar_name,
        title: r.title,
        start_at: r.start_at,
        end_at: r.end_at,
        all_day: r.all_day,
        is_movable: r.is_movable,
        is_heads_down: r.is_heads_down,
        status: r.status,
        attendee_count: r.attendee_count,
    }).collect())
}

pub async fn get_pending_suggestions(
    pool: &PgPool,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<SuggestionInfo>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT os.id, e.title AS event_title,
               os.original_start, os.suggested_start,
               os.reason, os.applied
        FROM optimization_suggestions os
        JOIN optimization_runs r ON r.id = os.run_id
        JOIN events e ON e.id = os.event_id
        WHERE r.org_id = $1 AND r.user_id = $2
          AND r.status = 'completed'
          AND os.created_at > now() - interval '7 days'
        ORDER BY os.created_at DESC
        LIMIT 20
        "#,
        org_id,
        user_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| SuggestionInfo {
        id: r.id,
        event_title: r.event_title,
        original_start: r.original_start,
        suggested_start: r.suggested_start,
        reason: r.reason,
        applied: r.applied,
    }).collect())
}
