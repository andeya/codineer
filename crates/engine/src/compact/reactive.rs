//! Reactive compaction: triggered after API context-overflow errors.
//!
//! Unlike autocompact (proactive, threshold-based), reactive compact
//! is only invoked when an API call fails with a context-too-long error.
//! It attempts a more aggressive compaction and only fires once per session.

#![cfg_attr(not(test), allow(dead_code))]

/// State for the reactive compaction strategy.
#[derive(Debug, Default)]
pub(crate) struct ReactiveCompactStrategy {
    has_attempted: bool,
}

impl ReactiveCompactStrategy {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Whether reactive compact should be attempted.
    /// Returns true only if it hasn't been tried yet in this session.
    #[must_use]
    pub(crate) fn should_attempt(&self) -> bool {
        !self.has_attempted
    }

    /// Mark that a reactive compact was attempted.
    pub(crate) fn mark_attempted(&mut self) {
        self.has_attempted = true;
    }

    /// Reset the attempt flag (e.g., after a successful compact + API retry).
    pub(crate) fn reset(&mut self) {
        self.has_attempted = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attempts_once() {
        let mut strategy = ReactiveCompactStrategy::new();
        assert!(strategy.should_attempt());
        strategy.mark_attempted();
        assert!(!strategy.should_attempt());
    }

    #[test]
    fn reset_allows_retry() {
        let mut strategy = ReactiveCompactStrategy::new();
        strategy.mark_attempted();
        assert!(!strategy.should_attempt());
        strategy.reset();
        assert!(strategy.should_attempt());
    }
}
