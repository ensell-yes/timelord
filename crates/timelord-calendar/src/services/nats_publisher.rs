use async_nats::Client;
use serde::Serialize;
use uuid::Uuid;

/// Publish a domain event to NATS.
/// Subject format: `timelord.<entity>.<action>`
pub async fn publish_event<T: Serialize>(
    nats: &Client,
    entity: &str,
    action: &str,
    org_id: Uuid,
    entity_id: Uuid,
    payload: &T,
) {
    let subject = format!("timelord.{entity}.{action}");
    let body = match serde_json::to_vec(&serde_json::json!({
        "org_id": org_id,
        "entity_id": entity_id,
        "payload": payload,
    })) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, subject = %subject, "failed to serialize NATS event");
            return;
        }
    };

    if let Err(e) = nats.publish(subject.clone(), body.into()).await {
        tracing::warn!(error = %e, subject = %subject, "failed to publish NATS event");
    }
}
