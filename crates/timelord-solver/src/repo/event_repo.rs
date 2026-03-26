use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::models::SolverEvent;
use timelord_common::error::AppError;

#[derive(sqlx::FromRow)]
struct EventRow {
    id: Uuid,
    title: String,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    all_day: bool,
    is_movable: bool,
    is_heads_down: bool,
    is_organizer: bool,
    attendees: Value,
    status: String,
}

pub async fn list_for_optimization<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    user_id: Uuid,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> Result<Vec<SolverEvent>, AppError> {
    let rows = sqlx::query_as!(
        EventRow,
        r#"
        SELECT e.id, e.title, e.start_at, e.end_at, e.all_day,
               e.is_movable, e.is_heads_down, e.is_organizer,
               e.attendees, e.status::text AS "status!"
        FROM events e
        JOIN calendars c ON c.id = e.calendar_id
        WHERE e.org_id = $1
          AND c.user_id = $2
          AND e.status != 'cancelled'
          AND e.end_at > $3
          AND e.start_at < $4
        ORDER BY e.start_at ASC
        "#,
        org_id,
        user_id,
        window_start,
        window_end
    )
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SolverEvent {
            id: r.id,
            title: r.title,
            start_at: r.start_at,
            end_at: r.end_at,
            all_day: r.all_day,
            is_movable: r.is_movable,
            is_heads_down: r.is_heads_down,
            is_organizer: r.is_organizer,
            attendees: r.attendees,
            status: r.status,
        })
        .collect())
}

pub async fn update_times<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    event_id: Uuid,
    new_start: DateTime<Utc>,
    new_end: DateTime<Utc>,
) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE events SET start_at = $3, end_at = $4, updated_at = now() WHERE org_id = $1 AND id = $2",
        org_id,
        event_id,
        new_start,
        new_end
    )
    .execute(executor)
    .await?;
    Ok(())
}
