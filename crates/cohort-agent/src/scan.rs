//! Real local scanning for the context picker. Everything here observes;
//! nothing is opened, attached to, or shared without the owner's toggle.
//!
//! Sources, in line with the product plan:
//! - Terminals: shell processes attached to a tty, mapped to their terminal
//!   emulator through the process tree; each shell's working directory is the
//!   profile detail.
//! - AI agents: running agent processes (Claude Code, Cursor, Codex, Aider),
//!   plus Claude Code session activity read from ~/.claude/projects transcript
//!   mtimes and their recorded cwd.
//!   TODO(P1): Cursor session introspection (its own OTLP export), OpenAI/
//!   Codex CLI session files, and richer per-session token stats move into
//!   the detector daemon.
//! - Files/directories: the working directories the terminals and agents
//!   above are actually in, capped at MAX_DIR_SUGGESTIONS.
//!   TODO(P1): files open in editors/IDEs (needs per-editor integration).

use crate::ArtifactCandidate;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MAX_DIR_SUGGESTIONS: usize = 5;
const MAX_TERMINAL_SUGGESTIONS: usize = 6;
/// A Claude Code transcript touched within this window counts as active.
const ACTIVE_SESSION_SECS: u64 = 30 * 60;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: i32,
    pub ppid: i32,
    pub tty: String,
    /// Full command line; the first token is the executable.
    pub command: String,
}

impl ProcessInfo {
    /// Basename of the executable, lowercased, login-shell dash stripped.
    /// Note: an executable path containing spaces truncates at the first
    /// space; acceptable for the emulators and agents matched here.
    pub fn exe_basename(&self) -> String {
        let first = self.command.split_whitespace().next().unwrap_or("");
        let base = first.rsplit('/').next().unwrap_or(first);
        base.trim_start_matches('-').to_lowercase()
    }

    fn has_tty(&self) -> bool {
        !matches!(self.tty.as_str(), "" | "?" | "??" | "-")
    }
}

pub fn parse_ps(output: &str) -> Vec<ProcessInfo> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid: i32 = parts.next()?.parse().ok()?;
            let ppid: i32 = parts.next()?.parse().ok()?;
            let tty = parts.next()?.to_string();
            let command = parts.collect::<Vec<_>>().join(" ");
            if command.is_empty() {
                return None;
            }
            Some(ProcessInfo { pid, ppid, tty, command })
        })
        .collect()
}

fn list_processes() -> Vec<ProcessInfo> {
    let output = Command::new("ps").args(["-axo", "pid=,ppid=,tty=,args="]).output();
    match output {
        Ok(out) => parse_ps(&String::from_utf8_lossy(&out.stdout)),
        Err(_) => Vec::new(),
    }
}

/// Known terminal emulators by executable basename -> (display name, badge).
fn emulator_meta(basename: &str) -> Option<(&'static str, &'static str)> {
    match basename {
        "iterm2" => Some(("iTerm2", "iT")),
        "terminal" => Some(("Terminal", "Tm")),
        "kitty" => Some(("kitty", "kt")),
        "alacritty" => Some(("Alacritty", "Al")),
        "wezterm-gui" | "wezterm" => Some(("WezTerm", "Wz")),
        "tmux" => Some(("tmux", "tx")),
        "gnome-terminal-server" => Some(("GNOME Terminal", "GT")),
        "konsole" => Some(("Konsole", "Ko")),
        "xterm" => Some(("xterm", "xt")),
        _ => None,
    }
}

fn is_shell(basename: &str) -> bool {
    matches!(basename, "zsh" | "bash" | "fish" | "sh" | "dash")
}

