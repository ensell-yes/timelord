use axum::{
    extract::{Path, State},
    Json,
};
use axum_extra::{
    extract::TypedHeader,
    headers::{authorization::Bearer, Authorization},
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    models::org_member::OrgRole,
    repo::{org_repo, user_repo},
    services::{password, AppState},
};
use timelord_common::{
    audit::{insert_audit, AuditEntry},
    db,
    error::AppError,
};

/// Decode JWT and verify system_admin. Returns (user_id, org_id) from claims.
async fn require_system_admin(
    state: &AppState,
    auth: &Authorization<Bearer>,
) -> Result<(Uuid, Uuid), AppError> {
    let token_data = state.jwt.decode_access(auth.token())?;
    let claims = &token_data.claims;

    if !user_repo::is_system_admin(&state.pool, claims.sub).await? {
        return Err(AppError::Forbidden);
    }

    Ok((claims.sub, claims.org))
}

/// Decode JWT and return claims (for org-scoped endpoints).
fn decode_jwt(
    state: &AppState,
    auth: &Authorization<Bearer>,
) -> Result<timelord_common::auth_claims::Claims, AppError> {
    let token_data = state.jwt.decode_access(auth.token())?;
    Ok(token_data.claims)
}

// --- Instance admin endpoints ---

#[derive(serde::Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: Option<String>,
    pub display_name: Option<String>,
    pub provider: Option<String>,
    pub provider_sub: Option<String>,
    pub org_id: Option<Uuid>,
}

/// POST /admin/users — create a user (instance admin only).
pub async fn create_user(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(body): Json<CreateUserRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (admin_id, _) = require_system_admin(&state, &auth).await?;

    let email = body.email.trim().to_lowercase();
    let display_name = body.display_name.as_deref().unwrap_or(&email);

    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;

    let user = if let Some(password) = &body.password {
        // Local user
        let hash = password::hash_password(password)?;
        user_repo::create_local_user(&mut *tx, &email, display_name, &hash, false).await?
    } else {
        // OAuth pre-provision — validate provider is google or microsoft
        let provider = body
            .provider
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("provider required for OAuth pre-provision".into()))?;
        if !matches!(provider, "google" | "microsoft") {
            return Err(AppError::BadRequest("provider must be 'google' or 'microsoft'".into()));
        }
        let provider_sub = body
            .provider_sub
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("provider_sub required for OAuth pre-provision".into()))?;
        if provider_sub.is_empty() {
            return Err(AppError::BadRequest("provider_sub must not be empty".into()));
        }
        user_repo::upsert(&mut *tx, provider, provider_sub, &email, display_name, None).await?
    };

    // Org provisioning
    let org_id = if let Some(org_id) = body.org_id {
        // Add to existing org
        db::set_rls_context(&mut tx, org_id).await.map_err(AppError::internal)?;
        org_repo::add_member(&mut *tx, org_id, user.id, OrgRole::Member).await?;
        org_id
    } else {
        // Create personal org
        let slug = format!("personal-{}", &user.id.to_string()[..8]);
        let org = org_repo::create(&mut *tx, "Personal", &slug, true).await?;
        db::set_rls_context(&mut tx, org.id).await.map_err(AppError::internal)?;
        org_repo::add_member(&mut *tx, org.id, user.id, OrgRole::Owner).await?;
        org.id
    };

    user_repo::update_last_active_org(&mut *tx, user.id, org_id).await?;

    insert_audit(
        &mut *tx,
        AuditEntry::new(org_id, "admin_create_user", "user")
            .user(admin_id)
            .entity(user.id),
    )
    .await;

    tx.commit().await.map_err(AppError::internal)?;

    Ok(Json(serde_json::json!({
        "id": user.id,
        "email": user.email,
        "provider": user.provider,
        "system_admin": user.system_admin,
    })))
}

/// POST /admin/orgs — create an organization (instance admin only).
#[derive(serde::Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    pub slug: String,
}

pub async fn create_org(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Json(body): Json<CreateOrgRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (admin_id, _) = require_system_admin(&state, &auth).await?;

    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    let org = org_repo::create(&mut *tx, &body.name, &body.slug, false).await?;

    db::set_rls_context(&mut tx, org.id).await.map_err(AppError::internal)?;

    insert_audit(
        &mut *tx,
        AuditEntry::new(org.id, "admin_create_org", "org")
            .user(admin_id)
            .entity(org.id),
    )
    .await;

    tx.commit().await.map_err(AppError::internal)?;

    Ok(Json(serde_json::json!({
        "id": org.id,
        "name": org.name,
        "slug": org.slug,
    })))
}

// --- Org-scoped admin endpoints ---

