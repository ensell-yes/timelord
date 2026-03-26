use timelord_common::config::{env_or, env_parse, require_env};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub nats_url: String,
    pub http_port: u16,
    #[allow(dead_code)]
    pub default_working_hours_start: u32,
    #[allow(dead_code)]
    pub default_working_hours_end: u32,
    pub slot_minutes: u32,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: require_env("DATABASE_URL")?,
            nats_url: env_or("NATS_URL", "nats://localhost:4222"),
            http_port: env_parse("SOLVER_HTTP_PORT", 3004),
            default_working_hours_start: env_parse("SOLVER_WORK_START_HOUR", 9),
            default_working_hours_end: env_parse("SOLVER_WORK_END_HOUR", 17),
            slot_minutes: env_parse("SOLVER_SLOT_MINUTES", 15),
        })
    }
}
