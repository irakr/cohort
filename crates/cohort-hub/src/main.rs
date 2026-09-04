use cohort_hub::{build_router, config::Config, db, logging};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env();
    // Keep the guard alive so file logs flush on shutdown.
    let _log_guard = logging::init(&config);

    let pool = db::pool(&config.db).await?;

    let bind = config.bind.clone();
    tracing::info!(db = %config.db, "using database");
    let app = build_router(pool, config);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "cohort-hub listening");
    axum::serve(listener, app).await?;
    Ok(())
}
