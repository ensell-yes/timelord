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

        // Try to resolve user_id: first from payload, then from calendar_id → calendars.user_id
        let user_id = extract_field(&msg.payload, "user_id")
            .or_else(|| {
                // entity_id might be a calendar_id or event_id — try calendar_id field first
                extract_field(&msg.payload, "calendar_id")
            });

        // If we have a calendar_id, resolve user_id from the DB
        let user_id = match user_id {
            Some(id) => resolve_user_from_calendar(&pool, org_id, id).await.or(Some(id)),
            None => {
                // Try entity_id as a fallback
                let entity_id = extract_field(&msg.payload, "entity_id");
                if let Some(eid) = entity_id {
                    resolve_user_from_calendar(&pool, org_id, eid).await
                } else {
                    None
                }
            }
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

fn extract_field(payload: &[u8], field: &str) -> Option<Uuid> {
    let v: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let s = v.get(field)?.as_str()?;
    s.parse().ok()
}
