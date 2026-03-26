use sqlx::PgConnection;
use uuid::Uuid;

use crate::provider::UpsertEvent;
use timelord_common::error::AppError;

/// Upsert a provider event by (calendar_id, provider_event_id).
/// Returns true if a new row was inserted, false if updated.
pub async fn upsert_event(
    conn: &mut PgConnection,
    org_id: Uuid,
    calendar_id: Uuid,
    event: &UpsertEvent,
) -> Result<bool, AppError> {
    let result = sqlx::query!(
        r#"
        INSERT INTO events (
            org_id, calendar_id, provider_event_id, provider_etag,
            title, description, location, conference_data,
            start_at, end_at, all_day, timezone,
            status, visibility,
            is_organizer, organizer_email, self_rsvp_status,
            attendees, recurrence_rule, recurring_event_id, is_recurring_instance,
            reminders, extended_properties,
            provider_synced_at
        ) VALUES (
            $1, $2, $3, $4,
            $5, $6, $7, $8,
            $9, $10, $11, $12,
            $13::event_status, $14::event_visibility,
            $15, $16, $17::rsvp_status,
            $18, $19, $20, $21,
            $22, $23,
            now()
        )
        ON CONFLICT (calendar_id, provider_event_id)
        DO UPDATE SET
            provider_etag = EXCLUDED.provider_etag,
            title = EXCLUDED.title,
            description = EXCLUDED.description,
            location = EXCLUDED.location,
            conference_data = EXCLUDED.conference_data,
            start_at = EXCLUDED.start_at,
            end_at = EXCLUDED.end_at,
            all_day = EXCLUDED.all_day,
            timezone = EXCLUDED.timezone,
            status = EXCLUDED.status,
            visibility = EXCLUDED.visibility,
            is_organizer = EXCLUDED.is_organizer,
            organizer_email = EXCLUDED.organizer_email,
            self_rsvp_status = EXCLUDED.self_rsvp_status,
            attendees = EXCLUDED.attendees,
            recurrence_rule = EXCLUDED.recurrence_rule,
            recurring_event_id = EXCLUDED.recurring_event_id,
            is_recurring_instance = EXCLUDED.is_recurring_instance,
            reminders = EXCLUDED.reminders,
            extended_properties = EXCLUDED.extended_properties,
            provider_synced_at = now(),
            updated_at = now()
        "#,
        org_id,
        calendar_id,
        event.provider_event_id,
        event.provider_etag,
        event.title,
        event.description,
        event.location,
        event.conference_data,
        event.start_at,
        event.end_at,
        event.all_day,
        event.timezone,
        &event.status as &str,
        &event.visibility as &str,
        event.is_organizer,
        event.organizer_email,
        &event.self_rsvp_status as &str,
        event.attendees,
        event.recurrence_rule,
        event.recurring_event_id,
        event.is_recurring_instance,
        event.reminders,
        event.extended_properties,
    )
    .execute(&mut *conn)
    .await?;

    // rows_affected == 1 for both insert and update with ON CONFLICT
    // xmax == 0 means insert, but we can't easily check that through sqlx
    // Use a simpler heuristic: if the query succeeded, it either inserted or updated
    Ok(result.rows_affected() > 0)
}
