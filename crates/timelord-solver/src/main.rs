mod config;
mod models;
mod repo;
mod routes;
mod solver;

use std::sync::Arc;

use dotenvy::dotenv;
use timelord_common::{db, telemetry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    telemetry::init("timelord-solver");

    let config = config::Config::from_env()?;
    let pool = db::create_pool(&config.database_url).await?;

    let migrations_path = std::env::var("MIGRATIONS_PATH")
        .unwrap_or_else(|_| "crates/timelord-solver/migrations".to_string());
    db::run_migrations(&pool, &migrations_path).await?;

    let nats = async_nats::connect(&config.nats_url).await?;
    let config = Arc::new(config);

    let state = Arc::new(routes::AppState {
        pool,
        config: config.clone(),
        nats,
    });

    let app = routes::router(state);
    let addr = format!("0.0.0.0:{}", config.http_port);
    tracing::info!(addr = %addr, "timelord-solver listening");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
