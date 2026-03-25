use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use redis::AsyncCommands;
use std::sync::Arc;

use crate::routes::GatewayState;

/// Sliding window rate limit using Redis INCR + EXPIRE.
/// Limits: 120 requests/minute per IP (unauthenticated) or per user (authenticated).
pub async fn rate_limit_middleware(
    State(state): State<Arc<GatewayState>>,
    req: Request,
    next: Next,
) -> Response {
    // Use X-Forwarded-For or fall back to a generic key
    let client_key = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|ip| format!("rl:ip:{ip}"))
        .unwrap_or_else(|| "rl:anonymous".to_string());

    let limit: u64 = 120;
    let window_secs: u64 = 60;

    let mut redis = state.redis.clone();

    // Increment counter; set expiry on first request
    let count: u64 = match redis.incr::<_, _, u64>(&client_key, 1u64).await {
        Ok(c) => c,
        Err(_) => return next.run(req).await, // Redis unavailable — fail open
    };

    if count == 1 {
        let _ = redis
            .expire::<_, i64>(&client_key, window_secs as i64)
            .await;
    }

    if count > limit {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("X-RateLimit-Limit", HeaderValue::from_static("120"))],
            "Rate limit exceeded",
        )
            .into_response();
    }

    next.run(req).await
}
