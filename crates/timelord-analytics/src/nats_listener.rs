use async_nats::Client as NatsClient;
use chrono::Utc;
use sqlx::PgPool;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::{health, repo};
use timelord_common::{db, error::AppError};

/// Subscribe to domain events and trigger health snapshot updates.
/// Resolves user_id from the event payload or by looking up the calendar owner,
/// since published NATS events don't always include user_id at the top level.
pub async fn run_nats_listener(pool: PgPool, nats: NatsClient) {
    let mut sub = match nats.subscribe("timelord.>").await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "failed to subscribe to NATS");
            return;
        }
    };

    tracing::info!("analytics NATS listener started");

    while let Some(msg) = sub.next().await {
        let org_id = match extract_field(&msg.payload, "org_id") {
            Some(id) => id,
            None => continue,
        };

        // Resolve user_id: try direct field, then calendar_id lookup, then
        // entity_id (which may be an event_id — resolve via events→calendars join)
        let user_id = if let Some(uid) = extract_field(&msg.payload, "user_id") {
            Some(uid)
        } else if let Some(cal_id) = extract_field(&msg.payload, "calendar_id") {
            resolve_user_from_calendar(&pool, org_id, cal_id).await
        } else if let Some(entity_id) = extract_field(&msg.payload, "entity_id") {
            // entity_id could be a calendar_id or event_id — try both lookups
            let from_cal = resolve_user_from_calendar(&pool, org_id, entity_id).await;
            if from_cal.is_some() {
                from_cal
            } else {
                resolve_user_from_event(&pool, org_id, entity_id).await
            }
        } else {
            None
        };

        if let Some(user_id) = user_id {
            if let Err(e) = update_snapshot(&pool, org_id, user_id).await {
                tracing::warn!(
                    error = %e,
                    org_id = %org_id,
                    subject = %msg.subject,
                    "failed to update snapshot after NATS event"
                );
            }
        }
    }
}

async fn update_snapshot(pool: &PgPool, org_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
    let report = health::compute_health(pool, org_id, user_id).await?;
    let today = Utc::now().date_naive();
    let metrics = serde_json::to_value(&report).unwrap_or_default();

    let mut tx = pool.begin().await.map_err(AppError::internal)?;
    db::set_rls_context(&mut tx, org_id).await.map_err(AppError::internal)?;

    repo::upsert_snapshot(&mut *tx, org_id, user_id, today, report.health_score, &metrics).await?;
    tx.commit().await.map_err(AppError::internal)?;

    Ok(())
}

/// Resolve user_id from a calendar_id by querying calendars.user_id.
async fn resolve_user_from_calendar(pool: &PgPool, org_id: Uuid, calendar_id: Uuid) -> Option<Uuid> {
    let row = sqlx::query!(
        "SELECT user_id FROM calendars WHERE org_id = $1 AND id = $2",
        org_id,
        calendar_id
    )
    .fetch_optional(pool)
    .await
    .ok()?;
    row.map(|r| r.user_id)
}

/// Resolve user_id from an event_id by joining events → calendars.
async fn resolve_user_from_event(pool: &PgPool, org_id: Uuid, event_id: Uuid) -> Option<Uuid> {
    let row = sqlx::query!(
        "SELECT c.user_id FROM events e JOIN calendars c ON c.id = e.calendar_id WHERE e.org_id = $1 AND e.id = $2",
        org_id,
        event_id
    )
    .fetch_optional(pool)
    .await
    .ok()?;
    row.map(|r| r.user_id)
}

fn extract_field(payload: &[u8], field: &str) -> Option<Uuid> {
    let v: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let s = v.get(field)?.as_str()?;
    s.parse().ok()
}
