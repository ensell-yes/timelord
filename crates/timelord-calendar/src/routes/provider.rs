use axum::{extract::State, Extension, Json};
use chrono::Utc;
use std::sync::Arc;

use crate::services::{provider_client, AppState};
use timelord_common::{
    auth_claims::Claims,
    error::AppError,
    provider_token,
    token_refresh,
};

/// List calendars directly from the connected provider (Google or Microsoft).
/// Decrypts the stored provider access token, refreshes if expired, and calls
/// the provider API. Used during onboarding to let users pick which calendars to sync.
pub async fn list_provider_calendars(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut all_calendars = Vec::new();

    for provider in &["google", "microsoft"] {
        let token = provider_token::find_for_user(&state.pool, claims.sub, provider).await?;
        let token = match token {
            Some(t) => t,
            None => continue,
        };

        let access_token = get_valid_access_token(&state, &token, provider).await?;

        let calendars = match *provider {
            "google" => {
                provider_client::list_google_calendars(&state.http_client, &access_token).await?
            }
            "microsoft" => {
                provider_client::list_microsoft_calendars(&state.http_client, &access_token).await?
            }
            _ => continue,
        };

        let provider_cals: Vec<serde_json::Value> = calendars
            .into_iter()
            .map(|c| {
                serde_json::json!({
                    "provider": provider,
                    "provider_calendar_id": c.provider_id,
                    "name": c.name,
                    "color": c.color,
                    "is_primary": c.is_primary,
                    "timezone": c.timezone,
                })
            })
            .collect();
        all_calendars.extend(provider_cals);
    }

    Ok(Json(serde_json::json!({
        "calendars": all_calendars,
    })))
}

/// Get a valid (non-expired) access token, refreshing if needed.
/// If refresh is required, uses FOR UPDATE locking in a short transaction.
async fn get_valid_access_token(
    state: &AppState,
    token: &timelord_common::provider_token::ProviderToken,
    provider: &str,
) -> Result<String, AppError> {
    // Separate nonces: access_nonce (12 bytes) || refresh_nonce (12 bytes)
    let access_nonce = &token.token_nonce[..12];

    if token.expires_at > Utc::now() {
        // Token is still valid — decrypt and return without locking
        return state.encryptor.decrypt(&token.access_token_enc, access_nonce);
    }

    // Token expired — lock row and refresh
    let refresh_nonce = &token.token_nonce[12..];
    let refresh_token_plain = state.encryptor.decrypt(&token.refresh_token_enc, refresh_nonce)?;

    let result = match provider {
        "google" => {
            token_refresh::refresh_google_token(
                &state.http_client,
                &state.config.google_client_id,
                &state.config.google_client_secret,
                &refresh_token_plain,
            )
            .await?
        }
        "microsoft" => {
            token_refresh::refresh_microsoft_token(
                &state.http_client,
                &state.config.microsoft_client_id,
                &state.config.microsoft_client_secret,
                &state.config.microsoft_tenant_id,
                &refresh_token_plain,
            )
            .await?
        }
        _ => return Err(AppError::internal(format!("Unknown provider: {provider}"))),
    };

    // Re-encrypt and update in a locked transaction
    let new_refresh = result.refresh_token.as_deref().unwrap_or(&refresh_token_plain);
    let (access_enc, access_nonce_new) = state.encryptor.encrypt(&result.access_token)?;
    let (refresh_enc, refresh_nonce_new) = state.encryptor.encrypt(new_refresh)?;
    let mut combined_nonce = access_nonce_new;
    combined_nonce.extend_from_slice(&refresh_nonce_new);
    let new_expires_at = Utc::now() + chrono::Duration::seconds(result.expires_in_secs as i64);

    let mut tx = state.pool.begin().await.map_err(AppError::internal)?;
    // Lock the row to prevent concurrent refresh races
    let locked = provider_token::find_for_user_locked(&mut tx, token.user_id, provider).await?;
    if let Some(locked_token) = locked {
        provider_token::update_tokens(
            &mut tx,
            locked_token.id,
            &access_enc,
            &refresh_enc,
            &combined_nonce,
            new_expires_at,
        )
        .await?;
    }
    tx.commit().await.map_err(AppError::internal)?;

    Ok(result.access_token)
}
