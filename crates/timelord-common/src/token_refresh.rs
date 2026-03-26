use reqwest::Client;
use serde::Deserialize;

use crate::error::AppError;

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
}

/// Result of a successful token refresh.
pub struct RefreshResult {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in_secs: u64,
}

/// Refresh a Google OAuth2 access token using the refresh token.
pub async fn refresh_google_token(
    http: &Client,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<RefreshResult, AppError> {
    let resp = http
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|e| AppError::internal(format!("Google token refresh request: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::internal(format!(
            "Google token refresh failed ({status}): {body}"
        )));
    }

    let token: TokenResponse = resp
        .json()
        .await
        .map_err(|e| AppError::internal(format!("Google token refresh parse: {e}")))?;

    Ok(RefreshResult {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_in_secs: token.expires_in.unwrap_or(3600),
    })
}

/// Refresh a Microsoft OAuth2 access token using the refresh token.
pub async fn refresh_microsoft_token(
    http: &Client,
    client_id: &str,
    client_secret: &str,
    tenant_id: &str,
    refresh_token: &str,
) -> Result<RefreshResult, AppError> {
    let url = format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token");

    let resp = http
        .post(&url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("scope", "offline_access Calendars.ReadWrite User.Read"),
        ])
        .send()
        .await
        .map_err(|e| AppError::internal(format!("Microsoft token refresh request: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::internal(format!(
            "Microsoft token refresh failed ({status}): {body}"
        )));
    }

    let token: TokenResponse = resp
        .json()
        .await
        .map_err(|e| AppError::internal(format!("Microsoft token refresh parse: {e}")))?;

    Ok(RefreshResult {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_in_secs: token.expires_in.unwrap_or(3600),
    })
}
