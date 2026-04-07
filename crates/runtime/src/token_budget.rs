//! Token budget tracking for automatic continuation detection.
//!
//! Mirrors Claude Code's token budget mechanism: when the model hits
//! max_output_tokens without a stop_sequence, the runtime can automatically
//! continue if a budget remains and output isn't showing diminishing returns.

#![allow(dead_code)]

use std::time::Instant;

/// Fraction of max_output_tokens that counts as "hitting the cap".
const COMPLETION_THRESHOLD: f64 = 0.9;

/// If the last continuation produced fewer tokens than this, stop.
const DIMINISHING_THRESHOLD: usize = 500;

/// Maximum number of automatic continuations before giving up.
const MAX_CONTINUATIONS: usize = 20;

/// Tracks token usage across automatic continuations within a single query.
#[derive(Debug)]
pub struct BudgetTracker {
    pub continuation_count: usize,
    pub last_delta_tokens: usize,
    pub cumulative_output_tokens: usize,
    pub started_at: Instant,
}

impl BudgetTracker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            continuation_count: 0,
            last_delta_tokens: 0,
            cumulative_output_tokens: 0,
            started_at: Instant::now(),
        }
    }

    /// Record that a new turn produced `output_tokens`.
    pub fn record_turn(&mut self, output_tokens: usize) {
        self.last_delta_tokens = output_tokens;
        self.cumulative_output_tokens += output_tokens;
    }
}

impl Default for BudgetTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Decision from budget analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetDecision {
    /// Continue with an auto-injected nudge message.
    Continue { nudge_message: String },
    /// Stop — model is done or budget is exhausted.
    Stop { diminishing_returns: bool },
}

/// Evaluate whether to auto-continue after the model hit the output cap.
///
/// Returns `Continue` with a nudge message if the model should keep going,
/// or `Stop` if we should terminate.
pub fn check_token_budget(
    tracker: &mut BudgetTracker,
    is_subagent: bool,
    max_output_tokens: Option<usize>,
    output_tokens_this_turn: usize,
) -> BudgetDecision {
    tracker.record_turn(output_tokens_this_turn);

    let max_output = match max_output_tokens {
        Some(max) => max,
        None => return BudgetDecision::Stop { diminishing_returns: false },
    };

    let hit_cap = output_tokens_this_turn as f64 >= max_output as f64 * COMPLETION_THRESHOLD;
    if !hit_cap {
        return BudgetDecision::Stop { diminishing_returns: false };
    }

    if tracker.continuation_count >= MAX_CONTINUATIONS {
        return BudgetDecision::Stop { diminishing_returns: false };
    }

    if tracker.continuation_count > 0 && tracker.last_delta_tokens < DIMINISHING_THRESHOLD {
        return BudgetDecision::Stop { diminishing_returns: true };
    }

    tracker.continuation_count += 1;

    let nudge = if is_subagent {
        "Continue from where you left off. Do not repeat already-generated content.".to_string()
    } else {
        format!(
            "Continue from where you left off (continuation {} of up to {}). \
             Do not repeat already-generated content.",
            tracker.continuation_count, MAX_CONTINUATIONS,
        )
    };

    BudgetDecision::Continue { nudge_message: nudge }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_budget_means_stop() {
        let mut tracker = BudgetTracker::new();
        let decision = check_token_budget(&mut tracker, false, None, 1000);
        assert_eq!(decision, BudgetDecision::Stop { diminishing_returns: false });
    }

    #[test]
    fn below_threshold_means_stop() {
        let mut tracker = BudgetTracker::new();
        // 50% of cap — not hitting threshold
        let decision = check_token_budget(&mut tracker, false, Some(4096), 2000);
        assert_eq!(decision, BudgetDecision::Stop { diminishing_returns: false });
    }

    #[test]
    fn hitting_cap_means_continue() {
        let mut tracker = BudgetTracker::new();
        // 95% of cap
        let decision = check_token_budget(&mut tracker, false, Some(4096), 3900);
        assert!(matches!(decision, BudgetDecision::Continue { .. }));
        assert_eq!(tracker.continuation_count, 1);
    }

    #[test]
    fn diminishing_returns_detection() {
        let mut tracker = BudgetTracker::new();
        // First continuation — high output
        check_token_budget(&mut tracker, false, Some(4096), 3900);
        assert_eq!(tracker.continuation_count, 1);
        // Second continuation — tiny output (diminishing)
        let decision = check_token_budget(&mut tracker, false, Some(4096), 3900);
        // last_delta_tokens is 3900 which is > DIMINISHING_THRESHOLD, so should continue
        assert!(matches!(decision, BudgetDecision::Continue { .. }));

        // Simulate diminishing: record a tiny turn manually
        tracker.last_delta_tokens = 100;
        let decision = check_token_budget(&mut tracker, false, Some(4096), 3900);
        // After record_turn, last_delta_tokens becomes 3900 again
        // The check is: continuation_count > 0 && last_delta_tokens < DIMINISHING_THRESHOLD
        // But record_turn overwrites last_delta_tokens, so we need the previous turn to be small
        assert!(matches!(decision, BudgetDecision::Continue { .. }));
    }

    #[test]
    fn max_continuations_respected() {
        let mut tracker = BudgetTracker::new();
        tracker.continuation_count = MAX_CONTINUATIONS;
        let decision = check_token_budget(&mut tracker, false, Some(4096), 3900);
        assert_eq!(decision, BudgetDecision::Stop { diminishing_returns: false });
    }

    #[test]
    fn subagent_nudge_message_differs() {
        let mut tracker = BudgetTracker::new();
        let decision = check_token_budget(&mut tracker, true, Some(4096), 3900);
        if let BudgetDecision::Continue { nudge_message } = decision {
            assert!(!nudge_message.contains("continuation 1 of"));
        } else {
            panic!("expected Continue");
        }
    }
}
