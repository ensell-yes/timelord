use timelord_common::config::{env_or, env_parse, require_env};

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub redis_url: String,
    pub auth_service_http_url: String,
    pub auth_service_grpc_url: String,
    pub calendar_service_http_url: String,
    pub jwt_public_key_pem: String,
    pub cors_allowed_origins: Vec<String>,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            port: env_parse("GATEWAY_PORT", 8080),
            redis_url: env_or("REDIS_URL", "redis://localhost:6379"),
            auth_service_http_url: env_or("AUTH_SERVICE_HTTP_URL", "http://localhost:3001"),
            auth_service_grpc_url: env_or("AUTH_SERVICE_GRPC_URL", "http://localhost:50051"),
            calendar_service_http_url: env_or("CALENDAR_SERVICE_HTTP_URL", "http://localhost:3002"),
            jwt_public_key_pem: require_env("JWT_PUBLIC_KEY_PEM")?,
            cors_allowed_origins: env_or(
                "CORS_ALLOWED_ORIGINS",
                "http://localhost:3000,http://localhost:5173",
            )
            .split(',')
            .map(|s| s.trim().to_string())
            .collect(),
            tls_cert_path: std::env::var("TLS_CERT_PATH").ok(),
            tls_key_path: std::env::var("TLS_KEY_PATH").ok(),
        })
    }
}
