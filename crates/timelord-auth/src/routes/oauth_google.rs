use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
};
use oauth2::{PkceCodeVerifier, TokenResponse};
use redis::AsyncCommands;
use serde::Deserialize;
use std::sync::Arc;

use crate::{
    models::org_member::OrgRole,
    repo::{org_repo, token_repo, user_repo},
    services::{oauth::OAuthClients, session as session_svc, AppState},
};
use timelord_common::{
    audit::{insert_audit, AuditEntry},
    error::AppError,
};

#[derive(Deserialize)]
pub struct StartQuery {
    #[allow(dead_code)]
    pub redirect_uri: Option<String>,
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

pub async fn start(
    State(state): State<Arc<AppState>>,
    Query(_params): Query<StartQuery>,
) -> Result<impl IntoResponse, AppError> {
    let oauth = OAuthClients::new(&state.config).map_err(AppError::internal)?;

    let (url, csrf_token, pkce_verifier) = oauth.google_auth_url();

    // Store PKCE verifier in Redis keyed by state, TTL 10 minutes
    let verifier_key = format!("pkce:google:{}", csrf_token.secret());
    let verifier_json =
        serde_json::to_string(pkce_verifier.secret()).map_err(AppError::internal)?;

    let mut redis = state.redis.clone();
    redis
        .set_ex::<_, _, ()>(&verifier_key, &verifier_json, 600)
        .await
        .map_err(|e| AppError::internal(format!("Redis PKCE store: {e}")))?;

    Ok(Redirect::temporary(&url))
}

pub async fn callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CallbackQuery>,
) -> Result<impl IntoResponse, AppError> {
    // Retrieve and consume PKCE verifier from Redis
    let verifier_key = format!("pkce:google:{}", params.state);
    let mut redis = state.redis.clone();
    let verifier_json: Option<String> = redis
        .get_del(&verifier_key)
        .await
        .map_err(|e| AppError::internal(format!("Redis PKCE fetch: {e}")))?;

    let verifier_secret = verifier_json
        .ok_or_else(|| AppError::BadRequest("Invalid or expired state".to_string()))?;
    let verifier_secret: String =
        serde_json::from_str(&verifier_secret).map_err(AppError::internal)?;
    let pkce_verifier = PkceCodeVerifier::new(verifier_secret);

    let oauth = OAuthClients::new(&state.config).map_err(AppError::internal)?;

    let token_response = oauth.google_exchange(&params.code, pkce_verifier).await?;

    let access_token = token_response.access_token().secret();
    let userinfo = oauth.google_userinfo(access_token).await?;

    // Upsert user
    let user = user_repo::upsert(
        &state.pool,
        "google",
        &userinfo.sub,
        &userinfo.email,
        userinfo.name.as_deref().unwrap_or(&userinfo.email),
        userinfo.picture.as_deref(),
    )
    .await?;

    // Ensure user has an org (auto-create personal org on first login)
    let org_id = ensure_personal_org(&state, user.id).await?;

    // Store encrypted provider token
    let refresh_token_str = token_response
        .refresh_token()
        .map(|t| t.secret().as_str())
        .unwrap_or("");
    let expires_at = chrono::Utc::now()
        + token_response
            .expires_in()
            .map(|d| chrono::Duration::from_std(d).unwrap_or(chrono::Duration::hours(1)))
            .unwrap_or(chrono::Duration::hours(1));

    let (access_enc, access_nonce) = state.encryptor.encrypt(access_token)?;
    let (refresh_enc, refresh_nonce) = state.encryptor.encrypt(refresh_token_str)?;
    // Combine nonces: access_nonce || refresh_nonce
    let mut combined_nonce = access_nonce.clone();
    combined_nonce.extend_from_slice(&refresh_nonce);

    let scopes: Vec<String> = token_response
        .scopes()
        .map(|s| s.iter().map(|sc| sc.to_string()).collect())
        .unwrap_or_default();

    token_repo::upsert(
        &state.pool,
        user.id,
        org_id,
        "google",
        &access_enc,
        &refresh_enc,
        &combined_nonce,
        &scopes,
        expires_at,
    )
    .await?;

    // Create JWT session with actual role from org_members
    let role = org_repo::get_member_role(&state.pool, org_id, user.id)
        .await?
        .ok_or(AppError::Unauthorized)?
        .to_string();
    let (_session, tokens) =
        session_svc::create_session(&state.pool, &state.jwt, user.id, org_id, &role, None).await?;

    // Auto-promote first user to system admin
    let has_admin = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM users WHERE system_admin = true) AS "exists!""#
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    if !has_admin {
        user_repo::promote_to_system_admin(&state.pool, user.id).await?;
    }

    insert_audit(
        &state.pool,
        AuditEntry::new(org_id, "login", "user")
            .user(user.id)
            .entity(user.id),
    )
    .await;

    // Redirect to frontend with tokens in URL fragment
    let fragment = format!(
        "access_token={}&refresh_token={}&expires_at={}&user_id={}&email={}&display_name={}",
        urlencoding::encode(&tokens.access_token),
        urlencoding::encode(&tokens.refresh_token),
        urlencoding::encode(&tokens.expires_at.to_rfc3339()),
        user.id,
        urlencoding::encode(&user.email),
        urlencoding::encode(&user.display_name),
    );
    let redirect_url = format!(
        "{}/#{}",
        state.config.frontend_url.trim_end_matches('/'),
        fragment,
    );
    Ok(Redirect::temporary(&redirect_url))
}

async fn ensure_personal_org(
    state: &AppState,
    user_id: uuid::Uuid,
) -> Result<uuid::Uuid, AppError> {
    let orgs = org_repo::list_user_orgs(&state.pool, user_id).await?;
    if let Some((org, _)) = orgs.first() {
        return Ok(org.id);
    }

    // First login — create personal org
    let slug = format!("personal-{}", &user_id.to_string()[..8]);
    let org = org_repo::create(&state.pool, "Personal", &slug, true).await?;
    org_repo::add_member(&state.pool, org.id, user_id, OrgRole::Owner).await?;
    user_repo::update_last_active_org(&state.pool, user_id, org.id).await?;

    Ok(org.id)
}
