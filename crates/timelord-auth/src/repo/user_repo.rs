use sqlx::PgPool;
use uuid::Uuid;

use crate::models::user::User;
use timelord_common::error::AppError;

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>, AppError> {
    let user = sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

#[allow(dead_code)]
pub async fn find_by_provider_sub(
    pool: &PgPool,
    provider: &str,
    provider_sub: &str,
) -> Result<Option<User>, AppError> {
    let user = sqlx::query_as!(
        User,
        "SELECT * FROM users WHERE provider = $1 AND provider_sub = $2",
        provider,
        provider_sub
    )
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

pub async fn upsert(
    pool: &PgPool,
    provider: &str,
    provider_sub: &str,
    email: &str,
    display_name: &str,
    avatar_url: Option<&str>,
) -> Result<User, AppError> {
    let user = sqlx::query_as!(
        User,
        r#"
        INSERT INTO users (provider, provider_sub, email, display_name, avatar_url)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (provider, provider_sub) DO UPDATE
            SET email        = EXCLUDED.email,
                display_name = EXCLUDED.display_name,
                avatar_url   = EXCLUDED.avatar_url,
                updated_at   = now()
        RETURNING *
        "#,
        provider,
        provider_sub,
        email,
        display_name,
        avatar_url
    )
    .fetch_one(pool)
    .await?;
    Ok(user)
}

pub async fn update_last_active_org(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE users SET last_active_org_id = $1, updated_at = now() WHERE id = $2",
        org_id,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
