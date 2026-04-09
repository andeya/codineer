//! Model-specific context window limits for compaction and budgeting.

/// Known limits for a model family used to compute how much of the context window
/// is available for input after reserving output and auto-compact headroom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelContextWindow {
    pub context_window: usize,
    pub max_output_tokens: usize,
    pub autocompact_buffer: usize,
}

impl ModelContextWindow {
    /// Returns the effective input budget after reserving output and buffer space.
    #[must_use]
    pub fn effective_input_budget(&self) -> usize {
        self.context_window
            .saturating_sub(self.max_output_tokens)
            .saturating_sub(self.autocompact_buffer)
    }
}

/// Resolve context window metadata from a model id string (substring matching).
#[must_use]
pub fn context_window_for_model(model: &str) -> ModelContextWindow {
    // Known model families
    if model.contains("claude-4") || model.contains("opus") {
        return ModelContextWindow {
            context_window: 200_000,
            max_output_tokens: 16_384,
            autocompact_buffer: 20_000,
        };
    }
    if model.contains("claude-3") || model.contains("sonnet") || model.contains("haiku") {
        return ModelContextWindow {
            context_window: 200_000,
            max_output_tokens: 8_192,
            autocompact_buffer: 20_000,
        };
    }
    if model.contains("gpt-4o") || model.contains("gpt-4-turbo") {
        return ModelContextWindow {
            context_window: 128_000,
            max_output_tokens: 4_096,
            autocompact_buffer: 15_000,
        };
    }
    if model.contains("gemini") {
        return ModelContextWindow {
            context_window: 1_000_000,
            max_output_tokens: 8_192,
            autocompact_buffer: 50_000,
        };
    }
    // Default fallback
    ModelContextWindow {
        context_window: 100_000,
        max_output_tokens: 4_096,
        autocompact_buffer: 10_000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_input_budget_subtracts_output_and_buffer() {
        let w = ModelContextWindow {
            context_window: 100_000,
            max_output_tokens: 4_096,
            autocompact_buffer: 10_000,
        };
        assert_eq!(w.effective_input_budget(), 85_904);
    }

    #[test]
    fn effective_input_budget_saturates_at_zero() {
        let w = ModelContextWindow {
            context_window: 1000,
            max_output_tokens: 500,
            autocompact_buffer: 2000,
        };
        assert_eq!(w.effective_input_budget(), 0);
    }

    #[test]
    fn context_window_claude_4_family() {
        let w = context_window_for_model("anthropic/claude-4-sonnet");
        assert_eq!(w.context_window, 200_000);
        assert_eq!(w.max_output_tokens, 16_384);
        assert_eq!(w.autocompact_buffer, 20_000);
    }

    #[test]
    fn context_window_claude_3_sonnet() {
        let w = context_window_for_model("claude-3-5-sonnet-20241022");
        assert_eq!(w.context_window, 200_000);
        assert_eq!(w.max_output_tokens, 8_192);
    }

    #[test]
    fn context_window_gpt_4o() {
        let w = context_window_for_model("gpt-4o");
        assert_eq!(w.context_window, 128_000);
        assert_eq!(w.max_output_tokens, 4_096);
        assert_eq!(w.autocompact_buffer, 15_000);
    }

    #[test]
    fn context_window_gpt_4o_mini_matches_gpt_4o_branch() {
        let w = context_window_for_model("gpt-4o-mini");
        assert_eq!(w.context_window, 128_000);
    }

    #[test]
    fn context_window_gemini() {
        let w = context_window_for_model("gemini-2.0-flash");
        assert_eq!(w.context_window, 1_000_000);
        assert_eq!(w.max_output_tokens, 8_192);
        assert_eq!(w.autocompact_buffer, 50_000);
    }

    #[test]
    fn context_window_default_unknown_model() {
        let w = context_window_for_model("some-unknown-local");
        assert_eq!(w.context_window, 100_000);
        assert_eq!(w.max_output_tokens, 4_096);
        assert_eq!(w.autocompact_buffer, 10_000);
    }
}
