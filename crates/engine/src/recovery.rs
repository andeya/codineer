//! Tiered error recovery for API and runtime failures.
//!
//! [`RecoveryEngine`] encapsulates stateful recovery logic: classifying errors,
//! selecting strategies, and tracking attempts — mirroring Claude Code's
//! multi-layer error handling: fallback → compact → collapse → escalate.

use protocol::loop_state::Transition;

/// Token figures extracted from provider context-overflow error text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextOverflowInfo {
    pub prompt_tokens: Option<usize>,
    pub max_tokens: Option<usize>,
    pub overflow_tokens: Option<usize>,
}

/// Parse token counts from common API error messages like:
/// - "prompt is too long: 150000 tokens > 128000 maximum"
/// - "This request would exceed the token limit of 128000. Current usage: 150000 tokens"
pub fn parse_context_overflow(message: &str) -> Option<ContextOverflowInfo> {
    let mut info = ContextOverflowInfo {
        prompt_tokens: None,
        max_tokens: None,
        overflow_tokens: None,
    };

    if let Some(idx) = message.find("tokens >") {
        let left = message[..idx].trim_end();
        let right = message[idx + "tokens >".len()..].trim_start();
        info.prompt_tokens = last_number_in(left).or(info.prompt_tokens);
        info.max_tokens = first_number_in(right).or(info.max_tokens);
    }

    let lower = message.to_lowercase();
    if lower.contains("token limit") {
        if let Some(rest) = find_case_insensitive_tail(message, "token limit of") {
            info.max_tokens = first_number_in(rest).or(info.max_tokens);
        }
    }
    if lower.contains("current usage") {
        if let Some(rest) = find_case_insensitive_tail(message, "current usage:") {
            info.prompt_tokens = first_number_in(rest).or(info.prompt_tokens);
        }
    }

    match (info.prompt_tokens, info.max_tokens) {
        (Some(p), Some(m)) if p > m => {
            info.overflow_tokens = Some(p - m);
        }
        _ => {}
    }

    if info.prompt_tokens.is_none() && info.max_tokens.is_none() && info.overflow_tokens.is_none() {
        return None;
    }
    Some(info)
}

fn find_case_insensitive_tail<'a>(haystack: &'a str, needle: &str) -> Option<&'a str> {
    let hay = haystack.to_lowercase();
    let idx = hay.find(&needle.to_lowercase())?;
    Some(haystack[idx + needle.len()..].trim_start())
}

fn first_number_in(s: &str) -> Option<usize> {
    let mut started = false;
    let mut start_idx = 0;
    for (i, c) in s.char_indices() {
        if c.is_ascii_digit() {
            if !started {
                started = true;
                start_idx = i;
            }
        } else if started {
            return s[start_idx..i].parse().ok();
        }
    }
    if started {
        return s[start_idx..].parse().ok();
    }
    None
}

fn last_number_in(s: &str) -> Option<usize> {
    let mut end: Option<usize> = None;
    let mut start: Option<usize> = None;
    for (i, c) in s.char_indices().rev() {
        if c.is_ascii_digit() {
            if end.is_none() {
                end = Some(i + c.len_utf8());
            }
            start = Some(i);
        } else if start.is_some() {
            break;
        }
    }
    let (sidx, eidx) = (start?, end?);
    s[sidx..eidx].parse().ok()
}

/// Choose how many trailing messages to preserve for reactive compaction from overflow text.
#[must_use]
pub fn preserve_recent_for_reactive_compact(message: Option<&str>, total_messages: usize) -> usize {
    const FALLBACK: usize = 2;
    let msg = match message {
        Some(m) if !m.is_empty() => m,
        _ => return FALLBACK.min(total_messages.max(1)),
    };
    let Some(info) = parse_context_overflow(msg) else {
        return FALLBACK.min(total_messages.max(1));
    };
    let overflow = info
        .overflow_tokens
        .or_else(|| match (info.prompt_tokens, info.max_tokens) {
            (Some(p), Some(m)) if p > m => Some(p - m),
            _ => None,
        });
    let mut preserve = match overflow {
        None => FALLBACK,
        Some(o) if o < 2_000 => 8,
        Some(o) if o < 10_000 => 6,
        Some(o) if o < 30_000 => 4,
        Some(o) if o < 80_000 => 3,
        Some(_) => 2,
    };
    preserve = preserve.max(FALLBACK);
    preserve.min(total_messages.max(1))
}

