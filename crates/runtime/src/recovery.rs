//! Tiered error recovery for API and runtime failures.
//!
//! Classifies errors into categories and selects the appropriate recovery
//! strategy, mirroring Claude Code's multi-layer error handling:
//! fallback → compact → collapse → escalate.

#![allow(dead_code)]

use codineer_core::loop_state::Transition;

/// Classification of an API error for recovery routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiErrorKind {
    /// Context is too large for the model's window.
    ContextOverflow,
    /// Rate-limited or server overloaded; safe to retry after backoff.
    Overloaded,
    /// Network transient (timeout, DNS, connection reset).
    NetworkTransient,
    /// Output was truncated by max_output_tokens.
    MaxOutputTokens,
    /// Invalid request (bad parameters, unsupported model feature).
    InvalidRequest,
    /// Auth failure (expired token, wrong key).
    Auth,
    /// Unknown / unrecoverable.
    Fatal,
}

/// Recovery strategy selected based on error classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryStrategy {
    /// Retry immediately (e.g., transient network error).
    Retry { max_attempts: usize },
    /// Attempt autocompact, then retry the API call.
    AutocompactRetry,
    /// Attempt reactive compact (more aggressive), then retry.
    ReactiveCompactRetry,
    /// Fall back to non-streaming mode.
    StreamingFallback,
    /// Escalate max_output_tokens and retry.
    EscalateMaxOutput { new_limit: usize },
    /// Inject a "please continue" message.
    ContinueRecovery { attempt: usize },
    /// Give up — error is unrecoverable.
    GiveUp { reason: String },
}

/// Classify a raw API error status and message into an [`ApiErrorKind`].
pub fn classify_api_error(
    status_code: u16,
    error_type: Option<&str>,
    message: Option<&str>,
) -> ApiErrorKind {
    let msg_lower = message.unwrap_or("").to_lowercase();
    let err_type = error_type.unwrap_or("");

    if status_code == 413
        || msg_lower.contains("context")
            && (msg_lower.contains("too long")
                || msg_lower.contains("overflow")
                || msg_lower.contains("exceeds"))
        || msg_lower.contains("prompt is too long")
        || msg_lower.contains("超长")
    {
        return ApiErrorKind::ContextOverflow;
    }

    if status_code == 429
        || err_type == "overloaded_error"
        || err_type == "rate_limit_error"
        || msg_lower.contains("rate limit")
        || msg_lower.contains("overloaded")
    {
        return ApiErrorKind::Overloaded;
    }

    if status_code == 401 || status_code == 403 || err_type == "authentication_error" {
        return ApiErrorKind::Auth;
    }

    if status_code == 400 && (err_type == "invalid_request_error" || msg_lower.contains("invalid"))
    {
        if msg_lower.contains("context") || msg_lower.contains("token") {
            return ApiErrorKind::ContextOverflow;
        }
        return ApiErrorKind::InvalidRequest;
    }

    if status_code >= 500 || status_code == 0 {
        return ApiErrorKind::NetworkTransient;
    }

    ApiErrorKind::Fatal
}

/// Select a recovery strategy based on the error classification and current state.
pub fn select_recovery(
    kind: &ApiErrorKind,
    autocompact_available: bool,
    reactive_available: bool,
    streaming_active: bool,
    attempt: usize,
) -> RecoveryStrategy {
    match kind {
        ApiErrorKind::ContextOverflow => {
            if autocompact_available {
                RecoveryStrategy::AutocompactRetry
            } else if reactive_available {
                RecoveryStrategy::ReactiveCompactRetry
            } else {
                RecoveryStrategy::GiveUp {
                    reason: "context overflow, all compaction strategies exhausted".into(),
                }
            }
        }
        ApiErrorKind::Overloaded | ApiErrorKind::NetworkTransient => {
            if attempt < 3 {
                RecoveryStrategy::Retry { max_attempts: 3 }
            } else {
                RecoveryStrategy::GiveUp {
                    reason: format!("retries exhausted after {attempt} attempts"),
                }
            }
        }
        ApiErrorKind::MaxOutputTokens => RecoveryStrategy::EscalateMaxOutput {
            new_limit: 16384,
        },
        ApiErrorKind::Auth | ApiErrorKind::InvalidRequest | ApiErrorKind::Fatal => {
            if streaming_active {
                RecoveryStrategy::StreamingFallback
            } else {
                RecoveryStrategy::GiveUp {
                    reason: format!("unrecoverable error: {kind:?}"),
                }
            }
        }
    }
}