/// GET /admin/users — list members of the caller's active org (org admin only).
pub async fn list_users(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let claims = decode_jwt(&state, &auth)?;

    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, claims.org).await.map_err(AppError::internal)?;

    let caller_role = org_repo::get_member_role(&mut *tx, claims.org, claims.sub)
        .await?
        .ok_or(AppError::Forbidden)?;
    if !matches!(caller_role, OrgRole::Owner | OrgRole::Admin) {
        return Err(AppError::Forbidden);
    }

    let members = org_repo::list_org_members(&mut *tx, claims.org).await?;
    tx.commit().await.map_err(AppError::internal)?;

    Ok(Json(serde_json::json!({ "members": members })))
}

/// POST /admin/orgs/:id/members — add a user to an org.
#[derive(serde::Deserialize)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
    pub role: String,
}

pub async fn add_member(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path(org_id): Path<Uuid>,
    Json(body): Json<AddMemberRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let claims = decode_jwt(&state, &auth)?;

    let role = match body.role.as_str() {
        "owner" => OrgRole::Owner,
        "admin" => OrgRole::Admin,
        "member" => OrgRole::Member,
        _ => return Err(AppError::BadRequest("role must be owner, admin, or member".into())),
    };

    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, org_id).await.map_err(AppError::internal)?;

    // Verify caller is admin/owner of target org (inside RLS transaction)
    let caller_role = org_repo::get_member_role(&mut *tx, org_id, claims.sub)
        .await?
        .ok_or(AppError::Forbidden)?;
    if !matches!(caller_role, OrgRole::Owner | OrgRole::Admin) {
        return Err(AppError::Forbidden);
    }

    let member = org_repo::add_member(&mut *tx, org_id, body.user_id, role).await?;

    insert_audit(
        &mut *tx,
        AuditEntry::new(org_id, "admin_add_member", "org_member")
            .user(claims.sub)
            .entity(body.user_id)
            .meta(serde_json::json!({ "role": body.role })),
    )
    .await;

    tx.commit().await.map_err(AppError::internal)?;

    Ok(Json(serde_json::json!({
        "org_id": member.org_id,
        "user_id": member.user_id,
        "role": member.role.to_string(),
    })))
}

/// PUT /admin/users/:id/role — change a user's role within the caller's org.
#[derive(serde::Deserialize)]
pub struct ChangeRoleRequest {
    pub role: String,
}

pub async fn change_role(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path(user_id): Path<Uuid>,
    Json(body): Json<ChangeRoleRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let claims = decode_jwt(&state, &auth)?;

    let role = match body.role.as_str() {
        "owner" => OrgRole::Owner,
        "admin" => OrgRole::Admin,
        "member" => OrgRole::Member,
        _ => return Err(AppError::BadRequest("role must be owner, admin, or member".into())),
    };

    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, claims.org).await.map_err(AppError::internal)?;

    let caller_role = org_repo::get_member_role(&mut *tx, claims.org, claims.sub)
        .await?
        .ok_or(AppError::Forbidden)?;
    if !matches!(caller_role, OrgRole::Owner | OrgRole::Admin) {
        return Err(AppError::Forbidden);
    }

    let member = org_repo::add_member(&mut *tx, claims.org, user_id, role).await?;

    insert_audit(
        &mut *tx,
        AuditEntry::new(claims.org, "admin_change_role", "org_member")
            .user(claims.sub)
            .entity(user_id)
            .meta(serde_json::json!({ "new_role": body.role })),
    )
    .await;

    tx.commit().await.map_err(AppError::internal)?;

    Ok(Json(serde_json::json!({
        "user_id": member.user_id,
        "org_id": member.org_id,
        "role": member.role.to_string(),
    })))
}

/// DELETE /admin/users/:id — remove a user from the caller's org.
pub async fn remove_user(
    State(state): State<Arc<AppState>>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path(user_id): Path<Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let claims = decode_jwt(&state, &auth)?;

    if user_id == claims.sub {
        return Err(AppError::BadRequest("Cannot remove yourself".into()));
    }

    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, claims.org).await.map_err(AppError::internal)?;

    let caller_role = org_repo::get_member_role(&mut *tx, claims.org, claims.sub)
        .await?
        .ok_or(AppError::Forbidden)?;
    if !matches!(caller_role, OrgRole::Owner | OrgRole::Admin) {
        return Err(AppError::Forbidden);
    }

    org_repo::remove_member(&mut *tx, claims.org, user_id).await?;

    insert_audit(
        &mut *tx,
        AuditEntry::new(claims.org, "admin_remove_user", "org_member")
            .user(claims.sub)
            .entity(user_id),
    )
    .await;

    tx.commit().await.map_err(AppError::internal)?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
