use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// An audit log entry describing a write operation.
pub struct AuditEntry {
    pub org_id: Uuid,
    pub user_id: Option<Uuid>,
    /// Verb: "create", "update", "delete", "login", "logout", "org_switch"
    pub action: String,
    /// Entity type: "user", "calendar", "event", "session", "org"
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub metadata: Value,
    pub ip_address: Option<std::net::IpAddr>,
}

impl AuditEntry {
    pub fn new(org_id: Uuid, action: impl Into<String>, entity_type: impl Into<String>) -> Self {
        Self {
            org_id,
            user_id: None,
            action: action.into(),
            entity_type: entity_type.into(),
            entity_id: None,
            metadata: Value::Null,
            ip_address: None,
        }
    }

    pub fn user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    pub fn entity(mut self, entity_id: Uuid) -> Self {
        self.entity_id = Some(entity_id);
        self
    }

    pub fn meta(mut self, metadata: Value) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn ip(mut self, ip: std::net::IpAddr) -> Self {
        self.ip_address = Some(ip);
        self
    }
}

/// Insert an audit log entry. Fire-and-forget — logs errors but does not fail the caller.
pub async fn insert_audit(pool: &PgPool, entry: AuditEntry) {
    let ip_str = entry.ip_address.map(|ip| ip.to_string());
    let result = sqlx::query(
        r#"
        INSERT INTO audit_log (org_id, user_id, action, entity_type, entity_id, metadata, ip_address)
        VALUES ($1, $2, $3, $4, $5, $6, $7::inet)
        "#,
    )
    .bind(entry.org_id)
    .bind(entry.user_id)
    .bind(&entry.action)
    .bind(&entry.entity_type)
    .bind(entry.entity_id)
    .bind(&entry.metadata)
    .bind(ip_str.as_deref())
    .execute(pool)
    .await;

    if let Err(e) = result {
        tracing::warn!(
            error = %e,
            action = %entry.action,
            entity_type = %entry.entity_type,
            "failed to insert audit log entry"
        );
    }
}