/// Map a recovery strategy to a loop transition (for integration with the state machine).
pub fn recovery_to_transition(strategy: &RecoveryStrategy) -> Option<Transition> {
    match strategy {
        RecoveryStrategy::AutocompactRetry => Some(Transition::AutocompactRetry),
        RecoveryStrategy::ReactiveCompactRetry => Some(Transition::ReactiveCompactRetry),
        RecoveryStrategy::StreamingFallback => Some(Transition::StreamingFallbackRetry),
        RecoveryStrategy::EscalateMaxOutput { new_limit } => {
            Some(Transition::MaxOutputEscalation {
                escalated_to: *new_limit,
            })
        }
        RecoveryStrategy::ContinueRecovery { attempt } => {
            Some(Transition::MaxOutputRecovery { attempt: *attempt })
        }
        RecoveryStrategy::Retry { .. } => Some(Transition::NextTurn),
        RecoveryStrategy::GiveUp { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_context_overflow_by_status() {
        assert_eq!(
            classify_api_error(413, None, None),
            ApiErrorKind::ContextOverflow
        );
    }

    #[test]
    fn classify_context_overflow_by_message() {
        assert_eq!(
            classify_api_error(400, None, Some("prompt is too long for this model")),
            ApiErrorKind::ContextOverflow
        );
    }

    #[test]
    fn classify_chinese_context_overflow() {
        assert_eq!(
            classify_api_error(400, None, Some("您发送的文本超长啦")),
            ApiErrorKind::ContextOverflow
        );
    }

    #[test]
    fn classify_rate_limit() {
        assert_eq!(
            classify_api_error(429, Some("rate_limit_error"), None),
            ApiErrorKind::Overloaded
        );
    }

    #[test]
    fn classify_auth_error() {
        assert_eq!(
            classify_api_error(401, None, None),
            ApiErrorKind::Auth
        );
    }

    #[test]
    fn classify_server_error() {
        assert_eq!(
            classify_api_error(502, None, None),
            ApiErrorKind::NetworkTransient
        );
    }

    #[test]
    fn recovery_context_overflow_with_autocompact() {
        let strategy = select_recovery(&ApiErrorKind::ContextOverflow, true, true, false, 0);
        assert_eq!(strategy, RecoveryStrategy::AutocompactRetry);
    }

    #[test]
    fn recovery_context_overflow_fallback_reactive() {
        let strategy = select_recovery(&ApiErrorKind::ContextOverflow, false, true, false, 0);
        assert_eq!(strategy, RecoveryStrategy::ReactiveCompactRetry);
    }

    #[test]
    fn recovery_context_overflow_give_up() {
        let strategy = select_recovery(&ApiErrorKind::ContextOverflow, false, false, false, 0);
        assert!(matches!(strategy, RecoveryStrategy::GiveUp { .. }));
    }

    #[test]
    fn recovery_rate_limit_retry() {
        let strategy = select_recovery(&ApiErrorKind::Overloaded, false, false, false, 0);
        assert_eq!(strategy, RecoveryStrategy::Retry { max_attempts: 3 });
    }

    #[test]
    fn recovery_rate_limit_exhausted() {
        let strategy = select_recovery(&ApiErrorKind::Overloaded, false, false, false, 3);
        assert!(matches!(strategy, RecoveryStrategy::GiveUp { .. }));
    }

    #[test]
    fn recovery_auth_with_streaming_falls_back() {
        let strategy = select_recovery(&ApiErrorKind::Auth, false, false, true, 0);
        assert_eq!(strategy, RecoveryStrategy::StreamingFallback);
    }

    #[test]
    fn recovery_to_transition_mapping() {
        assert!(recovery_to_transition(&RecoveryStrategy::AutocompactRetry).is_some());
        assert!(recovery_to_transition(&RecoveryStrategy::GiveUp {
            reason: "done".into()
        })
        .is_none());
    }
}
