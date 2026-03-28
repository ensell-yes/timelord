use timelord_common::config::{env_or, env_parse, require_env};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub nats_url: String,
    pub http_port: u16,
    pub grpc_port: u16,
    // JWT
    pub jwt_private_key_pem: String,
    pub jwt_public_key_pem: String,
    pub jwt_key_id: String,
    pub jwt_expiry_seconds: i64,
    pub refresh_expiry_seconds: i64,
    // Encryption
    pub encryption_key: String,
    // Google OAuth
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_uri: String,
    // Microsoft OAuth
    pub microsoft_client_id: String,
    pub microsoft_client_secret: String,
    pub microsoft_redirect_uri: String,
    pub microsoft_tenant_id: String,
    // Frontend
    pub frontend_url: String,
    // CORS
    pub cors_allowed_origins: Vec<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: require_env("DATABASE_URL")?,
            redis_url: env_or("REDIS_URL", "redis://localhost:6379"),
            nats_url: env_or("NATS_URL", "nats://localhost:4222"),
            http_port: env_parse("AUTH_HTTP_PORT", 3001),
            grpc_port: env_parse("AUTH_GRPC_PORT", 50051),
            jwt_private_key_pem: require_env("JWT_PRIVATE_KEY_PEM")?,
            jwt_public_key_pem: require_env("JWT_PUBLIC_KEY_PEM")?,
            jwt_key_id: env_or("JWT_KEY_ID", "key-1"),
            jwt_expiry_seconds: env_parse("JWT_EXPIRY_SECONDS", 900),
            refresh_expiry_seconds: env_parse("REFRESH_EXPIRY_SECONDS", 604800),
            encryption_key: require_env("ENCRYPTION_KEY")?,
            google_client_id: require_env("GOOGLE_CLIENT_ID")?,
            google_client_secret: require_env("GOOGLE_CLIENT_SECRET")?,
            google_redirect_uri: env_or(
                "GOOGLE_REDIRECT_URI",
                "http://localhost:8080/auth/google/callback",
            ),
            microsoft_client_id: require_env("MICROSOFT_CLIENT_ID")?,
            microsoft_client_secret: require_env("MICROSOFT_CLIENT_SECRET")?,
            microsoft_redirect_uri: env_or(
                "MICROSOFT_REDIRECT_URI",
                "http://localhost:8080/auth/microsoft/callback",
            ),
            microsoft_tenant_id: env_or("MICROSOFT_TENANT_ID", "common"),
            frontend_url: env_or("FRONTEND_URL", "http://localhost:8080"),
            cors_allowed_origins: env_or(
                "CORS_ALLOWED_ORIGINS",
                "http://localhost:3000,http://localhost:5173",
            )
            .split(',')
            .map(|s| s.trim().to_string())
            .collect(),
        })
    }
}
