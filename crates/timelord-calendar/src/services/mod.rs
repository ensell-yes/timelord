pub mod calendar_service;
pub mod event_service;
pub mod nats_publisher;
pub mod provider_client;

use async_nats::Client as NatsClient;
use sqlx::PgPool;
use std::sync::Arc;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub nats: NatsClient,
}

impl AppState {
    pub async fn new(pool: PgPool, config: Config) -> anyhow::Result<Self> {
        let nats = async_nats::connect(&config.nats_url).await?;
        Ok(Self {
            pool,
            config: Arc::new(config),
            nats,
        })
    }
}
