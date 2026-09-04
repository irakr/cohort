use std::path::PathBuf;

/// Hub configuration, read from the environment. The hub is deployed to a
/// commonly accessible server (e.g. a company data centre), so bind address
/// and allowed origins are configurable. The hub calls no model: assistant
/// settings live on each machine's app.
#[derive(Debug, Clone)]
pub struct Config {
    /// `COHORT_BIND`, default `127.0.0.1:7400`. Use `0.0.0.0:7400` when serving a LAN.
    pub bind: String,
    /// `COHORT_DB`. Default: `<OS config dir>/cohort/config/cohort.db`
    /// (falls back to `./cohort.db` when no config dir exists, e.g. minimal
    /// containers). Tests use `sqlite::memory:`.
    pub db: String,
    /// `COHORT_ALLOWED_ORIGINS`, CSV. Defaults cover the Vite dev server and
    /// the Tauri webview origins on macOS/Windows.
    pub allowed_origins: Vec<String>,
    /// `COHORT_LOG_DIR` override for the log directory. Default:
    /// `<OS config dir>/cohort/logs`. None here means "use the default".
    pub log_dir: Option<PathBuf>,
}

impl Config {
    pub fn from_env() -> Self {
        let origins = std::env::var("COHORT_ALLOWED_ORIGINS").unwrap_or_else(|_| {
            "http://localhost:1420,tauri://localhost,http://tauri.localhost".into()
        });
        let db = std::env::var("COHORT_DB").unwrap_or_else(|_| {
            cohort_dirs::config_dir()
                .map(|dir| dir.join("cohort.db").to_string_lossy().to_string())
                .unwrap_or_else(|| "cohort.db".into())
        });
        Self {
            bind: std::env::var("COHORT_BIND").unwrap_or_else(|_| "127.0.0.1:7400".into()),
            db,
            allowed_origins: origins
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            log_dir: std::env::var_os("COHORT_LOG_DIR").map(PathBuf::from),
        }
    }
}
