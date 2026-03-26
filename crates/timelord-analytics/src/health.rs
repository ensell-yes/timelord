use chrono::{Duration, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::repo;
use timelord_common::{db, error::AppError};

#[derive(Debug, Serialize)]
pub struct HealthReport {
    pub health_score: i32,
    pub focus_time_ratio: f64,
    pub fragmentation: f64,
    pub rsvp_ratio: f64,
    pub sync_freshness: f64,
    pub optimization_adoption: f64,
}

/// Compute the current health score for a user.
/// Looks at events in the past 7 days for metric calculation.
pub async fn compute_health(
    pool: &PgPool,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<HealthReport, AppError> {
    let now = Utc::now();
    let window_start = now - Duration::days(7);
    let window_end = now;

    let mut tx = pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, org_id).await.map_err(AppError::internal)?;

    let focus_minutes = repo::focus_minutes_in_window(
        &mut *tx, org_id, user_id, window_start, window_end,
    )
    .await?;

    let fragmentation = repo::fragmentation_score(
        &mut *tx, org_id, user_id, window_start, window_end,
    )
    .await?;

    let rsvp = repo::rsvp_ratio(
        &mut *tx, org_id, user_id, window_start, window_end,
    )
    .await?;

    let sync_fresh = repo::sync_freshness(&mut *tx, org_id, user_id).await?;

    let opt_adoption = repo::optimization_adoption(&mut *tx, org_id, user_id).await?;

    tx.commit().await.map_err(AppError::internal)?;

    // Normalize focus time: target = 4h/day * 7 days = 1680 minutes
    let target_focus = 4.0 * 60.0 * 7.0;
    let focus_ratio = (focus_minutes as f64 / target_focus).min(1.0);

    let raw_score = 25.0 * focus_ratio
        + 25.0 * (1.0 - fragmentation)
        + 20.0 * rsvp
        + 15.0 * sync_fresh
        + 15.0 * opt_adoption;

    let score = (raw_score.round() as i32).clamp(0, 100);

    Ok(HealthReport {
        health_score: score,
        focus_time_ratio: focus_ratio,
        fragmentation,
        rsvp_ratio: rsvp,
        sync_freshness: sync_fresh,
        optimization_adoption: opt_adoption,
    })
}
