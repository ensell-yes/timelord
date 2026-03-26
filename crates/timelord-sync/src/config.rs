use timelord_common::config::{env_or, env_parse, require_env};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub nats_url: String,
    pub http_port: u16,
    pub encryption_key: String,
    pub sync_interval_secs: u64,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub microsoft_client_id: String,
    pub microsoft_client_secret: String,
    pub microsoft_tenant_id: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: require_env("DATABASE_URL")?,
            nats_url: env_or("NATS_URL", "nats://localhost:4222"),
            http_port: env_parse("SYNC_HTTP_PORT", 3003),
            encryption_key: require_env("ENCRYPTION_KEY")?,
            sync_interval_secs: env_parse("SYNC_INTERVAL_SECS", 300),
            google_client_id: require_env("GOOGLE_CLIENT_ID")?,
            google_client_secret: require_env("GOOGLE_CLIENT_SECRET")?,
            microsoft_client_id: require_env("MICROSOFT_CLIENT_ID")?,
            microsoft_client_secret: require_env("MICROSOFT_CLIENT_SECRET")?,
            microsoft_tenant_id: env_or("MICROSOFT_TENANT_ID", "common"),
        })
    }
}
