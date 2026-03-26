use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProviderToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub provider: String,
    pub access_token_enc: Vec<u8>,
    pub refresh_token_enc: Vec<u8>,
    pub token_nonce: Vec<u8>,
    pub scopes: Vec<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Read-only lookup — no row lock.
pub async fn find_for_user(
    pool: &PgPool,
    user_id: Uuid,
    provider: &str,
) -> Result<Option<ProviderToken>, AppError> {
    let token = sqlx::query_as!(
        ProviderToken,
        "SELECT * FROM provider_tokens WHERE user_id = $1 AND provider = $2",
        user_id,
        provider
    )
    .fetch_optional(pool)
    .await?;
    Ok(token)
}

/// Locked lookup for refresh — must run inside a transaction.
/// Acquires `FOR UPDATE` to prevent concurrent refresh races.
pub async fn find_for_user_locked(
    conn: &mut sqlx::PgConnection,
    user_id: Uuid,
    provider: &str,
) -> Result<Option<ProviderToken>, AppError> {
    let token = sqlx::query_as!(
        ProviderToken,
        "SELECT * FROM provider_tokens WHERE user_id = $1 AND provider = $2 FOR UPDATE",
        user_id,
        provider
    )
    .fetch_optional(&mut *conn)
    .await?;
    Ok(token)
}

/// Update encrypted tokens after a refresh — must run in same tx as the lock.
pub async fn update_tokens(
    conn: &mut sqlx::PgConnection,
    id: Uuid,
    access_token_enc: &[u8],
    refresh_token_enc: &[u8],
    token_nonce: &[u8],
    expires_at: DateTime<Utc>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE provider_tokens
        SET access_token_enc = $2,
            refresh_token_enc = $3,
            token_nonce = $4,
            expires_at = $5,
            updated_at = now()
        WHERE id = $1
        "#,
        id,
        access_token_enc,
        refresh_token_enc,
        token_nonce,
        expires_at
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}