/// Walk the process tree upward to find the shell's terminal emulator.
/// Returns (display name, badge, emulator executable path).
fn ancestor_emulator(
    procs: &[ProcessInfo],
    mut pid: i32,
) -> Option<(&'static str, &'static str, String)> {
    for _ in 0..12 {
        let proc_ = procs.iter().find(|p| p.pid == pid)?;
        if let Some((name, badge)) = emulator_meta(&proc_.exe_basename()) {
            let exe = proc_.command.split_whitespace().next().unwrap_or("").to_string();
            return Some((name, badge, exe));
        }
        if proc_.ppid <= 1 || proc_.ppid == pid {
            return None;
        }
        pid = proc_.ppid;
    }
    None
}

#[cfg(target_os = "linux")]
fn cwd_of(pid: i32) -> Option<String> {
    std::fs::read_link(format!("/proc/{pid}/cwd"))
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

#[cfg(target_os = "macos")]
fn cwd_of(pid: i32) -> Option<String> {
    let out = Command::new("lsof")
        .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|l| l.starts_with('n'))
        .map(|l| l[1..].to_string())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn cwd_of(_pid: i32) -> Option<String> {
    None
}

/// One artifact per interactive shell session, newest ttys last.
pub fn terminal_artifacts(
    procs: &[ProcessInfo],
    cwd_lookup: impl Fn(i32) -> Option<String>,
    icon_lookup: impl Fn(&str) -> Option<String>,
) -> Vec<ArtifactCandidate> {
    let mut out = Vec::new();
    let mut seen_ttys = Vec::new();
    for p in procs {
        if !p.has_tty() || !is_shell(&p.exe_basename()) || seen_ttys.contains(&p.tty) {
            continue;
        }
        seen_ttys.push(p.tty.clone());
        let (emulator, badge, icon) = match ancestor_emulator(procs, p.ppid) {
            Some((name, badge, exe)) => (name, badge, icon_lookup(&exe)),
            None => ("Shell", ">_", None),
        };
        let cwd = cwd_lookup(p.pid);
        out.push(ArtifactCandidate {
            id: format!("t-{}", p.tty.replace('/', "-")),
            kind: "terminal".into(),
            badge: badge.into(),
            label: format!("{emulator} ({})", p.tty),
            detail: cwd.unwrap_or_else(|| format!("{} session", p.exe_basename())),
            warn: false,
            icon,
            pid: Some(p.pid),
        });
        if out.len() >= MAX_TERMINAL_SUGGESTIONS {
            break;
        }
    }
    out
}

/// The tty inside a terminal label like "Shell (ttys001)" or "iTerm2 (pts/0)".
pub fn tty_from_label(label: &str) -> Option<String> {
    let start = label.rfind('(')? + 1;
    let end = label.rfind(')')?;
    let tty = label.get(start..end)?;
    let valid = !tty.is_empty()
        && tty.chars().all(|c| c.is_ascii_alphanumeric() || c == '/');
    valid.then(|| tty.to_string())
}

/// Feed lines for one terminal from its `ps -t <tty>` output: a header with
/// the working directory, then one line per running process.
pub fn activity_lines(label: &str, cwd: Option<&str>, ps_output: &str) -> Vec<String> {
    let mut out = vec![match cwd {
        Some(c) => format!("== {label} - {c}"),
        None => format!("== {label}"),
    }];
    let mut any = false;
    for line in ps_output.lines() {
        let line = line.trim();
        if !line.is_empty() {
            out.push(format!("  {line}"));
            any = true;
        }
    }
    if !any {
        out.push("  (no processes on this tty)".to_string());
    }
    out
}

/// What is actually running in the granted terminals right now. This is the
/// truthful terminal view until the detector streams real PTY output:
/// process list (pid, elapsed, command) per granted tty, refreshed by the
/// owner's app while the assist is open.
pub fn terminal_activity(labels: &[String]) -> Vec<String> {
    let procs = list_processes();
    let mut out = Vec::new();
    for label in labels {
        let Some(tty) = tty_from_label(label) else { continue };
        let ps_output = Command::new("ps")
            .args(["-t", &tty, "-o", "pid=,etime=,args="])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();
        let cwd = procs
            .iter()
            .find(|p| p.tty == tty && is_shell(&p.exe_basename()))
            .and_then(|p| cwd_of(p.pid));
        if !out.is_empty() {
            out.push(String::new());
        }
        out.extend(activity_lines(label, cwd.as_deref(), &ps_output));
    }
    out
}

/// Recent Claude Code activity: the most recently touched transcript under
/// ~/.claude/projects, and the working directory it records.
pub fn claude_activity(home: &Path) -> Option<(String, u64)> {
    let projects = home.join(".claude").join("projects");
    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
    for project in std::fs::read_dir(projects).ok()?.flatten() {
        let Ok(entries) = std::fs::read_dir(project.path()) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let Ok(modified) = entry.metadata().and_then(|m| m.modified()) else { continue };
            if newest.as_ref().map(|(_, t)| modified > *t).unwrap_or(true) {
                newest = Some((path, modified));
            }
        }
    }
    let (path, modified) = newest?;
    let age_secs = modified.elapsed().ok()?.as_secs();
    let cwd = jsonl_last_cwd(&path)?;
    Some((cwd, age_secs))
}

