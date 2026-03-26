pub mod apply;
pub mod health;
pub mod optimize;

use axum::{
    routing::{get, post},
    Router,
};
use async_nats::Client as NatsClient;
use sqlx::PgPool;
use std::sync::Arc;

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    #[allow(dead_code)]
    pub nats: NatsClient,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/api/v1/optimize", post(optimize::optimize))
        .route(
            "/api/v1/optimize/:run_id",
            get(apply::get_run),
        )
        .route(
            "/api/v1/optimize/:run_id/apply",
            post(apply::apply),
        )
        .with_state(state)
}