/// Classification of an API error for recovery routing.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiErrorKind {
    /// Context is too large for the model's window.
    ContextOverflow,
    /// Rate-limited or server overloaded; safe to retry after backoff.
    Overloaded,
    /// Network transient (timeout, DNS, connection reset).
    NetworkTransient,
    /// Invalid request (bad parameters, unsupported model feature).
    InvalidRequest,
    /// Auth failure (expired token, wrong key).
    Auth,
    /// Unknown / unrecoverable.
    Fatal,
}

/// Recovery strategy selected based on error classification.
#[non_exhaustive]
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
    /// Give up — error is unrecoverable.
    GiveUp { reason: String },
}

/// Stateful recovery engine that tracks attempts and guards against infinite loops.
#[derive(Debug)]
pub struct RecoveryEngine {
    max_retries: usize,
    attempt: usize,
    autocompact_used: bool,
    reactive_used: bool,
    streaming_active: bool,
}

impl RecoveryEngine {
    #[must_use]
    pub fn new(streaming: bool) -> Self {
        Self {
            max_retries: 3,
            attempt: 0,
            autocompact_used: false,
            reactive_used: false,
            streaming_active: streaming,
        }
    }

    /// Classify a raw API error status and message into an [`ApiErrorKind`].
    pub fn classify(
        &self,
        status_code: u16,
        error_type: Option<&str>,
        message: Option<&str>,
    ) -> ApiErrorKind {
        ApiErrorKind::classify(status_code, error_type, message)
    }

    /// Select a recovery strategy based on the error classification and internal state.
    pub fn select_strategy(&self, kind: &ApiErrorKind) -> RecoveryStrategy {
        RecoveryStrategy::select(
            kind,
            !self.autocompact_used,
            !self.reactive_used,
            self.streaming_active,
            self.attempt,
        )
    }

    /// Map a recovery strategy to a loop transition.
    pub fn to_transition(&self, strategy: &RecoveryStrategy) -> Option<Transition> {
        strategy.to_transition()
    }

    /// Record that a recovery attempt was made, updating internal state guards.
    pub fn record_attempt(&mut self, strategy: &RecoveryStrategy) {
        self.attempt += 1;
        match strategy {
            RecoveryStrategy::AutocompactRetry => self.autocompact_used = true,
            RecoveryStrategy::ReactiveCompactRetry => self.reactive_used = true,
            RecoveryStrategy::StreamingFallback => self.streaming_active = false,
            _ => {}
        }
    }

    /// Reset attempt counter (called after a successful API call).
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// Whether retries have been exhausted.
    #[must_use]
    pub fn retries_exhausted(&self) -> bool {
        self.attempt >= self.max_retries
    }
}

impl ApiErrorKind {
    /// Classify a raw API error status and message.
    pub fn classify(status_code: u16, error_type: Option<&str>, message: Option<&str>) -> Self {
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
            return Self::ContextOverflow;
        }

        if status_code == 429
            || err_type == "overloaded_error"
            || err_type == "rate_limit_error"
            || msg_lower.contains("rate limit")
            || msg_lower.contains("overloaded")
        {
            return Self::Overloaded;
        }

        if status_code == 401 || status_code == 403 || err_type == "authentication_error" {
            return Self::Auth;
        }

        if status_code == 400
            && (err_type == "invalid_request_error" || msg_lower.contains("invalid"))
        {
            if msg_lower.contains("context") || msg_lower.contains("token") {
                return Self::ContextOverflow;
            }
            return Self::InvalidRequest;
        }

        if status_code >= 500 || status_code == 0 {
            return Self::NetworkTransient;
        }

        Self::Fatal
    }
}

