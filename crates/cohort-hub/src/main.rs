use cohort_hub::{build_router, config::Config, db, seed};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cohort_hub=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env();
    let pool = db::pool(&config.db).await?;
    seed::seed(&pool).await?;

    let bind = config.bind.clone();
    let app = build_router(pool, config);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "cohort-hub listening");
    axum::serve(listener, app).await?;
    Ok(())
}
