mod config;
mod provider;
mod repo;
mod sync_loop;

use std::sync::Arc;

use axum::{routing::get, Json, Router};
use dotenvy::dotenv;
use serde_json::{json, Value};
use timelord_common::{db, telemetry};

pub async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "timelord-sync" }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    telemetry::init("timelord-sync");

    let config = config::Config::from_env()?;
    let pool = db::create_pool(&config.database_url).await?;
    let migrations_path = std::env::var("MIGRATIONS_PATH")
        .unwrap_or_else(|_| "crates/timelord-calendar/migrations".to_string());
    db::run_migrations(&pool, &migrations_path).await?;

    let nats = async_nats::connect(&config.nats_url).await?;
    let http = reqwest::Client::new();
    let encryptor = Arc::new(
        timelord_common::token_encryption::TokenEncryptor::new(&config.encryption_key)?,
    );
    let config = Arc::new(config);

    // Start sync loop in background
    let sync_pool = pool.clone();
    let sync_nats = nats.clone();
    let sync_http = http.clone();
    let sync_encryptor = encryptor.clone();
    let sync_config = config.clone();
    tokio::spawn(async move {
        sync_loop::run_sync_loop(sync_pool, sync_nats, sync_http, sync_encryptor, sync_config)
            .await;
    });

    // Health endpoint
    let app = Router::new().route("/healthz", get(healthz));
    let addr = format!("0.0.0.0:{}", config.http_port);
    tracing::info!(addr = %addr, "timelord-sync listening");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
