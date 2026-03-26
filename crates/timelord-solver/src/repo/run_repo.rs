use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::models::OptimizationRun;
use timelord_common::error::AppError;

pub async fn create<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    user_id: Uuid,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    config: &Value,
) -> Result<OptimizationRun, AppError> {
    let run = sqlx::query_as!(
        OptimizationRun,
        r#"
        INSERT INTO optimization_runs (org_id, user_id, window_start, window_end, config, status)
        VALUES ($1, $2, $3, $4, $5, 'running')
        RETURNING *
        "#,
        org_id,
        user_id,
        window_start,
        window_end,
        config
    )
    .fetch_one(executor)
    .await?;
    Ok(run)
}

pub async fn complete<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    run_id: Uuid,
    metrics: &Value,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE optimization_runs
        SET status = 'completed', metrics = $2, completed_at = now()
        WHERE id = $1
        "#,
        run_id,
        metrics
    )
    .execute(executor)
    .await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn fail<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    run_id: Uuid,
    error: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE optimization_runs SET status = 'failed', error = $2, completed_at = now() WHERE id = $1",
        run_id,
        error
    )
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn find_by_id<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    run_id: Uuid,
) -> Result<Option<OptimizationRun>, AppError> {
    let run = sqlx::query_as!(
        OptimizationRun,
        "SELECT * FROM optimization_runs WHERE org_id = $1 AND id = $2",
        org_id,
        run_id
    )
    .fetch_optional(executor)
    .await?;
    Ok(run)
}
