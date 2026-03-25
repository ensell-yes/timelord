mod config;
mod grpc_clients;
mod middleware;
mod routes;

use dotenvy::dotenv;
use timelord_common::telemetry;
use tracing::info;

pub use config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    telemetry::init("timelord-gateway");

    let config = Config::from_env()?;

    info!("timelord-gateway starting on :{}", config.port);
    routes::serve(config).await
}
