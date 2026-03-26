use uuid::Uuid;

use crate::models::{OptimizationSuggestion, SolverSuggestion};
use timelord_common::error::AppError;

pub async fn bulk_create(
    conn: &mut sqlx::PgConnection,
    run_id: Uuid,
    org_id: Uuid,
    suggestions: &[SolverSuggestion],
) -> Result<Vec<OptimizationSuggestion>, AppError> {
    let mut created = Vec::new();
    for s in suggestions {
        let row = sqlx::query_as!(
            OptimizationSuggestion,
            r#"
            INSERT INTO optimization_suggestions
                (run_id, org_id, event_id, original_start, original_end, suggested_start, suggested_end, reason)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
            run_id,
            org_id,
            s.event_id,
            s.original_start,
            s.original_end,
            s.suggested_start,
            s.suggested_end,
            s.reason,
        )
        .fetch_one(&mut *conn)
        .await?;
        created.push(row);
    }
    Ok(created)
}

pub async fn list_by_run<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    run_id: Uuid,
) -> Result<Vec<OptimizationSuggestion>, AppError> {
    let rows = sqlx::query_as!(
        OptimizationSuggestion,
        "SELECT * FROM optimization_suggestions WHERE org_id = $1 AND run_id = $2 ORDER BY created_at ASC",
        org_id,
        run_id
    )
    .fetch_all(executor)
    .await?;
    Ok(rows)
}

pub async fn mark_applied<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    suggestion_id: Uuid,
) -> Result<Option<OptimizationSuggestion>, AppError> {
    let row = sqlx::query_as!(
        OptimizationSuggestion,
        r#"
        UPDATE optimization_suggestions
        SET applied = true, applied_at = now()
        WHERE org_id = $1 AND id = $2
        RETURNING *
        "#,
        org_id,
        suggestion_id
    )
    .fetch_optional(executor)
    .await?;
    Ok(row)
}
