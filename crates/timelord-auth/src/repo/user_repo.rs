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

pub async fn upsert<'e>(
    executor: impl sqlx::PgExecutor<'e>,
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
    .fetch_one(executor)
    .await?;
    Ok(user)
}

/// Find a local user by email for password login.
pub async fn find_local_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, AppError> {
    let user = sqlx::query_as!(
        User,
        "SELECT * FROM users WHERE provider = 'local' AND provider_sub = $1 AND password_hash IS NOT NULL",
        email
    )
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

/// Create a local user with password hash.
pub async fn create_local_user<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    email: &str,
    display_name: &str,
    password_hash: &str,
    system_admin: bool,
) -> Result<User, AppError> {
    let user = sqlx::query_as!(
        User,
        r#"
        INSERT INTO users (provider, provider_sub, email, display_name, password_hash, system_admin)
        VALUES ('local', $1, $2, $3, $4, $5)
        RETURNING *
        "#,
        email,
        email,
        display_name,
        password_hash,
        system_admin
    )
    .fetch_one(executor)
    .await?;
    Ok(user)
}

/// Update a local user's password hash (used by CLI reset-password).
#[allow(dead_code)]
pub async fn update_password(
    pool: &PgPool,
    user_id: Uuid,
    password_hash: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE users SET password_hash = $2, updated_at = now() WHERE id = $1",
        user_id,
        password_hash
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Check if a user is a system admin.
pub async fn is_system_admin(pool: &PgPool, user_id: Uuid) -> Result<bool, AppError> {
    let row = sqlx::query!("SELECT system_admin FROM users WHERE id = $1", user_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.system_admin).unwrap_or(false))
}

pub async fn update_last_active_org<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    user_id: Uuid,
    org_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE users SET last_active_org_id = $1, updated_at = now() WHERE id = $2",
        org_id,
        user_id
    )
    .execute(executor)
    .await?;
    Ok(())
}
