use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value;
use uuid::Uuid;

use timelord_common::error::AppError;

/// Free working minutes: total working hours (8h/day) minus time occupied by events.
pub async fn focus_minutes_in_window<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    user_id: Uuid,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> Result<i64, AppError> {
    // Count minutes occupied by events, then subtract from total working minutes
    let row = sqlx::query!(
        r#"
        SELECT COALESCE(SUM(
            EXTRACT(EPOCH FROM (
                LEAST(e.end_at, $4) - GREATEST(e.start_at, $3)
            )) / 60
        ), 0)::bigint AS "busy_minutes!"
        FROM events e
        JOIN calendars c ON c.id = e.calendar_id
        WHERE e.org_id = $1 AND c.user_id = $2
          AND e.status != 'cancelled'
          AND e.end_at > $3 AND e.start_at < $4
        "#,
        org_id,
        user_id,
        window_start,
        window_end,
    )
    .fetch_one(executor)
    .await?;

    let total_days = (window_end - window_start).num_days().max(1);
    let total_working_minutes = total_days * 8 * 60; // 8h per day
    let free_minutes = (total_working_minutes - row.busy_minutes).max(0);
    Ok(free_minutes)
}

/// Fragmentation: average number of busy→free→busy transitions per day.
pub async fn fragmentation_score<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    user_id: Uuid,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> Result<f64, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT COUNT(*)::bigint AS "event_count!"
        FROM events e
        JOIN calendars c ON c.id = e.calendar_id
        WHERE e.org_id = $1 AND c.user_id = $2
          AND e.status != 'cancelled'
          AND e.end_at > $3 AND e.start_at < $4
        "#,
        org_id,
        user_id,
        window_start,
        window_end,
    )
    .fetch_one(executor)
    .await?;

    let total_days = (window_end - window_start).num_days().max(1) as f64;
    let events_per_day = row.event_count as f64 / total_days;
    // Simple heuristic: more events per day = more fragmented
    // Normalize: 0 events = 0 fragmentation, 8+ events = 1.0
    Ok((events_per_day / 8.0).min(1.0))
}

/// RSVP completeness: % of events where self_rsvp_status ≠ 'needs_action'.
pub async fn rsvp_ratio<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    user_id: Uuid,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> Result<f64, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT
            COUNT(*)::bigint AS "total!",
            COUNT(*) FILTER (WHERE e.self_rsvp_status != 'needs_action')::bigint AS "responded!"
        FROM events e
        JOIN calendars c ON c.id = e.calendar_id
        WHERE e.org_id = $1 AND c.user_id = $2
          AND e.status != 'cancelled'
          AND e.end_at > $3 AND e.start_at < $4
        "#,
        org_id,
        user_id,
        window_start,
        window_end,
    )
    .fetch_one(executor)
    .await?;

    if row.total == 0 {
        return Ok(1.0); // No events = fully responded
    }
    Ok(row.responded as f64 / row.total as f64)
}

/// Sync freshness: 1.0 if all calendars synced within 1h, decaying.
pub async fn sync_freshness<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<f64, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT
            COUNT(*)::bigint AS "total!",
            COUNT(*) FILTER (
                WHERE s.last_synced_at > now() - interval '1 hour'
            )::bigint AS "fresh!"
        FROM calendars c
        LEFT JOIN sync_state s ON s.calendar_id = c.id
        WHERE c.org_id = $1 AND c.user_id = $2 AND c.sync_enabled = true
        "#,
        org_id,
        user_id,
    )
    .fetch_one(executor)
    .await?;

    if row.total == 0 {
        return Ok(1.0);
    }
    Ok(row.fresh as f64 / row.total as f64)
}

/// Optimization adoption: % of suggestions applied in the last 30 days.
pub async fn optimization_adoption<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<f64, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT
            COUNT(*)::bigint AS "total!",
            COUNT(*) FILTER (WHERE os.applied = true)::bigint AS "applied!"
        FROM optimization_suggestions os
        JOIN optimization_runs r ON r.id = os.run_id
        WHERE r.org_id = $1 AND r.user_id = $2
          AND os.created_at > now() - interval '30 days'
        "#,
        org_id,
        user_id,
    )
    .fetch_one(executor)
    .await?;

    if row.total == 0 {
        return Ok(1.0); // No suggestions = perfect adoption
    }
    Ok(row.applied as f64 / row.total as f64)
}

/// Upsert a daily health snapshot.
pub async fn upsert_snapshot<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    user_id: Uuid,
    date: NaiveDate,
    score: i32,
    metrics: &Value,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO analytics_snapshots (org_id, user_id, snapshot_date, health_score, metrics)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (org_id, user_id, snapshot_date) DO UPDATE SET
            health_score = EXCLUDED.health_score,
            metrics = EXCLUDED.metrics
        "#,
        org_id,
        user_id,
        date,
        score,
        metrics,
    )
    .execute(executor)
    .await?;
    Ok(())
}

/// Get health score trend for a user over a date range.
pub async fn get_trends<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    org_id: Uuid,
    user_id: Uuid,
    days: i32,
) -> Result<Vec<(NaiveDate, i32, Value)>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT snapshot_date, health_score, metrics
        FROM analytics_snapshots
        WHERE org_id = $1 AND user_id = $2
          AND snapshot_date >= CURRENT_DATE - $3::integer
        ORDER BY snapshot_date ASC
        "#,
        org_id,
        user_id,
        days,
    )
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| (r.snapshot_date, r.health_score, r.metrics))
        .collect())
}
