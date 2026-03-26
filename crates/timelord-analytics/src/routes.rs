use axum::{extract::State, Extension, Json};
use serde::Deserialize;
use std::sync::Arc;

use crate::{health, repo, AppState};
use timelord_common::{auth_claims::Claims, db, error::AppError};

pub async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "timelord-analytics",
    }))
}

pub async fn get_health(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let report = health::compute_health(&state.pool, claims.org, claims.sub).await?;
    Ok(Json(serde_json::json!(report)))
}

#[derive(Debug, Deserialize)]
pub struct TrendsQuery {
    pub days: Option<i32>,
}

pub async fn get_trends(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    axum::extract::Query(query): axum::extract::Query<TrendsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let days = query.days.unwrap_or(30);

    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, claims.org)
        .await
        .map_err(AppError::internal)?;

    let trends = repo::get_trends(&mut *tx, claims.org, claims.sub, days).await?;
    tx.commit().await.map_err(AppError::internal)?;

    let data: Vec<serde_json::Value> = trends
        .into_iter()
        .map(|(date, score, metrics)| {
            serde_json::json!({
                "date": date.to_string(),
                "health_score": score,
                "metrics": metrics,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "trends": data })))
}
