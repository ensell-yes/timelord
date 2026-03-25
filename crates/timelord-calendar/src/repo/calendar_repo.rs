use sqlx::PgPool;
use uuid::Uuid;

use crate::models::calendar::{Calendar, CreateCalendarRequest};
use timelord_common::error::AppError;

pub async fn list_by_user(
    pool: &PgPool,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<Calendar>, AppError> {
    let calendars = sqlx::query_as!(
        Calendar,
        "SELECT * FROM calendars WHERE org_id = $1 AND user_id = $2 ORDER BY is_primary DESC, name ASC",
        org_id,
        user_id
    )
    .fetch_all(pool)
    .await?;
    Ok(calendars)
}

pub async fn find_by_id(
    pool: &PgPool,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<Calendar>, AppError> {
    let cal = sqlx::query_as!(
        Calendar,
        "SELECT * FROM calendars WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(cal)
}

pub async fn create(
    pool: &PgPool,
    org_id: Uuid,
    user_id: Uuid,
    req: &CreateCalendarRequest,
) -> Result<Calendar, AppError> {
    let cal = sqlx::query_as!(
        Calendar,
        r#"
        INSERT INTO calendars (org_id, user_id, provider, provider_calendar_id, name, color, is_primary, timezone, display_mode)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *
        "#,
        org_id,
        user_id,
        req.provider,
        req.provider_calendar_id,
        req.name,
        req.color,
        req.is_primary.unwrap_or(false),
        req.timezone.as_deref().unwrap_or("UTC"),
        req.display_mode.as_deref().unwrap_or("busy"),
    )
    .fetch_one(pool)
    .await?;
    Ok(cal)
}

pub async fn delete(pool: &PgPool, org_id: Uuid, id: Uuid) -> Result<bool, AppError> {
    let result = sqlx::query!(
        "DELETE FROM calendars WHERE org_id = $1 AND id = $2",
        org_id,
        id
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
