//! Microcompact strategy: selectively clear old, compactable tool results.
//!
//! This is the lightest compaction layer — it only clears tool results from
//! tools known to produce ephemeral output (e.g., bash, grep_search) while
//! preserving results from tools with persistent effects (e.g., write_file).

#![cfg_attr(not(test), allow(dead_code))]

use std::collections::HashSet;

/// Which tools' results can be safely cleared during microcompact.
static COMPACTABLE_TOOLS: &[&str] = &[
    "bash",
    "grep_search",
    "glob_search",
    "read_file",
    "WebSearch",
    "WebFetch",
    "ToolSearch",
    "Lsp",
    "MCPSearch",
    "ListMcpResources",
    "ReadMcpResource",
];

#[derive(Debug)]
pub(crate) struct MicrocompactStrategy {
    compactable: HashSet<&'static str>,
    keep_recent: usize,
}

impl MicrocompactStrategy {
    pub(crate) fn new(keep_recent: usize) -> Self {
        Self {
            compactable: COMPACTABLE_TOOLS.iter().copied().collect(),
            keep_recent,
        }
    }

    /// Check if any tool results are eligible for clearing.
    pub(crate) fn has_clearable_results(&self, tool_results: &[(String, String)]) -> bool {
        if tool_results.len() <= self.keep_recent {
            return false;
        }
        let clearable_count = tool_results.len() - self.keep_recent;
        tool_results[..clearable_count]
            .iter()
            .any(|(name, _)| self.compactable.contains(name.as_str()))
    }

    /// Clear eligible tool results, returning the count of cleared results.
    ///
    /// `tool_results` is `(tool_name, tool_result_content)` pairs.
    /// Results from the last `keep_recent` entries are never cleared.
    pub(crate) fn apply(&self, tool_results: &mut [(String, String)]) -> usize {
        if tool_results.len() <= self.keep_recent {
            return 0;
        }

        let clearable_end = tool_results.len() - self.keep_recent;
        let mut cleared = 0;

        for (name, content) in tool_results[..clearable_end].iter_mut() {
            if self.compactable.contains(name.as_str()) && !content.is_empty() {
                content.clear();
                cleared += 1;
            }
        }

        cleared
    }
}

impl Default for MicrocompactStrategy {
    fn default() -> Self {
        Self::new(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_clear_when_few_results() {
        let strategy = MicrocompactStrategy::new(3);
        let mut results = vec![
            ("bash".to_string(), "output".to_string()),
            ("grep_search".to_string(), "matches".to_string()),
        ];
        assert!(!strategy.has_clearable_results(&results));
        assert_eq!(strategy.apply(&mut results), 0);
    }

    #[test]
    fn clears_old_compactable_results() {
        let strategy = MicrocompactStrategy::new(1);
        let mut results = vec![
            ("bash".to_string(), "old output".to_string()),
            ("write_file".to_string(), "wrote file".to_string()),
            ("grep_search".to_string(), "recent matches".to_string()),
        ];
        assert!(strategy.has_clearable_results(&results));
        let cleared = strategy.apply(&mut results);
        assert_eq!(cleared, 1); // only bash, not write_file
        assert!(results[0].1.is_empty()); // bash cleared
        assert_eq!(results[1].1, "wrote file"); // write_file preserved
        assert_eq!(results[2].1, "recent matches"); // recent preserved
    }

    #[test]
    fn preserves_recent_entries() {
        let strategy = MicrocompactStrategy::new(2);
        let mut results = vec![
            ("bash".to_string(), "old".to_string()),
            ("bash".to_string(), "recent1".to_string()),
            ("bash".to_string(), "recent2".to_string()),
        ];
        let cleared = strategy.apply(&mut results);
        assert_eq!(cleared, 1);
        assert!(results[0].1.is_empty());
        assert_eq!(results[1].1, "recent1");
        assert_eq!(results[2].1, "recent2");
    }
}
