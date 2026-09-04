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

/// A hostname other machines can resolve. A bare short name (Linux
/// `hostname` gives no domain) is only known locally, so suggest its mDNS
/// form; a name that already carries a domain is left alone.
pub fn reachable_host(raw: &str) -> String {
    if raw.contains('.') {
        raw.to_string()
    } else {
        format!("{raw}.local")
    }
}

/// Suggested connection target for this machine: user@hostname. Only a
/// suggestion - the owner edits it at approval time.
pub fn target_suggestion() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".into());
    let host = std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|h| !h.is_empty())
        .map(|h| reachable_host(&h))
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

/// Terminal emulators to try on Linux, the running desktop's own first.
/// The x-terminal-emulator alternatives symlink comes late: it names what
/// the distribution installed, not what the current session can run (on an
/// XFCE session it may point at gnome-terminal, whose D-Bus activation
/// hangs without a GNOME session).
pub fn terminal_candidates(desktop: Option<&str>) -> Vec<&'static str> {
    let desktop = desktop.unwrap_or("").to_ascii_lowercase();
    let preferred = if desktop.contains("xfce") {
        Some("xfce4-terminal")
    } else if desktop.contains("kde") {
        Some("konsole")
    } else if desktop.contains("gnome") || desktop.contains("unity") || desktop.contains("ubuntu") {
        Some("gnome-terminal")
    } else {
        None
    };
    let mut out: Vec<&'static str> = Vec::new();
    out.extend(preferred);
    for term in ["gnome-terminal", "konsole", "xfce4-terminal", "x-terminal-emulator", "xterm"] {
        if !out.contains(&term) {
            out.push(term);
        }
    }
    out
}

/// The argument form that runs `command` inside the given emulator; they
/// differ: xfce4-terminal takes the whole command line as one -e string,
/// gnome-terminal wants `-- cmd args`, the rest take xterm-style `-e cmd
/// args`. `command` is shell-safe by construction (see valid_target).
pub fn terminal_args(program: &str, command: &str) -> Vec<String> {
    match program {
        "xfce4-terminal" => vec!["-e".into(), command.into()],
        "gnome-terminal" => {
            let mut args = vec!["--".to_string()];
            args.extend(command.split_whitespace().map(str::to_string));
            args
        }
        _ => {
            let mut args = vec!["-e".to_string()];
            args.extend(command.split_whitespace().map(str::to_string));
            args
        }
    }
}

/// Open the system terminal running ssh to the target.
pub fn open_terminal_ssh(target: &str) -> Result<(), String> {
    let command = ssh_command(target).ok_or_else(|| format!("invalid ssh target: {target}"))?;
    #[cfg(target_os = "macos")]
    {
        // When Terminal is not yet running, launching it opens its startup
        // window; run the command in that window instead of a second one.
        let script = format!(
            "if application \"Terminal\" is running then\n\
             tell application \"Terminal\" to do script \"{command}\"\n\
             else\n\
             tell application \"Terminal\" to do script \"{command}\" in window 1\n\
             end if\n\
             activate application \"Terminal\""
        );
        // Wait for osascript: it returns quickly, and a failure (e.g. the
        // Automation permission for controlling Terminal was denied) only
        // shows in its exit status and stderr.
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "could not open Terminal: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        use std::io::Read;
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").ok();
        let mut failures: Vec<String> = Vec::new();
        for program in terminal_candidates(desktop.as_deref()) {
            let mut child = match std::process::Command::new(program)
                .args(terminal_args(program, &command))
                .stderr(std::process::Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(_) => continue, // not installed
            };
            // A terminal that opens keeps running (or, like gnome-terminal
            // on GNOME, hands off and exits 0). One given bad arguments or
            // missing session infrastructure exits non-zero within moments:
            // poll briefly so that falls through to the next candidate.
            for _ in 0..4 {
                std::thread::sleep(std::time::Duration::from_millis(300));
                match child.try_wait() {
                    Ok(Some(status)) if status.success() => return Ok(()),
                    Ok(Some(_)) => {
                        let mut stderr = String::new();
                        if let Some(mut pipe) = child.stderr.take() {
                            let _ = pipe.read_to_string(&mut stderr);
                        }
                        failures.push(format!("{program}: {}", stderr.trim()));
                        break;
                    }
                    _ => {}
                }
            }
            if child.stderr.is_some() {
                return Ok(()); // still running after the grace period
            }
        }
        if failures.is_empty() {
            Err("no terminal emulator found".into())
        } else {
            Err(format!("could not open a terminal ({})", failures.join("; ")))
        }
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

    #[test]
    fn short_hostnames_get_the_mdns_suffix() {
        assert_eq!(reachable_host("spark-b4de"), "spark-b4de.local");
        assert_eq!(reachable_host("Iraks-MacBook-Pro.local"), "Iraks-MacBook-Pro.local");
        assert_eq!(reachable_host("box.corp.example"), "box.corp.example");
    }

    #[test]
    fn candidates_prefer_the_running_desktop() {
        assert_eq!(terminal_candidates(Some("XFCE"))[0], "xfce4-terminal");
        assert_eq!(terminal_candidates(Some("ubuntu:GNOME"))[0], "gnome-terminal");
        assert_eq!(terminal_candidates(Some("KDE"))[0], "konsole");
        // Unknown desktop: no preference, generic order with the
        // alternatives symlink ahead of only the bare-X fallback.
        let unknown = terminal_candidates(None);
        assert_eq!(unknown.first(), Some(&"gnome-terminal"));
        assert_eq!(unknown.last(), Some(&"xterm"));
        // No duplicates when the preferred one is also in the generic list.
        let xfce = terminal_candidates(Some("XFCE"));
        assert_eq!(xfce.iter().filter(|t| **t == "xfce4-terminal").count(), 1);
    }

    #[test]
    fn terminal_args_match_each_emulator() {
        // xfce4-terminal: whole command line as one -e string.
        assert_eq!(
            terminal_args("xfce4-terminal", "ssh -p 2222 dev@host"),
            vec!["-e", "ssh -p 2222 dev@host"]
        );
        // gnome-terminal: `--` then the words.
        assert_eq!(
            terminal_args("gnome-terminal", "ssh dev@host"),
            vec!["--", "ssh", "dev@host"]
        );
        // xterm-style for the rest.
        assert_eq!(
            terminal_args("x-terminal-emulator", "ssh dev@host"),
            vec!["-e", "ssh", "dev@host"]
        );
    }

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
        install_key(&home, "ssh-ed25519 AAAC responder@laptop", "S-7:3").unwrap();
        install_key(&home, "ssh-ed25519 AAAC responder@laptop", "S-7:3").unwrap(); // idempotent
        let content = std::fs::read_to_string(home.join(".ssh/authorized_keys")).unwrap();
        assert_eq!(content.matches("AAAC").count(), 1);
        assert!(content.contains("cohort:S-7:3"));
        // The comment from the key file is replaced by the marker.
        assert!(!content.contains("responder@laptop"));
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
        assert!(valid_target("owner@build-host.local"));
        assert!(valid_target("u_1@10.0.0.5:2222"));
        assert!(!valid_target("owner@host; rm -rf /"));
        assert!(!valid_target("owner@host\"x"));
        assert!(!valid_target("nouser.local"));
        assert!(!valid_target("a@b:22x"));
        assert_eq!(ssh_command("u@h:2222").unwrap(), "ssh -p 2222 u@h");
        assert_eq!(ssh_command("u@h").unwrap(), "ssh u@h");
        assert!(ssh_command("u@h; whoami").is_none());
    }
}
