use timelord_common::config::{env_or, env_parse, require_env};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub http_port: u16,
    /// "stdio" or "http"
    pub transport: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: require_env("DATABASE_URL")?,
            http_port: env_parse("MCP_HTTP_PORT", 3006),
            transport: env_or("MCP_TRANSPORT", "stdio"),
        })
    }
}
