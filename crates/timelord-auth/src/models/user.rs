use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub provider: String,
    pub provider_sub: String,
    pub is_active: bool,
    pub last_active_org_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MicrosoftUserInfo {
    pub id: String,
    pub mail: Option<String>,
    #[serde(rename = "userPrincipalName")]
    pub user_principal_name: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
}

impl MicrosoftUserInfo {
    pub fn email(&self) -> String {
        self.mail
            .clone()
            .or_else(|| self.user_principal_name.clone())
            .unwrap_or_default()
    }
}
