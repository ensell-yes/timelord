pub mod health;
pub mod proxy;

use axum::{middleware, routing::any, routing::get, Router};
use jsonwebtoken::DecodingKey;
use redis::aio::ConnectionManager;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    config::Config,
    middleware::{
        auth::auth_middleware, rate_limit::rate_limit_middleware, request_id::request_id_middleware,
    },
};

#[derive(Clone)]
pub struct GatewayState {
    pub config: Arc<Config>,
    pub http_client: reqwest::Client,
    pub redis: ConnectionManager,
    pub decoding_key: Arc<DecodingKey>,
}

pub async fn serve(config: Config) -> anyhow::Result<()> {
    let public_pem = config.jwt_public_key_pem.replace("\\n", "\n");
    let decoding_key = DecodingKey::from_rsa_pem(public_pem.as_bytes())
        .map_err(|e| anyhow::anyhow!("Invalid JWT_PUBLIC_KEY_PEM: {e}"))?;

    let redis_client = redis::Client::open(config.redis_url.as_str())?;
    let redis = ConnectionManager::new(redis_client).await?;

    let state = Arc::new(GatewayState {
        config: Arc::new(config.clone()),
        http_client: reqwest::Client::new(),
        redis,
        decoding_key: Arc::new(decoding_key),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/healthz", get(health::healthz))
        // Proxy all other traffic
        .route("/{*path}", any(proxy::proxy_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn(request_id_middleware))
        .layer(cors)
        .with_state(state.clone());

    let addr = format!("0.0.0.0:{}", config.port);

    if let (Some(cert_path), Some(key_path)) = (&config.tls_cert_path, &config.tls_key_path) {
        let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_path, key_path)
            .await?;
        tracing::info!(addr = %addr, "gateway listening (TLS)");
        axum_server::bind_rustls(addr.parse()?, tls_config)
            .serve(app.into_make_service())
            .await?;
    } else {
        tracing::info!(addr = %addr, "gateway listening");
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
    }

    Ok(())
}
