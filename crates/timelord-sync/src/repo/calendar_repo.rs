use sqlx::PgPool;
use uuid::Uuid;

use timelord_common::error::AppError;

/// A work item from the SECURITY DEFINER function — no raw cross-org queries.
#[derive(Debug)]
pub struct SyncWorkItem {
    pub calendar_id: Uuid,
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub provider_calendar_id: String,
    pub sync_token: Option<String>,
}

/// List all sync-enabled calendars across all orgs.
/// Uses the SECURITY DEFINER function which bypasses RLS.
pub async fn list_sync_work_items(pool: &PgPool) -> Result<Vec<SyncWorkItem>, AppError> {
    // The function returns nullable columns for UUIDs/strings because it's a
    // RETURNS TABLE, so we query into an intermediate type with Options.
    let rows = sqlx::query!(
        "SELECT calendar_id, org_id, user_id, provider, provider_calendar_id, sync_token FROM list_sync_work_items()"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|r| {
            Some(SyncWorkItem {
                calendar_id: r.calendar_id?,
                org_id: r.org_id?,
                user_id: r.user_id?,
                provider: r.provider?,
                provider_calendar_id: r.provider_calendar_id?,
                sync_token: r.sync_token,
            })
        })
        .collect())
}
