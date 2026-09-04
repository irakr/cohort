//! Hub logging: stdout (for terminals and container runtimes) plus a file at
//! `<log dir>/hub.log`, where the log dir is `COHORT_LOG_DIR` or the shared
//! cohort namespace (`<OS config dir>/cohort/logs`, see cohort-dirs).
//!
//! The file is truncated on every start: a log covers exactly one run, so
//! what is in it always belongs to the process you are looking at.

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

    match resolve_log_dir(config).and_then(|dir| {
        let path = dir.join("hub.log");
        // Truncating open, rather than tracing_appender's rolling::never,
        // which always appends.
        let file = std::fs::File::create(&path).ok()?;
        Some((dir, file))
    }) {
        Some((dir, file)) => {
            let (writer, guard) = tracing_appender::non_blocking(file);
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
            tracing::warn!("no usable log file; logging to stdout only");
            None
        }
    }
}
