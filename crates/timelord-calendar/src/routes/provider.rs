use axum::{extract::State, Extension, Json};
use std::sync::Arc;

use crate::services::AppState;
use timelord_common::{auth_claims::Claims, error::AppError};

/// List calendars directly from the connected provider (Google or Microsoft).
/// Requires a valid provider access token stored for the authenticated user.
///
/// This endpoint is used during onboarding to let users pick which calendars to sync.
pub async fn list_provider_calendars(
    State(_state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    // TODO Phase 2: retrieve decrypted provider token from provider_tokens table
    // and call the appropriate provider API.
    // For Phase 1, return a stub indicating the endpoint is wired.
    Ok(Json(serde_json::json!({
        "message": "Provider calendar listing requires Phase 2 sync service integration",
        "user_id": claims.sub,
        "org_id": claims.org,
    })))
}
