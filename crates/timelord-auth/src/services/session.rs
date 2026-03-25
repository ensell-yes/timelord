use chrono::{Duration, Utc};
use redis::{aio::ConnectionManager, AsyncCommands};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    models::session::{Session, TokenPair},
    repo::{org_repo, session_repo, user_repo},
    services::jwt::{self, JwtService},
};
use timelord_common::error::AppError;

/// Create a new session: mint JWT + refresh token, store hashes in DB.
pub async fn create_session(
    pool: &PgPool,
    jwt_svc: &JwtService,
    user_id: Uuid,
    org_id: Uuid,
    role: &str,
    user_agent: Option<&str>,
) -> Result<(Session, TokenPair), AppError> {
    let access_token = jwt_svc.encode_access(user_id, org_id, role)?;
    let refresh_token = jwt::generate_refresh_token();

    let token_hash = jwt::hash_token(&access_token);
    let refresh_hash = jwt::hash_token(&refresh_token);

    let now = Utc::now();
    let expires_at = now + Duration::seconds(jwt_svc.access_ttl_secs);
    let refresh_expires_at = now + Duration::seconds(jwt_svc.refresh_ttl_secs);

    let session = session_repo::create(
        pool,
        user_id,
        org_id,
        &token_hash,
        &refresh_hash,
        user_agent,
        expires_at,
        refresh_expires_at,
    )
    .await?;

    let pair = TokenPair::new(access_token, refresh_token, expires_at);
    Ok((session, pair))
}

/// Validate a refresh token and issue a new access token.
pub async fn refresh_session(
    pool: &PgPool,
    jwt_svc: &JwtService,
    refresh_token: &str,
) -> Result<TokenPair, AppError> {
    let refresh_hash = jwt::hash_token(refresh_token);
    let session = session_repo::find_by_refresh_hash(pool, &refresh_hash)
        .await?
        .ok_or(AppError::Unauthorized)?;

    if session.refresh_expires_at < Utc::now() {
        return Err(AppError::Unauthorized);
    }

    // Re-resolve the best org for this user
    let user = user_repo::find_by_id(pool, session.user_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let org_id = user.last_active_org_id.unwrap_or(session.org_id);

    let role = org_repo::get_member_role(pool, org_id, session.user_id)
        .await?
        .ok_or(AppError::Unauthorized)?
        .to_string();
    let access_token = jwt_svc.encode_access(session.user_id, org_id, &role)?;

    // Keep token_hash current so logout can find this session after refresh
    let new_token_hash = jwt::hash_token(&access_token);
    session_repo::update_token_hash(pool, session.id, &new_token_hash).await?;

    let now = Utc::now();
    let expires_at = now + Duration::seconds(jwt_svc.access_ttl_secs);

    Ok(TokenPair::new(
        access_token,
        refresh_token.to_string(),
        expires_at,
    ))
}

/// Revoke a session and add its jti to the Redis denylist.
#[allow(dead_code)]
pub async fn revoke_session(
    pool: &PgPool,
    redis: &mut ConnectionManager,
    session_id: Uuid,
    jti: Uuid,
    remaining_ttl_secs: i64,
) -> Result<(), AppError> {
    session_repo::revoke(pool, session_id).await?;

    // Add jti to Redis denylist with TTL aligned to token lifetime
    if remaining_ttl_secs > 0 {
        let key = format!("jti:{jti}");
        redis
            .set_ex::<_, _, ()>(&key, "1", remaining_ttl_secs as u64)
            .await
            .map_err(|e| AppError::internal(format!("Redis jti denylist: {e}")))?;
    }

    Ok(())
}

/// Check if a jti has been revoked (exists in Redis denylist).
#[allow(dead_code)]
pub async fn is_jti_revoked(redis: &mut ConnectionManager, jti: &Uuid) -> Result<bool, AppError> {
    let key = format!("jti:{jti}");
    let exists: bool = redis
        .exists(&key)
        .await
        .map_err(|e| AppError::internal(format!("Redis jti check: {e}")))?;
    Ok(exists)
}
