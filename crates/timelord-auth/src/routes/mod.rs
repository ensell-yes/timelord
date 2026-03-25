pub mod health;
pub mod oauth_google;
pub mod oauth_microsoft;
pub mod session;

use axum::{routing::delete, routing::get, routing::post, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::services::AppState;

pub async fn serve(state: Arc<AppState>) -> anyhow::Result<()> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Health
        .route("/healthz", get(health::healthz))
        // Google OAuth
        .route("/auth/google", get(oauth_google::start))
        .route("/auth/google/callback", get(oauth_google::callback))
        // Microsoft OAuth
        .route("/auth/microsoft", get(oauth_microsoft::start))
        .route("/auth/microsoft/callback", get(oauth_microsoft::callback))
        // Session management
        .route("/auth/refresh", post(session::refresh))
        .route("/auth/logout", delete(session::logout))
        .route("/auth/me", get(session::me))
        .route("/auth/org/switch", post(session::switch_org))
        // JWKS
        .route("/.well-known/jwks.json", get(session::jwks))
        .layer(cors)
        .with_state(state.clone());

    let addr = format!("0.0.0.0:{}", state.config.http_port);
    tracing::info!(addr = %addr, "auth HTTP server listening");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
