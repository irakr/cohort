//! Where this machine's model settings live:
//! `<OS config dir>/cohort/config/assistant.json`, owner-readable only.
//! The key is in that file, the way ~/.aws/credentials keeps one. No file
//! means no assistant, and every feature degrades to its empty state.

use cohort_llm::LlmConfig;
use std::io;
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "assistant.json";

pub fn path() -> Option<PathBuf> {
    cohort_dirs::config_dir().map(|dir| dir.join(FILE_NAME))
}

/// None when there is no file, or it does not parse. Either way the
/// assistant is simply not configured.
pub fn load() -> Option<LlmConfig> {
    load_from(&path()?)
}

pub fn load_from(path: &Path) -> Option<LlmConfig> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save(cfg: &LlmConfig) -> io::Result<()> {
    let path = path().ok_or_else(|| io::Error::other("no OS config directory"))?;
    save_to(&path, cfg)
}

pub fn save_to(path: &Path, cfg: &LlmConfig) -> io::Result<()> {
    let json = serde_json::to_string_pretty(cfg)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    io::Write::write_all(&mut file, json.as_bytes())?;
    #[cfg(unix)]
    {
        // The mode above only applies on create; an existing file keeps
        // whatever it had, so pin it either way.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Forget the configuration. Not an error when there was none.
pub fn clear() -> io::Result<()> {
    if let Some(path) = path() {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cohort_llm::Protocol;

    fn sample() -> LlmConfig {
        LlmConfig {
            protocol: Protocol::OpenaiCompatible,
            base_url: "http://10.0.0.7:8000/v1".into(),
            api_key: None,
            model: "qwen3".into(),
        }
    }

    #[test]
    fn round_trips_and_is_owner_only() {
        let dir = std::env::temp_dir().join(format!("cohort-assistant-cfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(FILE_NAME);

        assert!(load_from(&file).is_none(), "no file means not configured");
        save_to(&file, &sample()).unwrap();
        assert_eq!(load_from(&file), Some(sample()));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }

        // Overwriting keeps the mode and replaces the content.
        let mut changed = sample();
        changed.model = "llama3".into();
        save_to(&file, &changed).unwrap();
        assert_eq!(load_from(&file).unwrap().model, "llama3");

        std::fs::write(&file, "not json").unwrap();
        assert!(load_from(&file).is_none(), "a broken file is not configured");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
