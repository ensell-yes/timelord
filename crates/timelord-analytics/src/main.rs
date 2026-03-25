use axum::{routing::get, Json, Router};
use dotenvy::dotenv;
use serde_json::{json, Value};
use timelord_common::{config::env_parse, telemetry};

pub async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "timelord-analytics" }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    telemetry::init("timelord-analytics");

    let port: u16 = env_parse("ANALYTICS_HTTP_PORT", 3005);
    let app = Router::new().route("/healthz", get(healthz));

    tracing::info!(port = port, "timelord-analytics stub listening");
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
