use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    models::calendar::CreateCalendarRequest,
    repo::{calendar_repo, sync_state_repo},
    services::{calendar_service, nats_publisher, AppState},
};
use timelord_common::{
    audit::{insert_audit, AuditEntry},
    auth_claims::Claims,
    db,
    error::AppError,
};

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let calendars = calendar_service::list(&state, claims.org, claims.sub).await?;
    Ok(Json(serde_json::json!({ "calendars": calendars })))
}

pub async fn get_one(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let cal = calendar_service::get(&state, claims.org, id).await?;
    Ok(Json(serde_json::json!(cal)))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateCalendarRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let cal = calendar_service::create(&state, claims.org, claims.sub, body).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!(cal))))
}

pub async fn delete_one(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    calendar_service::delete(&state, claims.org, claims.sub, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct ImportCalendarEntry {
    pub provider: String,
    pub provider_calendar_id: String,
    pub name: String,
    pub color: Option<String>,
    pub is_primary: Option<bool>,
    pub timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub calendars: Vec<ImportCalendarEntry>,
}

/// Bulk-import provider calendars with sync enabled.
/// org_id and user_id come from JWT claims (never from request body).
/// Runs in a single transaction with RLS context. Emits one audit entry per
/// calendar (action "import") — calendar_service::create is bypassed to avoid
/// duplicate "create" audits.
pub async fn import(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<ImportRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, claims.org).await.map_err(AppError::internal)?;

    let mut created = Vec::new();

    for entry in &body.calendars {
        let req = CreateCalendarRequest {
            provider: entry.provider.clone(),
            provider_calendar_id: entry.provider_calendar_id.clone(),
            name: entry.name.clone(),
            color: entry.color.clone(),
            is_primary: entry.is_primary,
            timezone: entry.timezone.clone(),
            display_mode: Some("busy".to_string()),
        };

        let cal = calendar_repo::create(&mut *tx, claims.org, claims.sub, &req).await?;

        // Create sync_state row with org_id denormalized from calendar
        sync_state_repo::create(&mut *tx, claims.org, cal.id).await?;

        insert_audit(
            &mut *tx,
            AuditEntry::new(claims.org, "import", "calendar")
                .user(claims.sub)
                .entity(cal.id),
        )
        .await;

        created.push(cal);
    }

    tx.commit().await.map_err(AppError::internal)?;

    // Publish NATS events after commit
    for cal in &created {
        nats_publisher::publish_event(&state.nats, "calendar", "created", claims.org, cal.id, cal)
            .await;
    }

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "calendars": created }))))
}