/// The `cwd` recorded on the last JSON line of a Claude Code transcript.
/// Reads at most the final 64 KiB of the file.
pub fn jsonl_last_cwd(path: &Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(64 * 1024);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;
    buf.lines().rev().filter(|l| !l.trim().is_empty()).find_map(|line| {
        let value: serde_json::Value = serde_json::from_str(line).ok()?;
        value.get("cwd")?.as_str().map(str::to_string)
    })
}

/// Running AI agents from the process table, enriched with Claude Code
/// session activity; agents installed but not running are listed as such.
pub fn agent_artifacts(
    procs: &[ProcessInfo],
    home: &Path,
    icon_lookup: impl Fn(&str) -> Option<String>,
) -> (Vec<ArtifactCandidate>, Option<String>) {
    // (basename, id, badge, display name)
    let known: [(&str, &str, &str, &str); 4] = [
        ("claude", "a-claude", "CC", "Claude Code"),
        ("cursor", "a-cursor", "Cu", "Cursor"),
        ("codex", "a-codex", "Cx", "Codex"),
        ("aider", "a-aider", "Ai", "Aider"),
    ];
    let mut out: Vec<ArtifactCandidate> = Vec::new();
    let mut active_cwd = None;

    let claude = claude_activity(home);
    for (basename, id, badge, label) in known {
        let running = procs.iter().find(|p| p.exe_basename() == basename);
        let detail = if basename == "claude" {
            match (&running, &claude) {
                (_, Some((cwd, age))) if *age <= ACTIVE_SESSION_SECS => {
                    active_cwd = Some(cwd.clone());
                    format!("agent session active - {cwd}")
                }
                (Some(p), _) => format!("running (pid {})", p.pid),
                (None, Some((cwd, _))) => format!("last session - {cwd}"),
                (None, None) => String::new(),
            }
        } else {
            match running {
                Some(p) => format!("running (pid {})", p.pid),
                None => String::new(),
            }
        };
        if !detail.is_empty() {
            let icon = running
                .and_then(|p| p.command.split_whitespace().next().map(str::to_string))
                .and_then(|exe| icon_lookup(&exe));
            out.push(ArtifactCandidate {
                id: id.into(),
                kind: "ai_agent".into(),
                badge: badge.into(),
                label: label.into(),
                detail,
                warn: false,
                icon,
                pid: running.map(|p| p.pid),
            });
        }
    }

    // Installed-only agents (real detection, weakest signal) fill in last.
    for installed in crate::agent_installs(home) {
        if !out.iter().any(|a| a.id == installed.id) {
            out.push(installed);
        }
    }
    (out, active_cwd)
}

