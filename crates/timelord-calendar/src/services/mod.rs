pub mod calendar_service;
pub mod event_service;
pub mod nats_publisher;
pub mod provider_client;

use async_nats::Client as NatsClient;
use sqlx::PgPool;
use std::sync::Arc;
use timelord_common::token_encryption::TokenEncryptor;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub nats: NatsClient,
    pub http_client: reqwest::Client,
    pub encryptor: Arc<TokenEncryptor>,
}

impl AppState {
    pub async fn new(pool: PgPool, config: Config) -> anyhow::Result<Self> {
        let nats = async_nats::connect(&config.nats_url).await?;
        let encryptor = Arc::new(TokenEncryptor::new(&config.encryption_key)?);
        Ok(Self {
            pool,
            config: Arc::new(config),
            nats,
            http_client: reqwest::Client::new(),
            encryptor,
        })
    }
}
