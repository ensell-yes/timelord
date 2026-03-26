use timelord_common::config::{env_or, env_parse, require_env};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub nats_url: String,
    pub http_port: u16,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: require_env("DATABASE_URL")?,
            nats_url: env_or("NATS_URL", "nats://localhost:4222"),
            http_port: env_parse("ANALYTICS_HTTP_PORT", 3005),
        })
    }
}
