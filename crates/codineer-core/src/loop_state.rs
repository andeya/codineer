//! Explicit state machine for the conversation loop.
//!
//! The runtime loop transitions through states via [`Transition`] variants
//! and terminates with a [`StopReason`]. Exhaustive `match` on both enums
//! ensures the compiler enforces handling of every case.

/// Why the loop should continue for another iteration.
#[derive(Debug, Clone)]
pub enum Transition {
    /// Normal tool-use → next turn.
    NextTurn,
    /// Autocompact fired and succeeded; retry the API call.
    AutocompactRetry,
    /// Reactive compact fired after a context-overflow error.
    ReactiveCompactRetry,
    /// Context collapse drained committed messages; retry.
    CollapseDrainRetry { committed: usize },
    /// Max-output-tokens cap was raised; retry with higher limit.
    MaxOutputEscalation { escalated_to: usize },
    /// Partial output recovered via a "please continue" meta-message.
    MaxOutputRecovery { attempt: usize },
    /// Model appears unfinished; auto-continue with a nudge.
    TokenBudgetContinuation { continuation_count: usize },
    /// Streaming failed mid-way; retrying in non-streaming mode.
    StreamingFallbackRetry,
}

/// Why the loop terminated.
#[derive(Debug, Clone)]
pub enum StopReason {
    /// Model returned end_turn / stop_sequence with no pending tool use.
    Completed,
    /// Hard iteration cap reached.
    MaxIterations,
    /// Configurable turn limit reached.
    MaxTurns { limit: usize },
    /// A hook observer vetoed continuation.
    HookPrevented(String),
    /// User or system requested cancellation.
    Aborted,
    /// Prompt exceeds context window even after all compaction attempts.
    PromptTooLong,
    /// Token budget continuation detected diminishing returns.
    TokenBudgetExhausted { diminishing_returns: bool },
    /// Max-output recovery attempts exhausted.
    MaxOutputRecoveryExhausted { attempts: usize },
    /// Unrecoverable model-side error.
    ModelError(String),
}

/// Accumulated state carried across loop iterations.
#[derive(Debug)]
pub struct LoopState {
    pub iteration: usize,
    pub turn: usize,
    pub max_output_escalated: bool,
    pub max_output_recovery_attempts: usize,
    pub streaming_fallback_active: bool,
}

impl LoopState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            iteration: 0,
            turn: 0,
            max_output_escalated: false,
            max_output_recovery_attempts: 0,
            streaming_fallback_active: false,
        }
    }

    pub fn advance_iteration(&mut self) {
        self.iteration += 1;
    }

    pub fn advance_turn(&mut self) {
        self.turn += 1;
    }
}

impl Default for LoopState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_state_defaults() {
        let state = LoopState::new();
        assert_eq!(state.iteration, 0);
        assert_eq!(state.turn, 0);
        assert!(!state.max_output_escalated);
    }

    #[test]
    fn advance_counters() {
        let mut state = LoopState::new();
        state.advance_iteration();
        state.advance_turn();
        assert_eq!(state.iteration, 1);
        assert_eq!(state.turn, 1);
    }

    #[test]
    fn transition_exhaustive() {
        let transitions = [
            Transition::NextTurn,
            Transition::AutocompactRetry,
            Transition::ReactiveCompactRetry,
            Transition::CollapseDrainRetry { committed: 5 },
            Transition::MaxOutputEscalation { escalated_to: 8192 },
            Transition::MaxOutputRecovery { attempt: 1 },
            Transition::TokenBudgetContinuation { continuation_count: 2 },
            Transition::StreamingFallbackRetry,
        ];
        assert_eq!(transitions.len(), 8);
    }

    #[test]
    fn stop_reason_exhaustive() {
        let reasons = [
            StopReason::Completed,
            StopReason::MaxIterations,
            StopReason::MaxTurns { limit: 10 },
            StopReason::HookPrevented("test".into()),
            StopReason::Aborted,
            StopReason::PromptTooLong,
            StopReason::TokenBudgetExhausted { diminishing_returns: true },
            StopReason::MaxOutputRecoveryExhausted { attempts: 3 },
            StopReason::ModelError("test".into()),
        ];
        assert_eq!(reasons.len(), 9);
    }
}
