//! What of this machine the model gets to see: the shared paths, read
//! through the same bounded, secret-masking snapshot the responder's file
//! view uses, then cut to a per-task budget. Characters, not tokens: four
//! characters per token keeps every model comfortably inside its window
//! without shipping a tokenizer per provider.

use crate::snapshot;

pub struct ContextBudget {
    pub total_chars: usize,
    pub per_file_chars: usize,
}

pub const INSIGHTS: ContextBudget = ContextBudget { total_chars: 40_000, per_file_chars: 8_000 };

#[derive(Debug, Clone, PartialEq)]
pub struct FileBlock {
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
    /// Paths that exist in the snapshot but did not fit the total budget.
    pub not_included: Vec<String>,
    /// The snapshot's own notes (unreadable, binary, truncated at 128 KB).
    pub notes: Vec<String>,
}

/// Snapshot the paths and fit them to the budget.
pub fn file_blocks(paths: &[String], budget: &ContextBudget) -> FileContext {
    let snap = snapshot::snapshot_paths(paths);
    let mut ctx = budget_files(snap.files.into_iter().collect(), budget);
    ctx.notes = snap.notes;
    ctx
}

/// Deterministic (sorted by path) so the same inputs build the same prompt.
/// Each file gets at most `per_file_chars`; the run stops adding files once
/// `total_chars` is spent and lists the rest as not included.
pub fn budget_files(mut files: Vec<(String, String)>, budget: &ContextBudget) -> FileContext {
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = FileContext::default();
    let mut used = 0usize;
    for (path, content) in files {
        if used >= budget.total_chars {
            out.not_included.push(path);
            continue;
        }
        let total_chars = content.chars().count();
        let room = (budget.total_chars - used).min(budget.per_file_chars);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn files(specs: &[(&str, usize)]) -> Vec<(String, String)> {
        specs.iter().map(|(p, n)| (p.to_string(), "x".repeat(*n))).collect()
    }

    #[test]
    fn fits_small_files_whole_and_sorts_by_path() {
        let ctx = budget_files(files(&[("b.rs", 10), ("a.rs", 5)]), &ContextBudget { total_chars: 100, per_file_chars: 50 });
        let paths: Vec<&str> = ctx.blocks.iter().map(|b| b.path.as_str()).collect();
        assert_eq!(paths, vec!["a.rs", "b.rs"]);
        assert!(ctx.blocks.iter().all(|b| !b.truncated));
        assert!(ctx.not_included.is_empty());
    }

    #[test]
    fn caps_each_file_and_records_the_full_length() {
        let ctx = budget_files(files(&[("big.log", 1000)]), &ContextBudget { total_chars: 100, per_file_chars: 40 });
        assert_eq!(ctx.blocks.len(), 1);
        assert_eq!(ctx.blocks[0].content.len(), 40);
        assert!(ctx.blocks[0].truncated);
        assert_eq!(ctx.blocks[0].total_chars, 1000);
    }

    #[test]
    fn stops_at_the_total_and_names_what_was_left_out() {
        let ctx = budget_files(
            files(&[("a", 40), ("b", 40), ("c", 40), ("d", 40)]),
            &ContextBudget { total_chars: 100, per_file_chars: 40 },
        );
        // a (40) + b (40) + c (20, cut to the remaining room) = 100; d is out.
        let paths: Vec<&str> = ctx.blocks.iter().map(|b| b.path.as_str()).collect();
        assert_eq!(paths, vec!["a", "b", "c"]);
        assert_eq!(ctx.blocks[2].content.len(), 20);
        assert!(ctx.blocks[2].truncated);
        assert_eq!(ctx.not_included, vec!["d"]);
    }

    #[test]
    fn cuts_on_character_boundaries() {
        let ctx = budget_files(
            vec![("u.txt".into(), "aaaa\u{e9}\u{e9}\u{e9}\u{e9}".into())],
            &ContextBudget { total_chars: 100, per_file_chars: 6 },
        );
        assert_eq!(ctx.blocks[0].content, "aaaa\u{e9}\u{e9}");
    }

    #[test]
    fn real_snapshot_is_redacted_before_it_reaches_the_prompt() {
        let base = std::env::temp_dir().join(format!("cohort-assistant-ctx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("app.env"), "API_KEY=abc123\nregion=eu\n").unwrap();
        let ctx = file_blocks(&[base.to_string_lossy().to_string()], &INSIGHTS);
        assert_eq!(ctx.blocks.len(), 1);
        assert!(ctx.blocks[0].content.contains("<redacted>"));
        assert!(!ctx.blocks[0].content.contains("abc123"));
        assert!(ctx.blocks[0].content.contains("region=eu"));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
