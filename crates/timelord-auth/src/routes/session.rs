use axum::{extract::State, Json};
use axum_extra::{
    extract::TypedHeader,
    headers::{authorization::Bearer, Authorization},
};
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    repo::{org_repo, session_repo, user_repo},
    services::{jwt, session as session_svc, AppState},
};
use timelord_common::{
    audit::{insert_audit, AuditEntry},
    error::AppError,
};

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Deserialize)]
pub struct SwitchOrgRequest {
    pub org_id: Uuid,
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let tokens = session_svc::refresh_session(&state.pool, &state.jwt, &body.refresh_token).await?;
    Ok(Json(serde_json::json!({
        "access_token": tokens.access_token,
        "refresh_token": tokens.refresh_token,
        "expires_at": tokens.expires_at,
        "token_type": tokens.token_type,
    })))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<axum::http::StatusCode, AppError> {
    let token_data = state.jwt.decode_access(auth.token())?;
    let claims = &token_data.claims;

    // Calculate remaining TTL for jti denylist
    let remaining = claims.exp - Utc::now().timestamp();

    // Revoke session in DB
    let token_hash = jwt::hash_token(auth.token());
    if let Some(session) = session_repo::find_by_token_hash(&state.pool, &token_hash).await? {
        session_repo::revoke(&state.pool, session.id).await?;
    }

    // Add jti to Redis denylist so gateway rejects the token immediately
    let mut redis = state.redis.clone();
    if remaining > 0 {
        use redis::AsyncCommands;
        let key = format!("jti:{}", claims.jti);
        redis
            .set_ex::<_, _, ()>(&key, "1", remaining as u64)
            .await
            .map_err(|e| AppError::internal(format!("Redis jti denylist: {e}")))?;
    }

    insert_audit(
        &state.pool,
        AuditEntry::new(claims.org, "logout", "user").user(claims.sub),
    )
    .await;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let token_data = state.jwt.decode_access(auth.token())?;
    let claims = &token_data.claims;

    let user = user_repo::find_by_id(&state.pool, claims.sub)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    let orgs = org_repo::list_user_orgs(&state.pool, claims.sub).await?;
    let orgs_json: Vec<_> = orgs
        .iter()
        .map(|(org, role)| {
            serde_json::json!({
                "id": org.id,
                "name": org.name,
                "slug": org.slug,
                "is_personal": org.is_personal,
                "role": role.to_string(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "id": user.id,
        "email": user.email,
        "display_name": user.display_name,
        "avatar_url": user.avatar_url,
        "active_org_id": claims.org,
        "role": claims.role,
        "orgs": orgs_json,
    })))
}

pub async fn switch_org(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(body): Json<SwitchOrgRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let token_data = state.jwt.decode_access(auth.token())?;
    let claims = &token_data.claims;

    // Verify user is a member of the target org
    let role = org_repo::get_member_role(&state.pool, body.org_id, claims.sub)
        .await?
        .ok_or_else(|| AppError::Forbidden)?;

    // Issue new access token with updated org
    let new_token = state
        .jwt
        .encode_access(claims.sub, body.org_id, &role.to_string())?;
    let expires_at = Utc::now() + chrono::Duration::seconds(state.jwt.access_ttl_secs);

    // Persist last active org first — if this fails, the client keeps the old
    // token whose hash still matches the DB row, so logout stays consistent.
    user_repo::update_last_active_org(&state.pool, claims.sub, body.org_id).await?;

    insert_audit(
        &state.pool,
        AuditEntry::new(body.org_id, "org_switch", "user")
            .user(claims.sub)
            .meta(serde_json::json!({ "from_org": claims.org, "to_org": body.org_id })),
    )
    .await;

    // Update token_hash last — if this fails the client gets an error and keeps
    // the old token, which still matches the old hash in DB. If it succeeds we
    // are about to return the new token, so the hash stays consistent.
    let current_hash = jwt::hash_token(auth.token());
    let new_token_hash = jwt::hash_token(&new_token);
    match session_repo::find_by_token_hash(&state.pool, &current_hash).await? {
        Some(session) => {
            session_repo::update_token_hash(&state.pool, session.id, &new_token_hash).await?;
        }
        None => {
            tracing::warn!(
                user_id = %claims.sub,
                "switch_org: no session found for current bearer token hash"
            );
        }
    }

    Ok(Json(serde_json::json!({
        "access_token": new_token,
        "expires_at": expires_at,
        "token_type": "Bearer",
        "org_id": body.org_id,
    })))
}

pub async fn jwks(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(state.jwt.jwks_json())
}
