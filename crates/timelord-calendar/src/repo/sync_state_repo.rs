use uuid::Uuid;

use timelord_common::error::AppError;

/// Create an initial sync_state row for a newly imported calendar.
pub async fn create<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    calendar_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO sync_state (org_id, calendar_id)
        VALUES ($1, $2)
        ON CONFLICT (calendar_id) DO NOTHING
        "#,
        org_id,
        calendar_id
    )
    .execute(executor)
    .await?;
    Ok(())
}
