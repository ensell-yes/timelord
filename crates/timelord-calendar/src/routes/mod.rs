pub mod calendars;
pub mod events;
pub mod health;
pub mod provider;

use axum::{routing::get, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::services::AppState;

pub async fn serve(state: Arc<AppState>) -> anyhow::Result<()> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/healthz", get(health::healthz))
        .route(
            "/api/v1/calendars",
            get(calendars::list).post(calendars::create),
        )
        .route(
            "/api/v1/calendars/:id",
            get(calendars::get_one).delete(calendars::delete_one),
        )
        .route(
            "/api/v1/calendars/:cal_id/events",
            get(events::list).post(events::create),
        )
        .route(
            "/api/v1/events/:id",
            get(events::get_one).delete(events::delete_one),
        )
        // Provider calendar listing (fetches live from Google/Microsoft)
        .route(
            "/api/v1/provider/calendars",
            get(provider::list_provider_calendars),
        )
        .layer(cors)
        .with_state(state.clone());

    let addr = format!("0.0.0.0:{}", state.config.http_port);
    tracing::info!(addr = %addr, "calendar HTTP server listening");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
