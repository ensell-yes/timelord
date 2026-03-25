use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

pub static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// Inject a unique X-Request-Id header if not already present.
pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    if !req.headers().contains_key(&X_REQUEST_ID) {
        let id = Uuid::new_v4().to_string();
        if let Ok(val) = HeaderValue::from_str(&id) {
            req.headers_mut().insert(&X_REQUEST_ID, val);
        }
    }
    next.run(req).await
}
