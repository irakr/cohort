//! Cohort's on-disk namespace, shared by the desktop app and the hub:
//!
//! ```text
//! <OS config dir>/cohort/
//! |- logs/     app.log, hub.log
//! |- config/   config-ish files (hub SQLite database, future settings)
//! ```
//!
//! The OS config dir is ~/Library/Application Support on macOS,
//! $XDG_CONFIG_HOME or ~/.config on Linux, and %APPDATA% on Windows.

use std::path::PathBuf;

/// `<OS config dir>/cohort`. None when the platform has no config dir
/// (e.g. a container without HOME).
pub fn base_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("cohort"))
}

fn ensured(sub: &str) -> Option<PathBuf> {
    let dir = base_dir()?.join(sub);
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// `<base>/logs`, created on first use.
pub fn logs_dir() -> Option<PathBuf> {
    ensured("logs")
}

/// `<base>/config`, created on first use.
pub fn config_dir() -> Option<PathBuf> {
    ensured("config")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_is_cohort_with_subdirs() {
        let base = base_dir().expect("config dir on dev machines");
        assert!(base.ends_with("cohort"));
        let logs = logs_dir().expect("logs dir");
        assert!(logs.ends_with("cohort/logs"));
        assert!(logs.is_dir());
        let config = config_dir().expect("config dir");
        assert!(config.ends_with("cohort/config"));
        assert!(config.is_dir());
    }
}
