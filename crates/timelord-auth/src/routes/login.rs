use axum::{extract::State, Json};
use std::sync::Arc;

use crate::{
    repo::{org_repo, user_repo},
    services::{password, session as session_svc, AppState},
};
use timelord_common::{
    audit::{insert_audit, AuditEntry},
    error::AppError,
};

#[derive(serde::Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// POST /auth/login — email/password authentication for local users.
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let email = body.email.trim().to_lowercase();

    // Rate-limit: atomic INCR returns new count; set TTL on first attempt
    {
        use redis::AsyncCommands;
        let rate_key = format!("login_attempts:{email}");
        let mut redis = state.redis.clone();
        let count: i64 = redis.incr(&rate_key, 1i64).await.unwrap_or(1);
        if count == 1 {
            let _: () = redis.expire(&rate_key, 60).await.unwrap_or_default();
        }
        if count > 5 {
            return Err(AppError::BadRequest(
                "Too many login attempts. Try again in 1 minute.".into(),
            ));
        }
    }

    let user = user_repo::find_local_by_email(&state.pool, &email)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let hash = user
        .password_hash
        .as_deref()
        .ok_or(AppError::Unauthorized)?;
    if !password::verify_password(&body.password, hash)? {
        return Err(AppError::Unauthorized);
    }

    // Resolve org_id (same logic as OAuth callback)
    let org_id = match user.last_active_org_id {
        Some(id) => id,
        None => {
            let orgs = org_repo::list_user_orgs(&state.pool, user.id).await?;
            orgs.first()
                .map(|(org, _)| org.id)
                .ok_or(AppError::Unauthorized)?
        }
    };

    // Resolve role
    let role = org_repo::get_member_role(&state.pool, org_id, user.id)
        .await?
        .ok_or(AppError::Unauthorized)?;

    let (_session, tokens) = session_svc::create_session(
        &state.pool,
        &state.jwt,
        user.id,
        org_id,
        &role.to_string(),
        None,
    )
    .await?;

    insert_audit(
        &state.pool,
        AuditEntry::new(org_id, "login", "user")
            .user(user.id)
            .entity(user.id),
    )
    .await;

    Ok(Json(serde_json::json!({
        "access_token": tokens.access_token,
        "refresh_token": tokens.refresh_token,
        "expires_at": tokens.expires_at,
        "token_type": tokens.token_type,
        "user": {
            "id": user.id,
            "email": user.email,
            "display_name": user.display_name,
        }
    })))
}
