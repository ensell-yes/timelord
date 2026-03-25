/// Solver stub — verifies good_lp dependency compiles.
/// Phase 3 will implement the full ILP optimization engine.
use axum::{routing::get, Json, Router};
use dotenvy::dotenv;
use serde_json::{json, Value};
use timelord_common::{config::env_parse, telemetry};

// Verify good_lp compiles (clarabel pure-rust feature)
#[allow(unused_imports)]
use good_lp::*;

pub async fn healthz() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "timelord-solver",
        "solver_backend": "clarabel (pure-rust)",
        "phase": "stub — ILP engine in Phase 3"
    }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    telemetry::init("timelord-solver");

    let port: u16 = env_parse("SOLVER_HTTP_PORT", 3004);
    let app = Router::new().route("/healthz", get(healthz));

    tracing::info!(port = port, "timelord-solver stub listening");
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
