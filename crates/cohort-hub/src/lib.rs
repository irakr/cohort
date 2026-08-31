pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod llm;
pub mod logging;
pub mod routes;
pub mod seed;

use axum::http::{header, HeaderValue, Method};
use axum::Router;
use config::Config;
use sqlx::SqlitePool;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
    pub http: reqwest::Client,
}

pub fn build_router(pool: SqlitePool, config: Config) -> Router {
    let origins: Vec<HeaderValue> = config
        .allowed_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::HeaderName::from_static("x-user-id")]);

    let state = AppState {
        pool,
        config: Arc::new(config),
        http: reqwest::Client::new(),
    };
    routes::api_router()
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
