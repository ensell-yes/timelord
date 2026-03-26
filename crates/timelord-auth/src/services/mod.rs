pub mod jwt;
pub mod oauth;
pub mod rbac;
pub mod session;

use redis::aio::ConnectionManager;
use sqlx::PgPool;
use std::sync::Arc;

use crate::config::Config;

/// Shared application state for the auth service.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub redis: ConnectionManager,
    pub jwt: Arc<jwt::JwtService>,
    pub encryptor: Arc<timelord_common::token_encryption::TokenEncryptor>,
}

impl AppState {
    pub async fn new(pool: PgPool, config: Config) -> anyhow::Result<Self> {
        let redis_client = redis::Client::open(config.redis_url.as_str())?;
        let redis = ConnectionManager::new(redis_client).await?;
        let jwt = Arc::new(jwt::JwtService::new(&config)?);
        let encryptor = Arc::new(timelord_common::token_encryption::TokenEncryptor::new(
            &config.encryption_key,
        )?);
        Ok(Self {
            pool,
            config: Arc::new(config),
            redis,
            jwt,
            encryptor,
        })
    }
}
