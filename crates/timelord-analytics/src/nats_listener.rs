use async_nats::Client as NatsClient;
use chrono::Utc;
use sqlx::PgPool;
use tokio_stream::StreamExt;

use crate::{health, repo};
use timelord_common::db;

/// Subscribe to domain events and trigger health snapshot updates.
pub async fn run_nats_listener(pool: PgPool, nats: NatsClient) {
    // Subscribe to all timelord domain events
    let mut sub = match nats.subscribe("timelord.>").await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "failed to subscribe to NATS");
            return;
        }
    };

    tracing::info!("analytics NATS listener started");

    while let Some(msg) = sub.next().await {
        // Extract org_id from the event payload
        let org_id = match extract_org_id(&msg.payload) {
            Some(id) => id,
            None => continue,
        };

        // Find users in this org and update their snapshots
        // For now, we extract user_id from the payload if available
        let user_id = extract_user_id(&msg.payload);

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

async fn update_snapshot(
    pool: &PgPool,
    org_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Result<(), timelord_common::error::AppError> {
    let report = health::compute_health(pool, org_id, user_id).await?;
    let today = Utc::now().date_naive();
    let metrics = serde_json::to_value(&report).unwrap_or_default();

    let mut tx = pool.begin().await.map_err(timelord_common::error::AppError::internal)?;
    db::set_rls_context(&mut tx, org_id)
        .await
        .map_err(timelord_common::error::AppError::internal)?;

    repo::upsert_snapshot(&mut *tx, org_id, user_id, today, report.health_score, &metrics).await?;
    tx.commit().await.map_err(timelord_common::error::AppError::internal)?;

    Ok(())
}

fn extract_org_id(payload: &[u8]) -> Option<uuid::Uuid> {
    let v: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let s = v.get("org_id")?.as_str()?;
    s.parse().ok()
}

fn extract_user_id(payload: &[u8]) -> Option<uuid::Uuid> {
    let v: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let s = v.get("user_id")?.as_str()?;
    s.parse().ok()
}
