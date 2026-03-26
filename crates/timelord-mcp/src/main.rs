mod config;
mod repo;
mod tools;

use std::sync::Arc;

use axum::{routing::get, Json, Router};
use dotenvy::dotenv;
use rmcp::ServiceExt;
use serde_json::{json, Value};
use timelord_common::{db, telemetry};

async fn healthz() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "timelord-mcp",
        "transports": ["stdio"],
    }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    telemetry::init("timelord-mcp");

    let config = config::Config::from_env()?;
    let pool = db::create_pool(&config.database_url).await?;
    let pool = Arc::new(pool);

    let handler = tools::TimelordTools::new(pool);

    match config.transport.as_str() {
        "stdio" => {
            tracing::info!("starting MCP server on stdio");
            let transport = rmcp::transport::stdio();
            let server = handler.serve(transport).await?;
            server.waiting().await?;
        }
        "http" => {
            // HTTP mode: serve health endpoint only (MCP stdio is the primary transport)
            let app = Router::new().route("/healthz", get(healthz));
            let addr = format!("0.0.0.0:{}", config.http_port);
            tracing::info!(addr = %addr, "timelord-mcp HTTP listening (stdio is primary MCP transport)");
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            axum::serve(listener, app).await?;
        }
        other => {
            anyhow::bail!("Unknown MCP_TRANSPORT: {other} (expected 'stdio' or 'http')");
        }
    }

    Ok(())
}
