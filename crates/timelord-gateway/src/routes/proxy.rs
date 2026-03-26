use axum::{
    body::Body,
    extract::{Request, State},
    response::Response,
};
use std::sync::Arc;

use crate::routes::GatewayState;
use timelord_common::error::AppError;

/// Route a request to the appropriate upstream service based on path prefix.
pub async fn proxy_handler(
    State(state): State<Arc<GatewayState>>,
    req: Request,
) -> Result<Response, AppError> {
    let path = req.uri().path().to_string();
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();

    let upstream_base = if path.starts_with("/auth/")
        || path.starts_with("/.well-known/")
        || path.starts_with("/admin/")
        || path.starts_with("/setup/")
    {
        &state.config.auth_service_http_url
    } else {
        &state.config.calendar_service_http_url
    };

    let upstream_url = format!("{upstream_base}{path}{query}");

    let method = reqwest::Method::from_bytes(req.method().as_str().as_bytes())
        .map_err(|e| AppError::internal(format!("Invalid method: {e}")))?;

    let mut upstream_req = state.http_client.request(method, &upstream_url);

    // Forward all headers except Host
    for (name, value) in req.headers() {
        if name != "host" {
            if let Ok(val_str) = value.to_str() {
                upstream_req = upstream_req.header(name.as_str(), val_str);
            }
        }
    }

    // Forward body
    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|e| AppError::internal(format!("Body read: {e}")))?;

    if !body_bytes.is_empty() {
        upstream_req = upstream_req.body(body_bytes);
    }

    let upstream_resp = upstream_req
        .send()
        .await
        .map_err(|e| AppError::internal(format!("Upstream request failed: {e}")))?;

    // Build downstream response
    let status = axum::http::StatusCode::from_u16(upstream_resp.status().as_u16())
        .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);

    let mut builder = Response::builder().status(status);

    for (name, value) in upstream_resp.headers() {
        // Skip hop-by-hop headers
        let name_str = name.as_str();
        if matches!(
            name_str,
            "transfer-encoding" | "connection" | "keep-alive" | "te" | "trailers" | "upgrade"
        ) {
            continue;
        }
        if let Ok(val) = axum::http::HeaderValue::from_bytes(value.as_bytes()) {
            builder = builder.header(name_str, val);
        }
    }

    let resp_body = upstream_resp
        .bytes()
        .await
        .map_err(|e| AppError::internal(format!("Upstream body: {e}")))?;

    builder
        .body(Body::from(resp_body))
        .map_err(|e| AppError::internal(format!("Response build: {e}")))
}
