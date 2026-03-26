use axum::Json;
use serde_json::{json, Value};

pub async fn healthz() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "timelord-solver",
        "solver_backend": "clarabel (pure-rust)",
    }))
}
