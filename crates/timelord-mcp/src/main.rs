/// MCP stub — Phase 4 will implement full Model Context Protocol server.
use axum::{routing::get, Json, Router};
use dotenvy::dotenv;
use serde_json::{json, Value};
use timelord_common::{config::env_parse, telemetry};

pub async fn healthz() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "timelord-mcp",
        "phase": "stub — MCP server in Phase 4",
        "transports": ["stdio", "sse"]
    }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    telemetry::init("timelord-mcp");

    let port: u16 = env_parse("MCP_HTTP_PORT", 3006);
    let app = Router::new().route("/healthz", get(healthz));

    tracing::info!(port = port, "timelord-mcp stub listening");
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
