//! What of this machine the model gets to see: the shared paths, read
//! through the same bounded, secret-masking snapshot the responder's file
//! view uses, then chosen and cut to a per-task budget.
//!
//! Selection, in order: files that cost tokens and say nothing about the
//! problem are skipped (lockfiles, minified bundles, source maps); the rest
//! are ranked shallow-first under the shared root, then by path, so a
//! directory share leads with its top-level files rather than whatever
//! sorts first alphabetically; the budget is spread across them - each file
//! gets `total / n` characters, clamped to `[min, max]` - because for "what
//! is this about" breadth beats seven deep heads.
//!
//! Budgets are characters, not tokens: no tokenizer per provider. Measured
//! on real drafts, source and markup tokenize at roughly 2 characters per
//! token and prose at about 4, so 40k characters is 10k-20k input tokens.

use crate::snapshot;
use std::path::Path;

pub struct ContextBudget {
    pub total_chars: usize,
    /// The floor a file gets even when many share the budget: enough for
    /// imports and the opening comment, which is what "what is this" needs.
    pub min_file_chars: usize,
    pub max_file_chars: usize,
}

pub const INSIGHTS: ContextBudget =
    ContextBudget { total_chars: 40_000, min_file_chars: 2_000, max_file_chars: 8_000 };

#[derive(Debug, Clone, PartialEq)]
pub struct FileBlock {
    /// Shown to the model: `<shared root name>/<path under it>`.
    pub path: String,
    /// The head of the file within budget; see `truncated`.
    pub content: String,
    pub truncated: bool,
    /// Length of the whole (redacted) file, for the truncation marker.
    pub total_chars: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileContext {
    pub blocks: Vec<FileBlock>,
    /// Candidates that did not fit the total budget.
    pub not_included: Vec<String>,
    /// Files left out on purpose as low-value context.
    pub skipped: Vec<String>,
    /// The snapshot's own notes (unreadable, binary, truncated at 128 KB).
    pub notes: Vec<String>,
}

/// Snapshot the paths, choose, and fit to the budget.
pub fn file_blocks(paths: &[String], budget: &ContextBudget) -> FileContext {
    let snap = snapshot::snapshot_paths(paths);
    let mut candidates = Vec::new();
    let mut skipped = Vec::new();
    for (absolute, content) in snap.files {
        let display = display_path(&absolute, paths);
        if is_low_value(&display) {
            skipped.push(display);
        } else {
            candidates.push((display, content));
        }
    }
    skipped.sort();
    let mut ctx = budget_files(candidates, budget);
    ctx.skipped = skipped;
    ctx.notes = snap.notes;
    ctx
}

/// Rank shallow-first, then by path, and spread the budget: every file gets
/// `total / n` characters clamped to `[min, max]`, until the total is spent.
/// Deterministic, so the same inputs build the same prompt.
pub fn budget_files(mut files: Vec<(String, String)>, budget: &ContextBudget) -> FileContext {
    files.sort_by(|a, b| depth(&a.0).cmp(&depth(&b.0)).then_with(|| a.0.cmp(&b.0)));
    let per_file = if files.is_empty() {
        budget.max_file_chars
    } else {
        (budget.total_chars / files.len()).clamp(budget.min_file_chars, budget.max_file_chars)
    };
    let mut out = FileContext::default();
    let mut used = 0usize;
    for (path, content) in files {
        if used >= budget.total_chars {
            out.not_included.push(path);
            continue;
        }
        let total_chars = content.chars().count();
        let room = (budget.total_chars - used).min(per_file);
        let (text, truncated) = if total_chars > room {
            (content.chars().take(room).collect::<String>(), true)
        } else {
            (content, false)
        };
        used += text.chars().count();
        out.blocks.push(FileBlock { path, content: text, truncated, total_chars });
    }
    out
}

fn depth(display: &str) -> usize {
    display.matches('/').count()
}

/// `<shared root name>/<path under it>`: shorter than an absolute path and
/// free of the user's home directory. A shared file is just its name. A
/// path under none of the roots (should not happen) stays absolute.
pub fn display_path(absolute: &str, roots: &[String]) -> String {
    let file = Path::new(absolute);
    let root = roots
        .iter()
        .map(|r| Path::new(r.as_str()))
        .filter(|r| file.starts_with(r))
        .max_by_key(|r| r.as_os_str().len());
    let Some(root) = root else {
        return absolute.to_string();
    };
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());
    match file.strip_prefix(root) {
        Ok(rel) if !rel.as_os_str().is_empty() => {
            format!("{name}/{}", rel.to_string_lossy().replace('\\', "/"))
        }
        _ => name,
    }
}

