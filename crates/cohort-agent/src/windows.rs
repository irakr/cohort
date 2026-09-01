//! Application-window enumeration and capture (mirroring plan, M3).
//!
//! macOS: CoreGraphics window list + the system `screencapture -l` for JPEG
//! frames (requires the one-time Screen Recording permission).
//! Linux/X11: `wmctrl -lx` to list and ImageMagick `import` to capture
//! (both in the bootstrap package list). Wayland is not supported in this
//! milestone.
//!
//! A window's identity travels inside the grant target as
//! `w-<id>|<app>: <title>`; the capture side parses the id back out.

use crate::ArtifactCandidate;

/// The window id encoded in a grant target (`w-<id>|...`).
pub fn window_id_from_target(target: &str) -> Option<String> {
    let head = target.split('|').next()?;
    let id = head.strip_prefix("w-")?;
    let valid = !id.is_empty()
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == 'x');
    valid.then(|| id.to_string())
}

/// Build the grant target for a catalog window item.
pub fn target_for(item: &ArtifactCandidate) -> String {
    format!("{}|{}: {}", item.id, item.label, item.detail)
}

fn candidate(id: String, app: String, title: String) -> ArtifactCandidate {
    ArtifactCandidate {
        id: format!("w-{id}"),
        kind: "window".into(),
        badge: "WIN".into(),
        label: app,
        detail: if title.is_empty() { "(untitled window)".into() } else { title },
        warn: false,
        icon: None,
        pid: None,
    }
}

/// Parse `wmctrl -lx` output: `<id> <desktop> <class> <host> <title...>`.
/// Sticky/dock windows (desktop -1) are skipped.
pub fn parse_wmctrl(output: &str) -> Vec<ArtifactCandidate> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let id = parts.next()?.to_string();
            let desktop = parts.next()?;
            let class = parts.next()?.to_string();
            let _host = parts.next()?;
            let title = parts.collect::<Vec<_>>().join(" ");
            if desktop == "-1" || !id.starts_with("0x") {
                return None;
            }
            let app = class.rsplit('.').next().unwrap_or(&class).to_string();
            Some(candidate(id, app, title))
        })
        .collect()
}

#[cfg(target_os = "macos")]
mod platform {
    use super::candidate;
    use crate::ArtifactCandidate;
    use core_foundation::array::CFArray;
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGWindowListCopyWindowInfo(
            option: u32,
            relative_to: u32,
        ) -> core_foundation::array::CFArrayRef;
    }
    const ON_SCREEN_ONLY: u32 = 1 << 0;
    const EXCLUDE_DESKTOP: u32 = 1 << 4;

    fn get_string(d: &CFDictionary<CFString, CFType>, key: &str) -> Option<String> {
        d.find(CFString::new(key))
            .and_then(|v| v.downcast::<CFString>())
            .map(|s| s.to_string())
    }

    fn get_i64(d: &CFDictionary<CFString, CFType>, key: &str) -> Option<i64> {
        d.find(CFString::new(key))
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_i64())
    }

    pub fn list_windows() -> Vec<ArtifactCandidate> {
        let array: CFArray<CFDictionary<CFString, CFType>> = unsafe {
            let raw = CGWindowListCopyWindowInfo(ON_SCREEN_ONLY | EXCLUDE_DESKTOP, 0);
            if raw.is_null() {
                return Vec::new();
            }
            TCFType::wrap_under_create_rule(raw)
        };
        let mut out = Vec::new();
        for entry in array.iter() {
            // Layer 0 = normal application windows (menus/overlays sit higher).
            if get_i64(&entry, "kCGWindowLayer") != Some(0) {
                continue;
            }
            let Some(id) = get_i64(&entry, "kCGWindowNumber") else { continue };
            let app = get_string(&entry, "kCGWindowOwnerName").unwrap_or_default();
            if app.is_empty() {
                continue;
            }
            // Window titles require the Screen Recording permission; without
            // it this is empty and shows as "(untitled window)".
            let title = get_string(&entry, "kCGWindowName").unwrap_or_default();
            out.push(candidate(id.to_string(), app, title));
        }
        out
    }

    pub fn capture(window_id: &str) -> Result<Vec<u8>, String> {
        let path = std::env::temp_dir().join(format!("cohort-frame-{window_id}.jpg"));
        let output = std::process::Command::new("screencapture")
            .args(["-x", "-o", "-l", window_id, "-t", "jpg"])
            .arg(&path)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "screencapture failed (Screen Recording permission?): {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&path);
        Ok(bytes)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use crate::ArtifactCandidate;

    pub fn list_windows() -> Vec<ArtifactCandidate> {
        std::process::Command::new("wmctrl")
            .arg("-lx")
            .output()
            .map(|o| super::parse_wmctrl(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    }

    pub fn capture(window_id: &str) -> Result<Vec<u8>, String> {
        let path = std::env::temp_dir().join(format!(
            "cohort-frame-{}.jpg",
            window_id.trim_start_matches("0x")
        ));
        let output = std::process::Command::new("import")
            .args(["-window", window_id, "-silent"])
            .arg(format!("jpeg:{}", path.display()))
            .output()
            .map_err(|e| format!("ImageMagick 'import' not available: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "window capture failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&path);
        Ok(bytes)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod platform {
    use crate::ArtifactCandidate;

    pub fn list_windows() -> Vec<ArtifactCandidate> {
        Vec::new()
    }

    pub fn capture(_window_id: &str) -> Result<Vec<u8>, String> {
        Err("window capture is not supported on this platform".into())
    }
}

/// Open application windows on this machine (empty on unsupported setups).
pub fn list_windows() -> Vec<ArtifactCandidate> {
    platform::list_windows()
}

/// Capture one JPEG frame of the window named by a grant target.
pub fn capture_target(target: &str) -> Result<Vec<u8>, String> {
    let id = window_id_from_target(target)
        .ok_or_else(|| format!("no window id in target: {target}"))?;
    platform::capture(&id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_round_trip() {
        let item = candidate("771".into(), "Google Chrome".into(), "Grafana - payments".into());
        let target = target_for(&item);
        assert_eq!(target, "w-771|Google Chrome: Grafana - payments");
        assert_eq!(window_id_from_target(&target).as_deref(), Some("771"));
        assert_eq!(window_id_from_target("w-0x3400007|Code: cohort").as_deref(), Some("0x3400007"));
        assert_eq!(window_id_from_target("Google Chrome: Grafana"), None);
        assert_eq!(window_id_from_target("w-77; rm -rf /|x"), None);
    }

    #[test]
    fn wmctrl_parsing() {
        let out = "\
0x03400007  0 code.Code            spark-b4de Cohort - Visual Studio Code
0x02c00003 -1 xfce4-panel.Xfce4-panel spark-b4de xfce4-panel
0x04a00001  1 Navigator.firefox    spark-b4de Grafana - Mozilla Firefox
";
        let windows = parse_wmctrl(out);
        assert_eq!(windows.len(), 2); // the sticky panel is skipped
        assert_eq!(windows[0].label, "Code");
        assert_eq!(windows[0].detail, "Cohort - Visual Studio Code");
        assert_eq!(windows[0].id, "w-0x03400007");
        assert_eq!(windows[1].label, "firefox");
        assert_eq!(windows[1].kind, "window");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_enumeration_does_not_panic() {
        let windows = list_windows();
        for w in &windows {
            assert!(w.id.starts_with("w-"));
            assert_eq!(w.kind, "window");
        }
    }
}
