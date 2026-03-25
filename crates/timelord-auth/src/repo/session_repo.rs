use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::session::Session;
use timelord_common::error::AppError;

#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Uuid,
    token_hash: &str,
    refresh_hash: &str,
    user_agent: Option<&str>,
    expires_at: DateTime<Utc>,
    refresh_expires_at: DateTime<Utc>,
) -> Result<Session, AppError> {
    let session = sqlx::query_as!(
        Session,
        r#"
        INSERT INTO sessions (user_id, org_id, token_hash, refresh_hash, user_agent, expires_at, refresh_expires_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#,
        user_id,
        org_id,
        token_hash,
        refresh_hash,
        user_agent,
        expires_at,
        refresh_expires_at
    )
    .fetch_one(pool)
    .await?;
    Ok(session)
}

pub async fn find_by_refresh_hash(
    pool: &PgPool,
    refresh_hash: &str,
) -> Result<Option<Session>, AppError> {
    let session = sqlx::query_as!(
        Session,
        "SELECT * FROM sessions WHERE refresh_hash = $1 AND revoked_at IS NULL",
        refresh_hash
    )
    .fetch_optional(pool)
    .await?;
    Ok(session)
}

pub async fn find_by_token_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<Session>, AppError> {
    let session = sqlx::query_as!(
        Session,
        "SELECT * FROM sessions WHERE token_hash = $1 AND revoked_at IS NULL",
        token_hash
    )
    .fetch_optional(pool)
    .await?;
    Ok(session)
}

pub async fn update_token_hash(
    pool: &PgPool,
    session_id: Uuid,
    token_hash: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE sessions SET token_hash = $2 WHERE id = $1",
        session_id,
        token_hash
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn revoke(pool: &PgPool, session_id: Uuid) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE sessions SET revoked_at = now() WHERE id = $1",
        session_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
