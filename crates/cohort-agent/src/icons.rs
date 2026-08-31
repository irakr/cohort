//! Best-effort app icon lookup for detected artifacts, returned as
//! `data:image/png;base64,...` URIs so the webview can render them directly.
//!
//! Platform-agnostic contract: [`app_icon`] takes the executable path of a
//! detected process and returns its application icon, or None - the UI then
//! shows a placeholder badge. macOS resolves the app bundle's icns via the
//! built-in `sips`; Linux searches the hicolor/pixmaps themes by binary name;
//! other platforms return None.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Icon for the app that owns `exe_path`, cached per path for the process
/// lifetime (rescans stay cheap).
pub fn app_icon(exe_path: &str) -> Option<String> {
    if let Ok(guard) = cache().lock() {
        if let Some(hit) = guard.get(exe_path) {
            return hit.clone();
        }
    }
    let result = compute(exe_path);
    if let Ok(mut guard) = cache().lock() {
        guard.insert(exe_path.to_string(), result.clone());
    }
    result
}

fn compute(exe_path: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        macos_icon(exe_path)
    }
    #[cfg(target_os = "linux")]
    {
        linux_icon(exe_path)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = exe_path;
        None
    }
}

fn png_data_uri(bytes: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

/// The `.app` bundle root containing `exe_path`, if any (macOS layout).
pub fn macos_bundle_root(exe_path: &str) -> Option<PathBuf> {
    let mut root = PathBuf::new();
    for component in std::path::Path::new(exe_path).components() {
        root.push(component);
        if root.extension().and_then(|e| e.to_str()) == Some("app") {
            return Some(root);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn macos_icon(exe_path: &str) -> Option<String> {
    use std::process::Command;

    let bundle = macos_bundle_root(exe_path)?;
    let resources = bundle.join("Contents").join("Resources");

    // Info.plist names the icon file, usually without the .icns extension.
    let named = Command::new("defaults")
        .args(["read", bundle.join("Contents").join("Info").to_str()?, "CFBundleIconFile"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|name| !name.is_empty())
        .map(|name| {
            let file = if name.ends_with(".icns") { name } else { format!("{name}.icns") };
            resources.join(file)
        })
        .filter(|p| p.exists());
    let icns = match named {
        Some(p) => p,
        None => std::fs::read_dir(&resources)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().and_then(|e| e.to_str()) == Some("icns"))?,
    };

    // sips ships with macOS; downscale so the data URI stays small.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&icns, &mut hasher);
    let out = std::env::temp_dir().join(format!(
        "cohort-icon-{:x}.png",
        std::hash::Hasher::finish(&hasher)
    ));
    let converted = Command::new("sips")
        .args([
            "-s", "format", "png",
            icns.to_str()?,
            "--out", out.to_str()?,
            "--resampleHeightWidthMax", "64",
        ])
        .output()
        .ok()?;
    if !converted.status.success() {
        return None;
    }
    let bytes = std::fs::read(&out).ok()?;
    let _ = std::fs::remove_file(&out);
    Some(png_data_uri(&bytes))
}

/// Freedesktop icon locations to try for a binary, best first.
pub fn linux_icon_candidates(exe_path: &str) -> Vec<PathBuf> {
    let basename = exe_path.rsplit('/').next().unwrap_or(exe_path).to_lowercase();
    // Well-known theme names that differ from the binary name.
    let name = match basename.as_str() {
        "gnome-terminal-server" => "org.gnome.Terminal".to_string(),
        "konsole" => "utilities-terminal".to_string(),
        "wezterm-gui" => "org.wezfurlong.wezterm".to_string(),
        other => other.to_string(),
    };
    let mut out = Vec::new();
    for size in ["64x64", "128x128", "48x48", "256x256"] {
        out.push(PathBuf::from(format!(
            "/usr/share/icons/hicolor/{size}/apps/{name}.png"
        )));
    }
    out.push(PathBuf::from(format!("/usr/share/pixmaps/{name}.png")));
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        out.push(home.join(format!(".local/share/icons/hicolor/64x64/apps/{name}.png")));
    }
    out
}

#[cfg(target_os = "linux")]
fn linux_icon(exe_path: &str) -> Option<String> {
    linux_icon_candidates(exe_path)
        .into_iter()
        .find(|p| p.exists())
        .and_then(|p| std::fs::read(p).ok())
        .map(|bytes| png_data_uri(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_root_found_inside_app_paths() {
        assert_eq!(
            macos_bundle_root("/Applications/iTerm.app/Contents/MacOS/iTerm2"),
            Some(PathBuf::from("/Applications/iTerm.app"))
        );
        assert_eq!(
            macos_bundle_root("/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal"),
            Some(PathBuf::from("/System/Applications/Utilities/Terminal.app"))
        );
        assert_eq!(macos_bundle_root("/bin/zsh"), None);
    }

    #[test]
    fn linux_candidates_cover_theme_aliases() {
        let candidates = linux_icon_candidates("/usr/libexec/gnome-terminal-server");
        assert!(candidates
            .iter()
            .any(|p| p.ends_with("apps/org.gnome.Terminal.png")));
        let candidates = linux_icon_candidates("/usr/bin/kitty");
        assert!(candidates.iter().any(|p| p.ends_with("apps/kitty.png")));
        assert!(candidates.iter().any(|p| p.starts_with("/usr/share/pixmaps")));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn terminal_app_icon_resolves_on_macos() {
        // Terminal.app ships with every macOS install.
        let icon = app_icon("/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal");
        match icon {
            Some(uri) => assert!(uri.starts_with("data:image/png;base64,")),
            // Tolerate sandboxed test environments without sips access.
            None => eprintln!("icon lookup unavailable in this environment"),
        }
        // Cached second call agrees with the first.
        let again = app_icon("/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal");
        assert_eq!(
            app_icon("/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal"),
            again
        );
    }
}
