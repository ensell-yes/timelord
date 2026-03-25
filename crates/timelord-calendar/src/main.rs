mod config;
mod grpc;
mod models;
mod repo;
mod routes;
mod services;

use dotenvy::dotenv;
use std::sync::Arc;
use timelord_common::{db, telemetry};
use tracing::info;

pub use config::Config;
pub use services::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    telemetry::init("timelord-calendar");

    let config = Config::from_env()?;
    let pool = db::create_pool(&config.database_url).await?;
    db::run_migrations(&pool, "./crates/timelord-calendar/migrations").await?;

    let state = Arc::new(AppState::new(pool, config).await?);

    info!("timelord-calendar starting");
    routes::serve(state).await
}
