use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub token_hash: String,
    pub refresh_hash: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Returned to the client after successful login or refresh.
#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub token_type: &'static str,
}

impl TokenPair {
    pub fn new(access_token: String, refresh_token: String, expires_at: DateTime<Utc>) -> Self {
        Self {
            access_token,
            refresh_token,
            expires_at,
            token_type: "Bearer",
        }
    }
}
