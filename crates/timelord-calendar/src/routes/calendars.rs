use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    models::calendar::CreateCalendarRequest,
    services::{calendar_service, AppState},
};
use timelord_common::{auth_claims::Claims, error::AppError};

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
