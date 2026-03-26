use axum::{extract::State, Json};
use std::sync::Arc;

use crate::services::AppState;

/// GET /setup/status — public, no auth required.
/// Returns first-run detection info for installers/UIs.
pub async fn status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let setup_complete = sqlx::query_scalar!(
        r#"SELECT COALESCE((SELECT value::text = 'true' FROM system_settings WHERE key = 'setup_complete'), false) AS "complete!""#
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    let has_admin = sqlx::query_scalar!(
        r#"SELECT EXISTS(SELECT 1 FROM users WHERE system_admin = true) AS "exists!""#
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(false);

    Json(serde_json::json!({
        "setup_complete": setup_complete,
        "has_admin": has_admin,
    }))
}
