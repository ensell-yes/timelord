use uuid::Uuid;

use crate::{
    models::event::{CreateEventRequest, Event},
    repo::event_repo,
    services::{nats_publisher, AppState},
};
use chrono::{DateTime, Utc};
use timelord_common::{
    audit::{insert_audit, AuditEntry},
    error::AppError,
};

pub async fn list(
    state: &AppState,
    org_id: Uuid,
    calendar_id: Uuid,
    time_min: Option<DateTime<Utc>>,
    time_max: Option<DateTime<Utc>>,
    limit: i64,
    offset: i64,
) -> Result<Vec<Event>, AppError> {
    event_repo::list_by_calendar(
        &state.pool,
        org_id,
        calendar_id,
        time_min,
        time_max,
        limit,
        offset,
    )
    .await
}

pub async fn get(state: &AppState, org_id: Uuid, id: Uuid) -> Result<Event, AppError> {
    event_repo::find_by_id(&state.pool, org_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Event {id} not found")))
}

pub async fn create(
    state: &AppState,
    org_id: Uuid,
    calendar_id: Uuid,
    user_id: Uuid,
    req: CreateEventRequest,
) -> Result<Event, AppError> {
    if req.end_at <= req.start_at {
        return Err(AppError::BadRequest(
            "end_at must be after start_at".to_string(),
        ));
    }

    let event = event_repo::create(&state.pool, org_id, calendar_id, &req).await?;

    nats_publisher::publish_event(&state.nats, "event", "created", org_id, event.id, &event).await;

    insert_audit(
        &state.pool,
        AuditEntry::new(org_id, "create", "event")
            .user(user_id)
            .entity(event.id),
    )
    .await;

    Ok(event)
}

pub async fn delete(
    state: &AppState,
    org_id: Uuid,
    user_id: Uuid,
    id: Uuid,
) -> Result<(), AppError> {
    let deleted = event_repo::delete(&state.pool, org_id, id).await?;
    if !deleted {
        return Err(AppError::NotFound(format!("Event {id} not found")));
    }

    nats_publisher::publish_event(
        &state.nats,
        "event",
        "deleted",
        org_id,
        id,
        &serde_json::json!({ "id": id }),
    )
    .await;

    insert_audit(
        &state.pool,
        AuditEntry::new(org_id, "delete", "event")
            .user(user_id)
            .entity(id),
    )
    .await;

    Ok(())
}
