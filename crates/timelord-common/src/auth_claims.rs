use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT claims embedded in every access token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject: user ID
    pub sub: Uuid,
    /// Active organization ID
    pub org: Uuid,
    /// Role within the active org: "owner" | "admin" | "member"
    pub role: String,
    /// JWT ID — used for revocation denylist in Redis
    pub jti: Uuid,
    /// Issued at (Unix timestamp)
    pub iat: i64,
    /// Expiry (Unix timestamp)
    pub exp: i64,
}

impl Claims {
    pub fn new(user_id: Uuid, org_id: Uuid, role: impl Into<String>, ttl_secs: i64) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            sub: user_id,
            org: org_id,
            role: role.into(),
            jti: Uuid::new_v4(),
            iat: now,
            exp: now + ttl_secs,
        }
    }
}
