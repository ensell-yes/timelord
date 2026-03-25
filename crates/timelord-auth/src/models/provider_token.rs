use chrono::{DateTime, Utc};
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProviderToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub provider: String,
    pub access_token_enc: Vec<u8>,
    pub refresh_token_enc: Vec<u8>,
    pub token_nonce: Vec<u8>,
    pub scopes: Vec<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
