use uuid::Uuid;

use crate::{
    models::calendar::{Calendar, CreateCalendarRequest},
    repo::calendar_repo,
    services::{nats_publisher, AppState},
};
use timelord_common::{
    audit::{insert_audit, AuditEntry},
    error::AppError,
};

pub async fn list(
    state: &AppState,
    org_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<Calendar>, AppError> {
    calendar_repo::list_by_user(&state.pool, org_id, user_id).await
}

pub async fn get(state: &AppState, org_id: Uuid, id: Uuid) -> Result<Calendar, AppError> {
    calendar_repo::find_by_id(&state.pool, org_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Calendar {id} not found")))
}

pub async fn create(
    state: &AppState,
    org_id: Uuid,
    user_id: Uuid,
    req: CreateCalendarRequest,
) -> Result<Calendar, AppError> {
    let cal = calendar_repo::create(&state.pool, org_id, user_id, &req).await?;

    nats_publisher::publish_event(&state.nats, "calendar", "created", org_id, cal.id, &cal).await;

    insert_audit(
        &state.pool,
        AuditEntry::new(org_id, "create", "calendar")
            .user(user_id)
            .entity(cal.id),
    )
    .await;

    Ok(cal)
}

pub async fn delete(
    state: &AppState,
    org_id: Uuid,
    user_id: Uuid,
    id: Uuid,
) -> Result<(), AppError> {
    let deleted = calendar_repo::delete(&state.pool, org_id, id).await?;
    if !deleted {
        return Err(AppError::NotFound(format!("Calendar {id} not found")));
    }

    nats_publisher::publish_event(
        &state.nats,
        "calendar",
        "deleted",
        org_id,
        id,
        &serde_json::json!({ "id": id }),
    )
    .await;

    insert_audit(
        &state.pool,
        AuditEntry::new(org_id, "delete", "calendar")
            .user(user_id)
            .entity(id),
    )
    .await;

    Ok(())
}
