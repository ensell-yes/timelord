mod config;
mod health;
mod nats_listener;
mod repo;
mod routes;

use std::sync::Arc;

use axum::{routing::get, Router};
use dotenvy::dotenv;
use sqlx::PgPool;
use timelord_common::{db, telemetry};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    telemetry::init("timelord-analytics");

    let config = config::Config::from_env()?;
    let pool = db::create_pool(&config.database_url).await?;

    let migrations_path = std::env::var("MIGRATIONS_PATH")
        .unwrap_or_else(|_| "crates/timelord-analytics/migrations".to_string());
    db::run_migrations(&pool, &migrations_path).await?;

    let nats = async_nats::connect(&config.nats_url).await?;

    // Start NATS listener in background
    let listener_pool = pool.clone();
    let listener_nats = nats.clone();
    tokio::spawn(async move {
        nats_listener::run_nats_listener(listener_pool, listener_nats).await;
    });

    let state = Arc::new(AppState { pool });

    let app = Router::new()
        .route("/healthz", get(routes::healthz))
        .route("/api/v1/analytics/health", get(routes::get_health))
        .route("/api/v1/analytics/trends", get(routes::get_trends))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.http_port);
    tracing::info!(addr = %addr, "timelord-analytics listening");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
