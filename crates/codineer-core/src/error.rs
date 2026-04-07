//! Structured runtime error types.
//!
//! [`RuntimeError`] replaces the previous string-wrapper error with a rich
//! enum whose variants encode specific failure modes. This enables:
//! - Exhaustive `match` for error handling
//! - Typed recovery strategies (see `recovery.rs`)
//! - `#[from]` automatic conversion from downstream errors

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("max iterations exceeded ({iterations})")]
    MaxIterations { iterations: usize },
    #[error("max turns exceeded ({turns})")]
    MaxTurns { turns: usize },
    #[error("empty model reply")]
    EmptyReply,
    #[error("stream ended without stop event")]
    IncompleteStream,
    #[error("context overflow: {message}")]
    ContextOverflow { message: String },
    #[error("hook prevented continuation: {reason}")]
    HookPrevented { reason: String },
    #[error("cancelled")]
    Cancelled,
    #[error("compaction failed: {0}")]
    Compaction(String),
    #[error("api error: {0}")]
    Api(String),
    #[error("tool error: {0}")]
    Tool(String),
    #[error("{0}")]
    Other(String),
}

impl RuntimeError {
    /// Check if this error represents a context overflow.
    #[must_use]
    pub fn is_context_overflow(&self) -> bool {
        matches!(self, Self::ContextOverflow { .. })
    }

    /// Check if this error was caused by cancellation.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_format() {
        let err = RuntimeError::MaxIterations { iterations: 100 };
        assert_eq!(err.to_string(), "max iterations exceeded (100)");

        let err = RuntimeError::ContextOverflow { message: "too long".into() };
        assert_eq!(err.to_string(), "context overflow: too long");

        let err = RuntimeError::Cancelled;
        assert_eq!(err.to_string(), "cancelled");
    }

    #[test]
    fn error_classification() {
        assert!(RuntimeError::ContextOverflow { message: "x".into() }.is_context_overflow());
        assert!(!RuntimeError::Cancelled.is_context_overflow());
        assert!(RuntimeError::Cancelled.is_cancelled());
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RuntimeError>();
    }
}
