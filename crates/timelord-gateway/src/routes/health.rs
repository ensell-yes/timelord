use axum::Json;
use serde_json::{json, Value};

pub async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "timelord-gateway" }))
}

pub async fn root() -> Json<Value> {
    Json(json!({
        "service": "timelord",
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
