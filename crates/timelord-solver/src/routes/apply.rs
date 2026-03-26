use axum::{
    extract::{Path, State},
    Extension, Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    models::ApplyRequest,
    repo::{event_repo, run_repo, suggestion_repo},
};
use timelord_common::{
    audit::{insert_audit, AuditEntry},
    auth_claims::Claims,
    db,
    error::AppError,
};

use super::AppState;

pub async fn apply(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Path(run_id): Path<Uuid>,
    Json(body): Json<ApplyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify run exists and belongs to this org
    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, claims.org).await.map_err(AppError::internal)?;

    let run = run_repo::find_by_id(&mut *tx, claims.org, run_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Optimization run {run_id} not found")))?;

    // Only the user who created the run can apply its suggestions
    if run.user_id != claims.sub {
        return Err(AppError::Forbidden);
    }

    if run.status != "completed" {
        return Err(AppError::BadRequest(format!(
            "Run status is '{}', expected 'completed'",
            run.status
        )));
    }
    tx.commit().await.map_err(AppError::internal)?;

    // Apply each suggestion
    let mut applied = Vec::new();
    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, claims.org).await.map_err(AppError::internal)?;

    for suggestion_id in &body.suggestion_ids {
        let suggestion = suggestion_repo::mark_applied(&mut *tx, claims.org, *suggestion_id)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!("Suggestion {suggestion_id} not found"))
            })?;

        // Ensure suggestion belongs to this run (prevents cross-run abuse)
        if suggestion.run_id != run_id {
            return Err(AppError::BadRequest(format!(
                "Suggestion {suggestion_id} does not belong to run {run_id}"
            )));
        }

        // Update event times
        event_repo::update_times(
            &mut *tx,
            claims.org,
            suggestion.event_id,
            suggestion.suggested_start,
            suggestion.suggested_end,
        )
        .await?;

        applied.push(serde_json::json!({
            "suggestion_id": suggestion.id,
            "event_id": suggestion.event_id,
            "new_start": suggestion.suggested_start,
            "new_end": suggestion.suggested_end,
        }));
    }

    insert_audit(
        &mut *tx,
        AuditEntry::new(claims.org, "apply_optimization", "calendar")
            .user(claims.sub)
            .entity(run_id)
            .meta(serde_json::json!({ "applied_count": applied.len() })),
    )
    .await;

    tx.commit().await.map_err(AppError::internal)?;

    Ok(Json(serde_json::json!({
        "run_id": run_id,
        "applied": applied,
    })))
}

pub async fn get_run(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Path(run_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, claims.org).await.map_err(AppError::internal)?;

    let run = run_repo::find_by_id(&mut *tx, claims.org, run_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Optimization run {run_id} not found")))?;

    let suggestions = suggestion_repo::list_by_run(&mut *tx, claims.org, run_id).await?;
    tx.commit().await.map_err(AppError::internal)?;

    Ok(Json(serde_json::json!({
        "run": run,
        "suggestions": suggestions,
    })))
}
