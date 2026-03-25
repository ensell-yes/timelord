use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    models::event::{CreateEventRequest, ListEventsQuery},
    services::{event_service, AppState},
};
use timelord_common::{auth_claims::Claims, error::AppError};

pub async fn list(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Path(cal_id): Path<Uuid>,
    Query(query): Query<ListEventsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let limit = query.page_size.unwrap_or(50).clamp(1, 200);
    let offset = query
        .page_token
        .as_ref()
        .and_then(|t| t.parse::<i64>().ok())
        .unwrap_or(0);

    let events = event_service::list(
        &state,
        claims.org,
        cal_id,
        query.time_min,
        query.time_max,
        limit,
        offset,
    )
    .await?;

    let next = if events.len() as i64 == limit {
        Some((offset + limit).to_string())
    } else {
        None
    };

    Ok(Json(serde_json::json!({
        "events": events,
        "next_page_token": next,
    })))
}

pub async fn get_one(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let event = event_service::get(&state, claims.org, id).await?;
    Ok(Json(serde_json::json!(event)))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Path(cal_id): Path<Uuid>,
    Json(body): Json<CreateEventRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let event = event_service::create(&state, claims.org, cal_id, claims.sub, body).await?;
    Ok((StatusCode::CREATED, Json(serde_json::json!(event))))
}

pub async fn delete_one(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    event_service::delete(&state, claims.org, claims.sub, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