/// Files that cost tokens and tell the model nothing about the problem.
pub fn is_low_value(display: &str) -> bool {
    let name = display.rsplit('/').next().unwrap_or(display).to_ascii_lowercase();
    const LOCKFILES: [&str; 9] = [
        "cargo.lock",
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "poetry.lock",
        "pipfile.lock",
        "composer.lock",
        "gemfile.lock",
        "go.sum",
    ];
    LOCKFILES.contains(&name.as_str())
        || name.ends_with(".min.js")
        || name.ends_with(".min.css")
        || name.ends_with(".map")
        || name.ends_with(".svg")
        || ((name.ends_with(".js") || name.ends_with(".css")) && name.contains("bundle"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(specs: &[(&str, usize)]) -> Vec<(String, String)> {
        specs.iter().map(|(p, n)| (p.to_string(), "x".repeat(*n))).collect()
    }

    #[test]
    fn ranks_shallow_first_then_by_path() {
        let ctx = budget_files(
            files(&[("root/deep/x/a.rs", 5), ("root/b.rs", 5), ("root/a.rs", 5), ("root/src/z.rs", 5)]),
            &ContextBudget { total_chars: 100, min_file_chars: 10, max_file_chars: 50 },
        );
        let paths: Vec<&str> = ctx.blocks.iter().map(|b| b.path.as_str()).collect();
        assert_eq!(paths, vec!["root/a.rs", "root/b.rs", "root/src/z.rs", "root/deep/x/a.rs"]);
        assert!(ctx.blocks.iter().all(|b| !b.truncated));
        assert!(ctx.not_included.is_empty());
    }

    #[test]
    fn a_lone_file_gets_the_maximum_and_records_its_full_length() {
        let ctx = budget_files(files(&[("big.log", 1000)]), &ContextBudget { total_chars: 100, min_file_chars: 20, max_file_chars: 40 });
        assert_eq!(ctx.blocks.len(), 1);
        assert_eq!(ctx.blocks[0].content.len(), 40, "total/1 clamps to the maximum");
        assert!(ctx.blocks[0].truncated);
        assert_eq!(ctx.blocks[0].total_chars, 1000);
    }

    #[test]
    fn spreads_the_budget_across_many_files() {
        // Four files of 40 into a total of 100: each gets 25, all are heard from.
        let ctx = budget_files(
            files(&[("a", 40), ("b", 40), ("c", 40), ("d", 40)]),
            &ContextBudget { total_chars: 100, min_file_chars: 10, max_file_chars: 40 },
        );
        assert_eq!(ctx.blocks.len(), 4);
        assert!(ctx.blocks.iter().all(|b| b.content.len() == 25 && b.truncated));
        assert!(ctx.not_included.is_empty());
    }

    #[test]
    fn the_floor_holds_and_the_rest_is_named_as_not_included() {
        // total/4 = 12 but the floor is 25: two files at 25, two left out.
        let ctx = budget_files(
            files(&[("a", 40), ("b", 40), ("c", 40), ("d", 40)]),
            &ContextBudget { total_chars: 50, min_file_chars: 25, max_file_chars: 40 },
        );
        let paths: Vec<&str> = ctx.blocks.iter().map(|b| b.path.as_str()).collect();
        assert_eq!(paths, vec!["a", "b"]);
        assert_eq!(ctx.not_included, vec!["c", "d"]);
    }

    #[test]
    fn cuts_on_character_boundaries() {
        let ctx = budget_files(
            vec![("u.txt".into(), "aaaa\u{e9}\u{e9}\u{e9}\u{e9}".into())],
            &ContextBudget { total_chars: 100, min_file_chars: 1, max_file_chars: 6 },
        );
        assert_eq!(ctx.blocks[0].content, "aaaa\u{e9}\u{e9}");
    }

    #[test]
    fn display_paths_are_relative_to_the_shared_root() {
        let roots = vec!["/home/me/work/Cohort".to_string(), "/home/me/notes.md".to_string()];
        assert_eq!(display_path("/home/me/work/Cohort/cohort/src/lib.rs", &roots), "Cohort/cohort/src/lib.rs");
        assert_eq!(display_path("/home/me/work/Cohort", &roots), "Cohort");
        assert_eq!(display_path("/home/me/notes.md", &roots), "notes.md");
        assert_eq!(display_path("/elsewhere/x.rs", &roots), "/elsewhere/x.rs");
        // The most specific root wins when one contains another.
        let nested = vec!["/w/proj".to_string(), "/w/proj/app".to_string()];
        assert_eq!(display_path("/w/proj/app/main.ts", &nested), "app/main.ts");
        // A trailing slash on the root changes nothing.
        assert_eq!(display_path("/w/proj/README", &["/w/proj/".to_string()]), "proj/README");
    }

    #[test]
    fn low_value_files_are_recognised_by_name_only() {
        for p in ["Cohort/cohort/Cargo.lock", "app/package-lock.json", "x/vendor.min.js", "x/app.bundle.js", "x/_ds_bundle.js", "x/styles.css.map", "x/logo.svg", "GO.SUM"] {
            assert!(is_low_value(p), "{p}");
        }
        for p in ["Cargo.toml", "src/lib.rs", "app/index.html", "styles.css", "bundle/README.md", "locks.rs"] {
            assert!(!is_low_value(p), "{p}");
        }
    }

    #[test]
    fn real_snapshot_is_redacted_relative_and_filtered() {
        let base = std::env::temp_dir().join(format!("cohort-assistant-ctx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("deep")).unwrap();
        std::fs::write(base.join("app.env"), "API_KEY=abc123\nregion=eu\n").unwrap();
        std::fs::write(base.join("Cargo.lock"), "[[package]]\nname = \"x\"\n").unwrap();
        std::fs::write(base.join("deep/vendor.min.js"), "!function(){}();").unwrap();
        let root = base.to_string_lossy().to_string();
        let root_name = base.file_name().unwrap().to_string_lossy().to_string();

        let ctx = file_blocks(&[root], &INSIGHTS);

        assert_eq!(ctx.blocks.len(), 1);
        assert_eq!(ctx.blocks[0].path, format!("{root_name}/app.env"));
        assert!(ctx.blocks[0].content.contains("<redacted>"));
        assert!(!ctx.blocks[0].content.contains("abc123"));
        assert!(ctx.blocks[0].content.contains("region=eu"));
        assert_eq!(ctx.skipped, vec![format!("{root_name}/Cargo.lock"), format!("{root_name}/deep/vendor.min.js")]);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
