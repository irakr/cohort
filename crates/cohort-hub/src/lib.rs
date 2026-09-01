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
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Newest mirrored frame for one window grant. Memory only, never persisted.
#[derive(Clone)]
pub struct Frame {
    pub bytes: Vec<u8>,
    pub at: String,
}

pub type FrameStore = Arc<Mutex<HashMap<(String, i64), Frame>>>;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<Config>,
    pub http: reqwest::Client,
    /// In-memory frame relay for window mirroring (see routes::frames).
    pub frames: FrameStore,
}

pub fn build_router(pool: SqlitePool, config: Config) -> Router {
    let origins: Vec<HeaderValue> = config
        .allowed_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::HeaderName::from_static("x-user-id")]);

    let state = AppState {
        pool,
        config: Arc::new(config),
        http: reqwest::Client::new(),
        frames: Arc::new(Mutex::new(HashMap::new())),
    };
    routes::api_router()
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
