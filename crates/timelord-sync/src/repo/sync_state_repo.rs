use sqlx::PgConnection;
use uuid::Uuid;

use timelord_common::error::AppError;

/// Update sync state after a successful sync iteration.
pub async fn update_after_sync(
    conn: &mut PgConnection,
    calendar_id: Uuid,
    sync_token: Option<&str>,
    event_count: i32,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE sync_state
        SET sync_token = $2,
            last_synced_at = now(),
            last_error = NULL,
            event_count = $3,
            updated_at = now()
        WHERE calendar_id = $1
        "#,
        calendar_id,
        sync_token,
        event_count,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Record a sync error for a calendar.
pub async fn record_error(
    conn: &mut PgConnection,
    calendar_id: Uuid,
    error: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE sync_state
        SET last_error = $2, updated_at = now()
        WHERE calendar_id = $1
        "#,
        calendar_id,
        error,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Clear the sync token (e.g., after a 410 Gone from Google).
pub async fn clear_sync_token(
    conn: &mut PgConnection,
    calendar_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE sync_state SET sync_token = NULL, updated_at = now() WHERE calendar_id = $1",
        calendar_id,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}
