use sqlx::PgConnection;
use uuid::Uuid;

use timelord_common::error::AppError;

/// Upsert sync state after a successful sync iteration.
/// Creates the row if it doesn't exist (handles calendars not created via import).
pub async fn update_after_sync(
    conn: &mut PgConnection,
    org_id: Uuid,
    calendar_id: Uuid,
    sync_token: Option<&str>,
    event_count: i32,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO sync_state (org_id, calendar_id, sync_token, last_synced_at, event_count)
        VALUES ($1, $2, $3, now(), $4)
        ON CONFLICT (calendar_id) DO UPDATE SET
            sync_token = EXCLUDED.sync_token,
            last_synced_at = now(),
            last_error = NULL,
            event_count = EXCLUDED.event_count,
            updated_at = now()
        "#,
        org_id,
        calendar_id,
        sync_token,
        event_count,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Record a sync error for a calendar. Creates the row if missing.
pub async fn record_error(
    conn: &mut PgConnection,
    org_id: Uuid,
    calendar_id: Uuid,
    error: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO sync_state (org_id, calendar_id, last_error)
        VALUES ($1, $2, $3)
        ON CONFLICT (calendar_id) DO UPDATE SET
            last_error = EXCLUDED.last_error,
            updated_at = now()
        "#,
        org_id,
        calendar_id,
        error,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Clear the sync token (e.g., after a 410 Gone from Google). Creates the row if missing.
pub async fn clear_sync_token(
    conn: &mut PgConnection,
    org_id: Uuid,
    calendar_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO sync_state (org_id, calendar_id, sync_token)
        VALUES ($1, $2, NULL)
        ON CONFLICT (calendar_id) DO UPDATE SET
            sync_token = NULL,
            updated_at = now()
        "#,
        org_id,
        calendar_id,
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}
