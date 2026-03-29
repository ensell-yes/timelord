use axum::{extract::Request, middleware::Next, response::Response};
use timelord_common::{auth_claims::Claims, error::AppError};
use uuid::Uuid;

/// Extract claims from trusted gateway headers (X-User-Id, X-Org-Id, X-Role, X-Jti).
pub async fn claims_from_headers(mut req: Request, next: Next) -> Result<Response, AppError> {
    let headers = req.headers();

    let sub = header_uuid(headers, "X-User-Id");
    let org = header_uuid(headers, "X-Org-Id");
    let role = headers
        .get("X-Role")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let jti = header_uuid(headers, "X-Jti");

    if let (Some(sub), Some(org), Some(role), Some(jti)) = (sub, org, role, jti) {
        let now = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub,
            org,
            role,
            jti,
            iat: now,
            exp: now + 900,
        };
        req.extensions_mut().insert(claims);
    }

    Ok(next.run(req).await)
}

fn header_uuid(headers: &axum::http::HeaderMap, name: &str) -> Option<Uuid> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
}
