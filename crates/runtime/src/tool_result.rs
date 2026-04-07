//! Large tool result persistence and per-message budget management.
//!
//! Mirrors Claude Code's tool result storage: results exceeding a size threshold
//! are written to disk, and the in-memory representation is replaced with a
//! truncated preview. Per-message aggregate budgets prevent context overflow.

use std::borrow::Cow;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const DEFAULT_MAX_RESULT_SIZE_CHARS: usize = 50_000;
const MAX_TOOL_RESULTS_PER_MESSAGE_CHARS: usize = 200_000;
const PREVIEW_SIZE_BYTES: usize = 2_000;

/// Manages persisting large tool results to disk and enforcing budgets.
#[derive(Debug)]
pub struct ToolResultManager {
    session_dir: PathBuf,
    persisted: HashSet<String>,
}

impl ToolResultManager {
    pub fn new(session_dir: impl Into<PathBuf>) -> Self {
        Self {
            session_dir: session_dir.into(),
            persisted: HashSet::new(),
        }
    }

    /// If `content` exceeds the threshold, persist it to disk and return a preview.
    /// Otherwise, return the original content unmodified (zero-copy).
    pub fn maybe_persist<'a>(
        &mut self,
        tool_use_id: &str,
        tool_name: &str,
        content: &'a str,
        threshold: Option<usize>,
    ) -> Cow<'a, str> {
        let max_size = threshold.unwrap_or(DEFAULT_MAX_RESULT_SIZE_CHARS);
        if content.len() <= max_size {
            return Cow::Borrowed(content);
        }

        let storage_dir = self.session_dir.join("tool_results");
        if std::fs::create_dir_all(&storage_dir).is_err() {
            return Cow::Borrowed(content);
        }

        let file_path = storage_dir.join(format!("{tool_use_id}.txt"));
        if std::fs::write(&file_path, content).is_err() {
            return Cow::Borrowed(content);
        }

        self.persisted.insert(tool_use_id.to_string());

        let preview = make_preview(content, tool_name, &file_path);
        Cow::Owned(preview)
    }

    /// Count how many results have been persisted.
    #[must_use]
    pub fn persisted_count(&self) -> usize {
        self.persisted.len()
    }

    /// Check if a specific tool result was persisted.
    #[must_use]
    pub fn is_persisted(&self, tool_use_id: &str) -> bool {
        self.persisted.contains(tool_use_id)
    }

    /// Enforce the per-message aggregate budget on a set of tool result texts.
    ///
    /// Returns the number of results that were truncated to fit the budget.
    pub fn enforce_budget(results: &mut [String]) -> usize {
        let total: usize = results.iter().map(String::len).sum();
        if total <= MAX_TOOL_RESULTS_PER_MESSAGE_CHARS {
            return 0;
        }

        let mut truncated = 0;
        let mut remaining_budget = MAX_TOOL_RESULTS_PER_MESSAGE_CHARS;

        for result in results.iter_mut() {
            if result.len() <= remaining_budget {
                remaining_budget -= result.len();
            } else {
                let preview_end = result
                    .char_indices()
                    .take_while(|(i, _)| *i < PREVIEW_SIZE_BYTES.min(remaining_budget))
                    .last()
                    .map_or(0, |(i, c)| i + c.len_utf8());
                *result = format!(
                    "{}\n\n[Truncated: original was {} chars, budget exceeded]",
                    &result[..preview_end],
                    result.len()
                );
                remaining_budget = 0;
                truncated += 1;
            }
        }

        truncated
    }
}

fn make_preview(content: &str, tool_name: &str, persisted_path: &Path) -> String {
    let preview_end = content
        .char_indices()
        .take_while(|(i, _)| *i < PREVIEW_SIZE_BYTES)
        .last()
        .map_or(0, |(i, c)| i + c.len_utf8());

    format!(
        "{}\n\n[{tool_name} output truncated: {} chars total, persisted to {}]",
        &content[..preview_end],
        content.len(),
        persisted_path.display(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_session_dir() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("codineer-tool-result-test-{nanos}"))
    }

    #[test]
    fn small_result_not_persisted() {
        let dir = temp_session_dir();
        let mut mgr = ToolResultManager::new(&dir);
        let content = "short content";
        let result = mgr.maybe_persist("id-1", "bash", content, None);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), content);
        assert_eq!(mgr.persisted_count(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn large_result_persisted_and_previewed() {
        let dir = temp_session_dir();
        let mut mgr = ToolResultManager::new(&dir);
        let content = "x".repeat(60_000);
        let result = mgr.maybe_persist("id-2", "grep_search", &content, None);
        assert!(matches!(result, Cow::Owned(_)));
        assert!(result.contains("[grep_search output truncated:"));
        assert!(result.contains("60000 chars total"));
        assert!(mgr.is_persisted("id-2"));

        let persisted = fs::read_to_string(dir.join("tool_results/id-2.txt")).unwrap();
        assert_eq!(persisted.len(), 60_000);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn custom_threshold() {
        let dir = temp_session_dir();
        let mut mgr = ToolResultManager::new(&dir);
        let content = "a".repeat(100);
        let result = mgr.maybe_persist("id-3", "bash", &content, Some(50));
        assert!(matches!(result, Cow::Owned(_)));
        assert!(mgr.is_persisted("id-3"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn enforce_budget_under_limit() {
        let mut results = vec!["hello".to_string(), "world".to_string()];
        let truncated = ToolResultManager::enforce_budget(&mut results);
        assert_eq!(truncated, 0);
        assert_eq!(results[0], "hello");
    }

    #[test]
    fn enforce_budget_truncates_excess() {
        let big = "x".repeat(150_000);
        let also_big = "y".repeat(100_000);
        let mut results = vec![big, also_big];
        let truncated = ToolResultManager::enforce_budget(&mut results);
        assert!(truncated > 0);
        // First result should fit (150K < 200K budget)
        assert_eq!(results[0].len(), 150_000);
        // Second result should be truncated
        assert!(results[1].contains("[Truncated:"));
    }
}
