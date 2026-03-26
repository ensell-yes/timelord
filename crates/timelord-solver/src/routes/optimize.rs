use axum::{extract::State, Extension, Json};
use chrono::NaiveTime;
use std::sync::Arc;

use crate::{
    models::{OptimizeRequest, SolverConfig},
    repo::{event_repo, run_repo, suggestion_repo},
    solver,
};
use timelord_common::{
    audit::{insert_audit, AuditEntry},
    auth_claims::Claims,
    db,
    error::AppError,
};

use super::AppState;

pub async fn optimize(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<OptimizeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let work_start = parse_time(
        body.working_hours_start.as_deref().unwrap_or("09:00"),
    )?;
    let work_end = parse_time(
        body.working_hours_end.as_deref().unwrap_or("17:00"),
    )?;

    let solver_config = SolverConfig {
        window_start: body.window_start,
        window_end: body.window_end,
        work_start,
        work_end,
        slot_minutes: state.config.slot_minutes as i64,
        ..Default::default()
    };

    // Create optimization run record
    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, claims.org).await.map_err(AppError::internal)?;

    let run_config = serde_json::json!({
        "work_start": work_start.to_string(),
        "work_end": work_end.to_string(),
        "timezone": body.timezone,
        "slot_minutes": solver_config.slot_minutes,
    });

    let run = run_repo::create(
        &mut *tx,
        claims.org,
        claims.sub,
        body.window_start,
        body.window_end,
        &run_config,
    )
    .await?;
    tx.commit().await.map_err(AppError::internal)?;

    // Fetch events
    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, claims.org).await.map_err(AppError::internal)?;

    let events = event_repo::list_for_optimization(
        &mut *tx,
        claims.org,
        claims.sub,
        body.window_start,
        body.window_end,
    )
    .await?;
    tx.commit().await.map_err(AppError::internal)?;

    // Run solver (CPU-bound, not async)
    let result = solver::optimize(events, &solver_config);

    // Store results
    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, claims.org).await.map_err(AppError::internal)?;

    let metrics_json = serde_json::to_value(&result.metrics).unwrap_or_default();
    run_repo::complete(&mut *tx, run.id, &metrics_json).await?;

    let suggestions = suggestion_repo::bulk_create(
        &mut tx, run.id, claims.org, &result.suggestions,
    )
    .await?;

    insert_audit(
        &mut *tx,
        AuditEntry::new(claims.org, "optimize", "calendar")
            .user(claims.sub)
            .entity(run.id)
            .meta(metrics_json.clone()),
    )
    .await;

    tx.commit().await.map_err(AppError::internal)?;

    // Build response
    let suggestion_json: Vec<serde_json::Value> = suggestions
        .iter()
        .zip(result.suggestions.iter())
        .map(|(db_s, solver_s)| {
            serde_json::json!({
                "id": db_s.id,
                "event_id": db_s.event_id,
                "event_title": solver_s.event_title,
                "original_start": db_s.original_start,
                "original_end": db_s.original_end,
                "suggested_start": db_s.suggested_start,
                "suggested_end": db_s.suggested_end,
                "reason": db_s.reason,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "run_id": run.id,
        "status": "completed",
        "metrics": metrics_json,
        "suggestions": suggestion_json,
    })))
}

fn parse_time(s: &str) -> Result<NaiveTime, AppError> {
    NaiveTime::parse_from_str(s, "%H:%M")
        .map_err(|e| AppError::BadRequest(format!("Invalid time format '{s}': {e}")))
}
