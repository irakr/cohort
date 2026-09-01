//! SSH access support for the passwordless grant flow.
//!
//! Responder side: read this machine's SSH public key so it can travel with
//! an ssh scope request; open a terminal running ssh once granted.
//! Owner side: install a responder's public key into authorized_keys, tagged
//! with a cohort marker so it can be removed later.
//!
//! NOTE: until the revoke feature lands, an installed key stays authorized
//! after the grant's TTL expires; removal is manual (delete the tagged line).

use std::path::{Path, PathBuf};

const KEY_FILES: [&str; 3] = ["id_ed25519.pub", "id_ecdsa.pub", "id_rsa.pub"];

fn ssh_dir(home: &Path) -> PathBuf {
    home.join(".ssh")
}

/// This machine's SSH public key, first of the standard key files found.
pub fn public_key(home: &Path) -> Option<String> {
    KEY_FILES.iter().find_map(|name| {
        let content = std::fs::read_to_string(ssh_dir(home).join(name)).ok()?;
        let line = content.trim();
        (!line.is_empty()).then(|| line.to_string())
    })
}

/// Suggested connection target for this machine: user@hostname.
pub fn target_suggestion() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".into());
    let host = std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "host".into());
    format!("{user}@{host}")
}

/// A single authorized_keys line: key material plus a cohort marker naming
/// the assist and request, so the grant is identifiable and removable.
pub fn authorized_line(public_key: &str, marker: &str) -> String {
    let key = public_key.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    format!("{key} cohort:{marker}")
}

/// Install a responder's public key. Idempotent: an existing line with the
/// same key material is left as-is. Creates ~/.ssh and authorized_keys with
/// owner-only permissions when missing.
pub fn install_key(home: &Path, public_key: &str, marker: &str) -> std::io::Result<()> {
    let key_material = public_key.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    if key_material.is_empty() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty public key"));
    }
    let dir = ssh_dir(home);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("authorized_keys");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.lines().any(|l| l.trim_start().starts_with(&key_material)) {
        return Ok(());
    }
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&authorized_line(public_key, marker));
    updated.push('\n');
    std::fs::write(&path, updated)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// True when the target is a plain `user@host` or `user@host:port` - the
/// only form we will pass to a shell command.
pub fn valid_target(target: &str) -> bool {
    let ok_part = |s: &str| {
        !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c))
    };
    let (user_host, port) = match target.rsplit_once(':') {
        Some((left, port)) => (left, Some(port)),
        None => (target, None),
    };
    if let Some(p) = port {
        if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }
    match user_host.split_once('@') {
        Some((user, host)) => ok_part(user) && ok_part(host),
        None => false,
    }
}

/// The ssh invocation for a validated target.
pub fn ssh_command(target: &str) -> Option<String> {
    if !valid_target(target) {
        return None;
    }
    Some(match target.rsplit_once(':') {
        Some((user_host, port)) => format!("ssh -p {port} {user_host}"),
        None => format!("ssh {target}"),
    })
}

/// Open the system terminal running ssh to the target.
pub fn open_terminal_ssh(target: &str) -> Result<(), String> {
    let command = ssh_command(target).ok_or_else(|| format!("invalid ssh target: {target}"))?;
    #[cfg(target_os = "macos")]
    {
        let script = format!("tell application \"Terminal\" to do script \"{command}\"\nactivate application \"Terminal\"");
        std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        for term in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
            let spawned = std::process::Command::new(term)
                .args(["-e", "sh", "-c", &command])
                .spawn();
            if spawned.is_ok() {
                return Ok(());
            }
        }
        Err("no terminal emulator found".into())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = command;
        Err("unsupported platform".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(name: &str) -> PathBuf {
        let home = std::env::temp_dir().join(format!("cohort-ssh-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        home
    }

    #[test]
    fn reads_first_available_public_key() {
        let home = temp_home("readkey");
        assert_eq!(public_key(&home), None);
        std::fs::create_dir_all(home.join(".ssh")).unwrap();
        std::fs::write(home.join(".ssh/id_rsa.pub"), "ssh-rsa AAAB rsa@x\n").unwrap();
        std::fs::write(home.join(".ssh/id_ed25519.pub"), "ssh-ed25519 AAAC ed@x\n").unwrap();
        // ed25519 wins (listed first).
        assert_eq!(public_key(&home).unwrap(), "ssh-ed25519 AAAC ed@x");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn installs_key_once_with_marker_and_perms() {
        let home = temp_home("install");
        install_key(&home, "ssh-ed25519 AAAC priya@laptop", "S-2412:7").unwrap();
        install_key(&home, "ssh-ed25519 AAAC priya@laptop", "S-2412:7").unwrap(); // idempotent
        let content = std::fs::read_to_string(home.join(".ssh/authorized_keys")).unwrap();
        assert_eq!(content.matches("AAAC").count(), 1);
        assert!(content.contains("cohort:S-2412:7"));
        // The comment from the key file is replaced by the marker.
        assert!(!content.contains("priya@laptop"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(home.join(".ssh/authorized_keys")).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn target_validation_blocks_injection() {
        assert!(valid_target("alex@spark-b4de.local"));
        assert!(valid_target("u_1@10.0.0.5:2222"));
        assert!(!valid_target("alex@host; rm -rf /"));
        assert!(!valid_target("alex@host\"x"));
        assert!(!valid_target("nouser.local"));
        assert!(!valid_target("a@b:22x"));
        assert_eq!(ssh_command("u@h:2222").unwrap(), "ssh -p 2222 u@h");
        assert_eq!(ssh_command("u@h").unwrap(), "ssh u@h");
        assert!(ssh_command("u@h; whoami").is_none());
    }
}
