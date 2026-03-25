use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::provider_token::ProviderToken;
use timelord_common::error::AppError;

#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Uuid,
    provider: &str,
    access_token_enc: &[u8],
    refresh_token_enc: &[u8],
    token_nonce: &[u8],
    scopes: &[String],
    expires_at: DateTime<Utc>,
) -> Result<ProviderToken, AppError> {
    let token = sqlx::query_as!(
        ProviderToken,
        r#"
        INSERT INTO provider_tokens
            (user_id, org_id, provider, access_token_enc, refresh_token_enc, token_nonce, scopes, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (user_id, provider) DO UPDATE
            SET access_token_enc  = EXCLUDED.access_token_enc,
                refresh_token_enc = EXCLUDED.refresh_token_enc,
                token_nonce       = EXCLUDED.token_nonce,
                scopes            = EXCLUDED.scopes,
                expires_at        = EXCLUDED.expires_at,
                updated_at        = now()
        RETURNING *
        "#,
        user_id,
        org_id,
        provider,
        access_token_enc,
        refresh_token_enc,
        token_nonce,
        scopes,
        expires_at
    )
    .fetch_one(pool)
    .await?;
    Ok(token)
}

#[allow(dead_code)]
pub async fn find_by_user_provider(
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