/// Directories the detected terminals and agents are working in.
pub fn directory_artifacts(
    terminal_details: &[String],
    agent_cwd: Option<&str>,
    home: &Path,
) -> Vec<ArtifactCandidate> {
    let home_str = home.to_string_lossy();
    let mut paths: Vec<&str> = Vec::new();
    if let Some(cwd) = agent_cwd {
        paths.push(cwd);
    }
    for detail in terminal_details {
        paths.push(detail);
    }
    let mut out = Vec::new();
    for path in paths {
        if !path.starts_with('/') || path == "/" || path == home_str {
            continue;
        }
        if out
            .iter()
            .any(|a: &ArtifactCandidate| a.detail == path)
        {
            continue;
        }
        let label = path.rsplit('/').next().unwrap_or(path).to_string();
        out.push(ArtifactCandidate {
            id: format!("d-{}", out.len() + 1),
            kind: "file".into(),
            badge: "DIR".into(),
            label,
            detail: path.to_string(),
            warn: false,
            icon: None, // the UI renders folder/file glyphs for this kind
            pid: None,
        });
        if out.len() >= MAX_DIR_SUGGESTIONS {
            break;
        }
    }
    out
}

/// Full scan: terminals, agents, and the directories they point at.
pub fn scan(home: &Path) -> Vec<crate::ArtifactGroup> {
    let procs = list_processes();
    let terminals = terminal_artifacts(&procs, cwd_of, crate::icons::app_icon);
    let (agents, agent_cwd) = agent_artifacts(&procs, home, crate::icons::app_icon);
    let terminal_dirs: Vec<String> = terminals.iter().map(|t| t.detail.clone()).collect();
    let directories = directory_artifacts(&terminal_dirs, agent_cwd.as_deref(), home);

    let mut groups = Vec::new();
    if !terminals.is_empty() {
        groups.push(crate::ArtifactGroup { title: "Terminals".into(), items: terminals });
    }
    if !directories.is_empty() {
        groups.push(crate::ArtifactGroup { title: "Files".into(), items: directories });
    }
    if !agents.is_empty() {
        groups.push(crate::ArtifactGroup { title: "AI agents".into(), items: agents });
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    const PS_SAMPLE: &str = "\
    1     0 ??      /sbin/launchd
  400     1 ??      /Applications/iTerm.app/Contents/MacOS/iTerm2
  512   400 ttys001 -zsh
  613   512 ttys001 vim notes.md
  700     1 ??      /Applications/Cursor.app/Contents/MacOS/Cursor
  801     1 ttys004 /bin/bash
  900   512 ttys001 claude
";

    #[test]
    fn parses_ps_output() {
        let procs = parse_ps(PS_SAMPLE);
        assert_eq!(procs.len(), 7);
        assert_eq!(procs[2].pid, 512);
        assert_eq!(procs[2].tty, "ttys001");
        assert_eq!(procs[2].exe_basename(), "zsh");
        assert_eq!(procs[1].exe_basename(), "iterm2");
    }

    #[test]
    fn terminals_map_to_their_emulator_with_cwd_and_icon() {
        let procs = parse_ps(PS_SAMPLE);
        let terminals = terminal_artifacts(
            &procs,
            |pid| (pid == 512).then(|| "/work/payments".to_string()),
            |exe| exe.contains("iTerm").then(|| "data:image/png;base64,AAA".to_string()),
        );
        assert_eq!(terminals.len(), 2);
        assert_eq!(terminals[0].label, "iTerm2 (ttys001)");
        assert_eq!(terminals[0].detail, "/work/payments");
        assert_eq!(terminals[0].kind, "terminal");
        // Icon resolved from the emulator's executable path.
        assert_eq!(terminals[0].icon.as_deref(), Some("data:image/png;base64,AAA"));
        // bash on ttys004 has no known emulator ancestor: placeholder.
        assert_eq!(terminals[1].label, "Shell (ttys004)");
        assert_eq!(terminals[1].icon, None);
    }

    #[test]
    fn one_artifact_per_tty() {
        let procs = parse_ps(
            "  512     1 ttys001 -zsh\n  513     1 ttys001 /bin/zsh\n",
        );
        assert_eq!(terminal_artifacts(&procs, |_| None, |_| None).len(), 1);
    }

    #[test]
    fn tty_labels_parse_and_reject_junk() {
        assert_eq!(tty_from_label("Shell (ttys001)").as_deref(), Some("ttys001"));
        assert_eq!(tty_from_label("GNOME Terminal (pts/0)").as_deref(), Some("pts/0"));
        assert_eq!(tty_from_label("no parens"), None);
        assert_eq!(tty_from_label("bad (tty; rm)"), None);
    }

    #[test]
    fn activity_lines_carry_header_and_processes() {
        let lines = activity_lines(
            "Shell (ttys001)",
            Some("/work/payments"),
            " 1604 05:12 -zsh\n 1799 00:40 vim notes.md\n",
        );
        assert_eq!(lines[0], "== Shell (ttys001) - /work/payments");
        assert_eq!(lines[2], "  1799 00:40 vim notes.md");

        let empty = activity_lines("Shell (ttys009)", None, "\n");
        assert_eq!(empty[1], "  (no processes on this tty)");
    }

    #[test]
    fn agents_detected_from_processes_and_transcripts() {
        let base = std::env::temp_dir().join(format!("cohort-scan-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let project = base.join(".claude").join("projects").join("-work-payments");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(
            project.join("s1.jsonl"),
            "{\"type\":\"user\",\"cwd\":\"/work/payments\"}\n{\"type\":\"assistant\",\"cwd\":\"/work/payments\"}\n",
        )
        .unwrap();

        let procs = parse_ps(PS_SAMPLE);
        let (agents, active_cwd) = agent_artifacts(&procs, &base, |exe| {
            exe.contains("Cursor").then(|| "data:image/png;base64,BBB".to_string())
        });
        let claude = agents.iter().find(|a| a.label == "Claude Code").unwrap();
        assert!(claude.detail.contains("agent session active - /work/payments"));
        assert_eq!(active_cwd.as_deref(), Some("/work/payments"));
        // Cursor is running but has no transcript integration yet.
        let cursor = agents.iter().find(|a| a.label == "Cursor").unwrap();
        assert!(cursor.detail.starts_with("running (pid"));
        assert_eq!(cursor.icon.as_deref(), Some("data:image/png;base64,BBB"));
        // Codex/Aider are neither running nor installed here.
        assert!(!agents.iter().any(|a| a.label == "Codex"));

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn jsonl_last_cwd_reads_final_line() {
        let path = std::env::temp_dir().join(format!("cohort-jsonl-{}.jsonl", std::process::id()));
        std::fs::write(&path, "{\"cwd\":\"/old\"}\n{\"cwd\":\"/new\"}\nnot-json\n").unwrap();
        assert_eq!(jsonl_last_cwd(&path).as_deref(), Some("/new"));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn directories_dedupe_and_cap_at_five() {
        let home = PathBuf::from("/Users/me");
        let terminal_dirs: Vec<String> = (1..=7).map(|i| format!("/work/p{i}")).collect();
        let dirs = directory_artifacts(&terminal_dirs, Some("/work/p1"), &home);
        assert_eq!(dirs.len(), MAX_DIR_SUGGESTIONS);
        assert_eq!(dirs[0].detail, "/work/p1");
        assert_eq!(dirs[0].label, "p1");
        // agent cwd and terminal cwd deduped
        assert_eq!(dirs.iter().filter(|d| d.detail == "/work/p1").count(), 1);
        // home itself and non-absolute details are excluded
        let dirs = directory_artifacts(
            &["zsh session".to_string(), "/Users/me".to_string()],
            None,
            &home,
        );
        assert!(dirs.is_empty());
    }
}
