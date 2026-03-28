use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, Algorithm, Validation};
use redis::AsyncCommands;
use std::sync::Arc;

use timelord_common::{auth_claims::Claims, error::AppError};

use crate::routes::GatewayState;

/// Public path prefixes that bypass JWT validation.
/// Note: do not include `"/"` here — `path.starts_with("/")` is true for every path.
static PUBLIC_PATHS: &[&str] = &[
    "/healthz",
    "/favicon.ico",
    "/auth/google",
    "/auth/microsoft",
    "/auth/login",
    "/.well-known/jwks.json",
    "/setup/status",
];

fn is_public(path: &str) -> bool {
    path == "/"
        || PUBLIC_PATHS.iter().any(|p| path.starts_with(p))
        || path.starts_with("/auth/google/callback")
        || path.starts_with("/auth/microsoft/callback")
}

pub async fn auth_middleware(
    State(state): State<Arc<GatewayState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let path = req.uri().path().to_string();

    if is_public(&path) {
        return Ok(next.run(req).await);
    }

    // Extract Bearer token
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?
        .to_string();

    // Verify JWT locally with public key
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;

    let token_data = decode::<Claims>(&token, &state.decoding_key, &validation)
        .map_err(|_| AppError::Unauthorized)?;

    let claims = token_data.claims;

    // Check jti revocation denylist in Redis
    let jti_key = format!("jti:{}", claims.jti);
    let mut redis = state.redis.clone();
    let revoked: bool = redis
        .exists(&jti_key)
        .await
        .map_err(|e| AppError::internal(format!("Redis jti check: {e}")))?;

    if revoked {
        return Err(AppError::Unauthorized);
    }

    // Inject claims into request extensions for downstream handlers
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}
