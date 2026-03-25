use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::event::{CreateEventRequest, Event, EventStatus, EventVisibility, RsvpStatus};
use timelord_common::error::AppError;

pub async fn list_by_calendar(
    pool: &PgPool,
    org_id: Uuid,
    calendar_id: Uuid,
    time_min: Option<DateTime<Utc>>,
    time_max: Option<DateTime<Utc>>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Event>, AppError> {
    let events = sqlx::query_as!(
        Event,
        r#"
        SELECT id, org_id, calendar_id, provider_event_id, provider_etag,
               title, description, location, conference_data,
               start_at, end_at, all_day, timezone,
               status AS "status: EventStatus",
               visibility AS "visibility: EventVisibility",
               is_organizer, organizer_email,
               self_rsvp_status AS "self_rsvp_status: RsvpStatus",
               attendees, recurrence_rule, recurring_event_id,
               is_recurring_instance, reminders, extended_properties,
               is_movable, is_heads_down, provider_synced_at,
               created_at, updated_at
        FROM events
        WHERE org_id = $1
          AND calendar_id = $2
          AND ($3::timestamptz IS NULL OR end_at >= $3)
          AND ($4::timestamptz IS NULL OR start_at <= $4)
          AND status != 'cancelled'
        ORDER BY start_at ASC
        LIMIT $5 OFFSET $6
        "#,
        org_id,
        calendar_id,
        time_min,
        time_max,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;
    Ok(events)
}

pub async fn find_by_id(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<Option<Event>, AppError> {
    let event = sqlx::query_as!(
        Event,
        r#"
        SELECT id, org_id, calendar_id, provider_event_id, provider_etag,
               title, description, location, conference_data,
               start_at, end_at, all_day, timezone,
               status AS "status: EventStatus",
               visibility AS "visibility: EventVisibility",
               is_organizer, organizer_email,
               self_rsvp_status AS "self_rsvp_status: RsvpStatus",
               attendees, recurrence_rule, recurring_event_id,
               is_recurring_instance, reminders, extended_properties,
               is_movable, is_heads_down, provider_synced_at,
               created_at, updated_at
        FROM events
        WHERE org_id = $1 AND id = $2
        "#,
        org_id,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(event)
}

pub async fn create(
    pool: &PgPool,
    org_id: Uuid,
    calendar_id: Uuid,
    req: &CreateEventRequest,
) -> Result<Event, AppError> {
    let visibility = match req.visibility.as_deref().unwrap_or("public") {
        "private" => EventVisibility::Private,
        "confidential" => EventVisibility::Confidential,
        _ => EventVisibility::Public,
    };
    let default_attendees = serde_json::json!([]);
    let attendees = req.attendees.as_ref().unwrap_or(&default_attendees);
    let event = sqlx::query_as!(
        Event,
        r#"
        INSERT INTO events
            (org_id, calendar_id, title, description, location,
             start_at, end_at, all_day, timezone, visibility,
             attendees, recurrence_rule, is_movable)
        VALUES
            ($1, $2, $3, $4, $5,
             $6, $7, $8, $9, $10,
             $11, $12, $13)
        RETURNING
            id, org_id, calendar_id, provider_event_id, provider_etag,
            title, description, location, conference_data,
            start_at, end_at, all_day, timezone,
            status AS "status: EventStatus",
            visibility AS "visibility: EventVisibility",
            is_organizer, organizer_email,
            self_rsvp_status AS "self_rsvp_status: RsvpStatus",
            attendees, recurrence_rule, recurring_event_id,
            is_recurring_instance, reminders, extended_properties,
            is_movable, is_heads_down, provider_synced_at,
            created_at, updated_at
        "#,
        org_id,
        calendar_id,
        req.title,
        req.description,
        req.location,
        req.start_at,
        req.end_at,
        req.all_day.unwrap_or(false),
        req.timezone.as_deref().unwrap_or("UTC"),
        visibility as EventVisibility,
        attendees,
        req.recurrence_rule,
        req.is_movable.unwrap_or(true),
    )
    .fetch_one(pool)
    .await?;
    Ok(event)
}

pub async fn delete(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<bool, AppError> {
    let result = sqlx::query!(
        "DELETE FROM events WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