impl RecoveryStrategy {
    /// Select a recovery strategy based on the error classification and current state.
    pub fn select(
        kind: &ApiErrorKind,
        autocompact_available: bool,
        reactive_available: bool,
        streaming_active: bool,
        attempt: usize,
    ) -> Self {
        match kind {
            ApiErrorKind::ContextOverflow => {
                if autocompact_available {
                    Self::AutocompactRetry
                } else if reactive_available {
                    Self::ReactiveCompactRetry
                } else {
                    Self::GiveUp {
                        reason: "context overflow, all compaction strategies exhausted".into(),
                    }
                }
            }
            ApiErrorKind::Overloaded | ApiErrorKind::NetworkTransient => {
                if attempt < 3 {
                    Self::Retry { max_attempts: 3 }
                } else {
                    Self::GiveUp {
                        reason: format!("retries exhausted after {attempt} attempts"),
                    }
                }
            }
            ApiErrorKind::Auth | ApiErrorKind::InvalidRequest | ApiErrorKind::Fatal => {
                if streaming_active {
                    Self::StreamingFallback
                } else {
                    Self::GiveUp {
                        reason: format!("unrecoverable error: {kind:?}"),
                    }
                }
            }
        }
    }

    /// Map to a loop transition for integration with the state machine.
    #[must_use]
    pub fn to_transition(&self) -> Option<Transition> {
        match self {
            Self::AutocompactRetry => Some(Transition::AutocompactRetry),
            Self::ReactiveCompactRetry => Some(Transition::ReactiveCompactRetry),
            Self::StreamingFallback => Some(Transition::StreamingFallbackRetry),
            Self::Retry { .. } => Some(Transition::NextTurn),
            Self::GiveUp { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_context_overflow_by_status() {
        assert_eq!(
            ApiErrorKind::classify(413, None, None),
            ApiErrorKind::ContextOverflow
        );
    }

    #[test]
    fn classify_context_overflow_by_message() {
        assert_eq!(
            ApiErrorKind::classify(400, None, Some("prompt is too long for this model")),
            ApiErrorKind::ContextOverflow
        );
    }

    #[test]
    fn classify_chinese_context_overflow() {
        assert_eq!(
            ApiErrorKind::classify(400, None, Some("您发送的文本超长啦")),
            ApiErrorKind::ContextOverflow
        );
    }

    #[test]
    fn classify_rate_limit() {
        assert_eq!(
            ApiErrorKind::classify(429, Some("rate_limit_error"), None),
            ApiErrorKind::Overloaded
        );
    }

    #[test]
    fn classify_auth_error() {
        assert_eq!(ApiErrorKind::classify(401, None, None), ApiErrorKind::Auth);
    }

    #[test]
    fn classify_server_error() {
        assert_eq!(
            ApiErrorKind::classify(502, None, None),
            ApiErrorKind::NetworkTransient
        );
    }

    #[test]
    fn recovery_context_overflow_with_autocompact() {
        let strategy =
            RecoveryStrategy::select(&ApiErrorKind::ContextOverflow, true, true, false, 0);
        assert_eq!(strategy, RecoveryStrategy::AutocompactRetry);
    }

    #[test]
    fn recovery_context_overflow_fallback_reactive() {
        let strategy =
            RecoveryStrategy::select(&ApiErrorKind::ContextOverflow, false, true, false, 0);
        assert_eq!(strategy, RecoveryStrategy::ReactiveCompactRetry);
    }

    #[test]
    fn recovery_context_overflow_give_up() {
        let strategy =
            RecoveryStrategy::select(&ApiErrorKind::ContextOverflow, false, false, false, 0);
        assert!(matches!(strategy, RecoveryStrategy::GiveUp { .. }));
    }

    #[test]
    fn recovery_rate_limit_retry() {
        let strategy = RecoveryStrategy::select(&ApiErrorKind::Overloaded, false, false, false, 0);
        assert_eq!(strategy, RecoveryStrategy::Retry { max_attempts: 3 });
    }

    #[test]
    fn recovery_rate_limit_exhausted() {
        let strategy = RecoveryStrategy::select(&ApiErrorKind::Overloaded, false, false, false, 3);
        assert!(matches!(strategy, RecoveryStrategy::GiveUp { .. }));
    }

    #[test]
    fn recovery_auth_with_streaming_falls_back() {
        let strategy = RecoveryStrategy::select(&ApiErrorKind::Auth, false, false, true, 0);
        assert_eq!(strategy, RecoveryStrategy::StreamingFallback);
    }

    #[test]
    fn recovery_to_transition_mapping() {
        assert!(RecoveryStrategy::AutocompactRetry.to_transition().is_some());
        assert!(RecoveryStrategy::GiveUp {
            reason: "done".into()
        }
        .to_transition()
        .is_none());
    }

    // ── RecoveryEngine method tests ─────────────────────────────────

    #[test]
    fn engine_classify_delegates_correctly() {
        let engine = RecoveryEngine::new(true);
        assert_eq!(
            engine.classify(429, Some("rate_limit_error"), None),
            ApiErrorKind::Overloaded
        );
    }

    #[test]
    fn engine_tracks_attempts_and_guards() {
        let mut engine = RecoveryEngine::new(true);
        assert!(!engine.retries_exhausted());

        let kind = ApiErrorKind::Overloaded;
        let strategy = engine.select_strategy(&kind);
        assert_eq!(strategy, RecoveryStrategy::Retry { max_attempts: 3 });
        engine.record_attempt(&strategy);

        engine.record_attempt(&RecoveryStrategy::Retry { max_attempts: 3 });
        engine.record_attempt(&RecoveryStrategy::Retry { max_attempts: 3 });
        assert!(engine.retries_exhausted());

        let strategy = engine.select_strategy(&kind);
        assert!(matches!(strategy, RecoveryStrategy::GiveUp { .. }));
    }

    #[test]
    fn engine_autocompact_guard() {
        let mut engine = RecoveryEngine::new(false);
        let kind = ApiErrorKind::ContextOverflow;

        let s1 = engine.select_strategy(&kind);
        assert_eq!(s1, RecoveryStrategy::AutocompactRetry);
        engine.record_attempt(&s1);

        let s2 = engine.select_strategy(&kind);
        assert_eq!(s2, RecoveryStrategy::ReactiveCompactRetry);
        engine.record_attempt(&s2);

        let s3 = engine.select_strategy(&kind);
        assert!(matches!(s3, RecoveryStrategy::GiveUp { .. }));
    }

    #[test]
    fn engine_streaming_fallback_guard() {
        let mut engine = RecoveryEngine::new(true);
        let kind = ApiErrorKind::Fatal;

        let s1 = engine.select_strategy(&kind);
        assert_eq!(s1, RecoveryStrategy::StreamingFallback);
        engine.record_attempt(&s1);

        let s2 = engine.select_strategy(&kind);
        assert!(matches!(s2, RecoveryStrategy::GiveUp { .. }));
    }

    #[test]
    fn engine_reset_clears_attempts() {
        let mut engine = RecoveryEngine::new(true);
        engine.record_attempt(&RecoveryStrategy::Retry { max_attempts: 3 });
        engine.record_attempt(&RecoveryStrategy::Retry { max_attempts: 3 });
        engine.record_attempt(&RecoveryStrategy::Retry { max_attempts: 3 });
        assert!(engine.retries_exhausted());
        engine.reset();
        assert!(!engine.retries_exhausted());
    }

    #[test]
    fn parse_context_overflow_tokens_gt_pattern() {
        let msg = "prompt is too long: 150000 tokens > 128000 maximum";
        let info = parse_context_overflow(msg).expect("parsed");
        assert_eq!(info.prompt_tokens, Some(150_000));
        assert_eq!(info.max_tokens, Some(128_000));
        assert_eq!(info.overflow_tokens, Some(22_000));
    }

    #[test]
    fn parse_context_overflow_limit_and_usage_pattern() {
        let msg =
            "This request would exceed the token limit of 128000. Current usage: 150000 tokens";
        let info = parse_context_overflow(msg).expect("parsed");
        assert_eq!(info.max_tokens, Some(128_000));
        assert_eq!(info.prompt_tokens, Some(150_000));
        assert_eq!(info.overflow_tokens, Some(22_000));
    }

    #[test]
    fn parse_context_overflow_returns_none_when_unrecognized() {
        assert!(parse_context_overflow("something went wrong").is_none());
    }

    #[test]
    fn preserve_recent_scales_with_overflow_magnitude() {
        let small = "prompt is too long: 128100 tokens > 128000 maximum";
        assert_eq!(preserve_recent_for_reactive_compact(Some(small), 20), 8);

        let huge = "prompt is too long: 250000 tokens > 128000 maximum";
        assert_eq!(preserve_recent_for_reactive_compact(Some(huge), 20), 2);
    }
}
