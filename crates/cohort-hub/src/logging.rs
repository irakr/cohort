//! Hub logging: stdout (for terminals and container runtimes) plus a file at
//! `<log dir>/hub.log`, where the log dir is `COHORT_LOG_DIR` or the shared
//! cohort namespace (`<OS config dir>/cohort/logs`, see cohort-dirs).

use crate::config::Config;
use std::path::PathBuf;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

fn resolve_log_dir(config: &Config) -> Option<PathBuf> {
    match &config.log_dir {
        Some(dir) => {
            std::fs::create_dir_all(dir).ok()?;
            Some(dir.clone())
        }
        None => cohort_dirs::logs_dir(),
    }
}

/// Initializes tracing. The returned guard must stay alive for the process
/// lifetime so buffered file output is flushed on shutdown.
pub fn init(config: &Config) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "cohort_hub=info,tower_http=info".into());

    match resolve_log_dir(config) {
        Some(dir) => {
            let appender = tracing_appender::rolling::never(&dir, "hub.log");
            let (writer, guard) = tracing_appender::non_blocking(appender);
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt::layer())
                .with(fmt::layer().with_ansi(false).with_writer(writer))
                .init();
            tracing::info!(path = %dir.join("hub.log").display(), "hub logging to file");
            Some(guard)
        }
        None => {
            tracing_subscriber::registry().with(filter).with(fmt::layer()).init();
            tracing::warn!("no usable log directory; logging to stdout only");
            None
        }
    }
}
