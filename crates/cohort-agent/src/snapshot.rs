//! Read-only snapshots of the paths an owner shares. Runs on the owner's
//! machine; the result reaches the hub only when the owner creates the assist
//! or approves a file grant.
//!
//! Bounded on purpose: text files only, size/depth/count caps, noisy build
//! directories skipped. A basic secret-masking pass runs over every line;
//! the plan's full redaction engine (P3) replaces it - keep patterns here
//! conservative rather than clever.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use ts_rs::TS;

pub const MAX_FILE_BYTES: usize = 128 * 1024;
pub const MAX_TOTAL_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_FILES: usize = 60;
pub const MAX_DEPTH: usize = 6;

const SKIP_DIRS: [&str; 6] = [".git", "node_modules", "target", "dist", ".venv", "__pycache__"];

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct SnapshotNode {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub children: Vec<SnapshotNode>,
}

/// Tree plus contents for the shared paths. Field names match the hub's
/// LiveData subset so the app can upload it directly.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "ts-export", ts(export))]
pub struct PathSnapshot {
    pub file_tree: Vec<SnapshotNode>,
    pub files: HashMap<String, String>,
    /// Paths that were skipped or truncated, for the owner's information.
    pub notes: Vec<String>,
}

/// Mask obvious secret values, line by line.
pub fn redact(content: &str) -> String {
    let markers = ["password", "passwd", "secret", "token", "api_key", "apikey", "authorization", "private_key"];
    content
        .lines()
        .map(|line| {
            let lower = line.to_lowercase();
            let hit = markers.iter().any(|m| {
                lower.contains(m)
                    && (lower.contains('=') || lower.contains(':'))
            });
            if hit {
                match line.find(|c| c == '=' || c == ':') {
                    Some(pos) => format!("{}{} <redacted>", &line[..pos], &line[pos..pos + 1]),
                    None => "<redacted>".to_string(),
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct Walker {
    files: Vec<(String, String)>, // (absolute path, content)
    total_bytes: usize,
    notes: Vec<String>,
}

impl Walker {
    fn full(&self) -> bool {
        self.files.len() >= MAX_FILES || self.total_bytes >= MAX_TOTAL_BYTES
    }

    fn take_file(&mut self, path: &Path) {
        if self.full() {
            return;
        }
        let Ok(bytes) = std::fs::read(path) else {
            self.notes.push(format!("unreadable: {}", path.display()));
            return;
        };
        let Ok(mut content) = String::from_utf8(bytes) else {
            self.notes.push(format!("binary skipped: {}", path.display()));
            return;
        };
        if content.len() > MAX_FILE_BYTES {
            let mut cut = MAX_FILE_BYTES;
            while !content.is_char_boundary(cut) {
                cut -= 1;
            }
            content.truncate(cut);
            content.push_str("\n... (truncated)");
            self.notes.push(format!("truncated: {}", path.display()));
        }
        let content = redact(&content);
        self.total_bytes += content.len();
        self.files.push((path.to_string_lossy().to_string(), content));
    }

    fn walk(&mut self, path: &Path, depth: usize) {
        if self.full() || depth > MAX_DEPTH {
            return;
        }
        if path.is_file() {
            self.take_file(path);
            return;
        }
        if !path.is_dir() {
            self.notes.push(format!("not found: {}", path.display()));
            return;
        }
        let Ok(entries) = std::fs::read_dir(path) else {
            self.notes.push(format!("unreadable: {}", path.display()));
            return;
        };
        let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for entry in entries {
            let name = entry.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || SKIP_DIRS.contains(&name) {
                continue;
            }
            self.walk(&entry, depth + 1);
        }
    }
}

/// Build the nested tree for one shared root from the captured file paths.
fn tree_for_root(root: &Path, files: &[(String, String)]) -> Option<SnapshotNode> {
    let root_str = root.to_string_lossy().to_string();
    let root_name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| root_str.clone());
    if root.is_file() {
        return files.iter().any(|(p, _)| *p == root_str).then(|| SnapshotNode {
            name: root_name,
            path: root_str,
            children: vec![],
        });
    }
    let mut root_node = SnapshotNode { name: root_name, path: root_str.clone(), children: vec![] };
    let prefix = format!("{}/", root_str.trim_end_matches('/'));
    for (path, _) in files.iter().filter(|(p, _)| p.starts_with(&prefix)) {
        let relative = &path[prefix.len()..];
        let mut node = &mut root_node;
        let mut so_far = prefix.trim_end_matches('/').to_string();
        for part in relative.split('/') {
            so_far = format!("{so_far}/{part}");
            let position = node.children.iter().position(|c| c.name == part);
            let index = match position {
                Some(i) => i,
                None => {
                    node.children.push(SnapshotNode {
                        name: part.to_string(),
                        path: so_far.clone(),
                        children: vec![],
                    });
                    node.children.len() - 1
                }
            };
            node = &mut node.children[index];
        }
    }
    (!root_node.children.is_empty()).then_some(root_node)
}

/// Snapshot the given files/directories, bounded and redacted.
pub fn snapshot_paths(paths: &[String]) -> PathSnapshot {
    let mut walker = Walker { files: vec![], total_bytes: 0, notes: vec![] };
    let mut seen: Vec<&String> = vec![];
    for path in paths {
        if path.trim().is_empty() || seen.contains(&path) {
            continue;
        }
        seen.push(path);
        walker.walk(Path::new(path), 0);
    }
    let mut tree = Vec::new();
    for path in seen {
        if let Some(node) = tree_for_root(Path::new(path), &walker.files) {
            tree.push(node);
        }
    }
    PathSnapshot {
        file_tree: tree,
        files: walker.files.into_iter().collect(),
        notes: walker.notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(name: &str) -> std::path::PathBuf {
        // One directory per test: they run concurrently in one process.
        let base = std::env::temp_dir().join(format!("cohort-snap-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("k8s/payments")).unwrap();
        std::fs::create_dir_all(base.join("node_modules/x")).unwrap();
        std::fs::write(base.join("k8s/payments/deployment.yaml"), "image: api:1.9.4\n").unwrap();
        std::fs::write(base.join("k8s/payments/secrets.env"), "API_KEY=abc123\nname: ok\n").unwrap();
        std::fs::write(base.join("node_modules/x/ignored.js"), "x").unwrap();
        std::fs::write(base.join("binary.bin"), [0u8, 159, 146, 150]).unwrap();
        base
    }

    #[test]
    fn snapshots_tree_contents_and_redacts() {
        let base = setup("redacts");
        let snap = snapshot_paths(&[base.to_string_lossy().to_string()]);

        let deployment = base.join("k8s/payments/deployment.yaml");
        assert_eq!(snap.files[&deployment.to_string_lossy().to_string()], "image: api:1.9.4");

        // Secrets masked, non-secret lines kept.
        let env = snap.files[&base.join("k8s/payments/secrets.env").to_string_lossy().to_string()].clone();
        assert!(env.contains("API_KEY= <redacted>"));
        assert!(!env.contains("abc123"));
        assert!(env.contains("name: ok"));

        // node_modules and binaries skipped, noted.
        assert!(!snap.files.keys().any(|p| p.contains("node_modules")));
        assert!(snap.notes.iter().any(|n| n.contains("binary skipped")));

        // Tree mirrors the captured structure with absolute paths.
        assert_eq!(snap.file_tree.len(), 1);
        let root = &snap.file_tree[0];
        let k8s = root.children.iter().find(|c| c.name == "k8s").unwrap();
        let payments = k8s.children.iter().find(|c| c.name == "payments").unwrap();
        assert!(payments.children.iter().any(|c| c.path == deployment.to_string_lossy()));

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn single_file_and_missing_path() {
        let base = setup("single");
        let file = base.join("k8s/payments/deployment.yaml");
        let snap = snapshot_paths(&[
            file.to_string_lossy().to_string(),
            base.join("nope.txt").to_string_lossy().to_string(),
        ]);
        assert_eq!(snap.files.len(), 1);
        assert_eq!(snap.file_tree.len(), 1);
        assert_eq!(snap.file_tree[0].name, "deployment.yaml");
        assert!(snap.notes.iter().any(|n| n.contains("not found")));
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn truncates_oversized_files() {
        let base = setup("truncate");
        std::fs::write(base.join("big.txt"), "a".repeat(MAX_FILE_BYTES + 100)).unwrap();
        let snap = snapshot_paths(&[base.join("big.txt").to_string_lossy().to_string()]);
        let content = snap.files.values().next().unwrap();
        assert!(content.ends_with("... (truncated)"));
        assert!(content.len() <= MAX_FILE_BYTES + 20);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
