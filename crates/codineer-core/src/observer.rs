//! Observer trait for runtime event handling.
//!
//! [`RuntimeObserver`] receives [`RuntimeEvent`]s and returns an [`EventDirective`]
//! that can influence runtime control flow.
//!
//! Key design choices:
//! - `()` implements `RuntimeObserver` as a zero-cost no-op
//! - `(A, B)` tuples compose observers with zero vtable overhead
//! - All decisions default to `Allow`, making the common case free

use crate::events::RuntimeEvent;

/// Control flow decision returned by an observer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Decision {
    /// Allow the runtime to proceed normally.
    #[default]
    Allow,
    /// Deny the current operation with a reason.
    Deny(String),
    /// Override the operation's input/behavior (future extension).
    Override(String),
}

/// Directive returned by [`RuntimeObserver::on_event`].
///
/// Carries a control-flow [`Decision`] and optional context to inject
/// into the next prompt or message.
#[derive(Debug, Clone, Default)]
#[must_use]
pub struct EventDirective {
    pub decision: Decision,
    /// Optional messages to inject into the conversation.
    pub messages: Vec<String>,
    /// Additional context to append to the system prompt.
    pub additional_context: Option<String>,
}

impl EventDirective {
    pub fn allow() -> Self {
        Self::default()
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            decision: Decision::Deny(reason.into()),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn is_denied(&self) -> bool {
        matches!(self.decision, Decision::Deny(_))
    }

    #[must_use]
    pub fn deny_reason(&self) -> Option<&str> {
        match &self.decision {
            Decision::Deny(reason) => Some(reason),
            _ => None,
        }
    }
}

/// Trait for observing runtime lifecycle events.
///
/// Implement this trait to hook into the runtime at any of the 35 event points.
pub trait RuntimeObserver {
    fn on_event(&mut self, event: &RuntimeEvent<'_>) -> EventDirective;
}

/// `()` is a zero-cost no-op observer.
/// When `ConversationRuntime<C, T, ()>`, all observer calls compile away.
impl RuntimeObserver for () {
    #[inline(always)]
    fn on_event(&mut self, _event: &RuntimeEvent<'_>) -> EventDirective {
        EventDirective::allow()
    }
}

/// Tuple composition: `(A, B)` fans out events to both observers.
/// If either denies, the composed result denies.
impl<A: RuntimeObserver, B: RuntimeObserver> RuntimeObserver for (A, B) {
    fn on_event(&mut self, event: &RuntimeEvent<'_>) -> EventDirective {
        let a = self.0.on_event(event);
        if a.is_denied() {
            return a;
        }
        let mut b = self.1.on_event(event);
        // Merge messages from both
        if !a.messages.is_empty() {
            let mut merged = a.messages;
            merged.append(&mut b.messages);
            b.messages = merged;
        }
        // Prefer A's additional_context if B has none
        if b.additional_context.is_none() {
            b.additional_context = a.additional_context;
        }
        b
    }
}

/// Convenience macro for emitting an event and checking the directive.
#[macro_export]
macro_rules! emit_event {
    ($observer:expr, $event:expr) => {{
        $observer.on_event(&$event)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::RuntimeEvent;

    #[test]
    fn unit_observer_allows_everything() {
        let mut obs = ();
        let event = RuntimeEvent::TurnStart {
            iteration: 0,
            turn: 0,
        };
        let directive = obs.on_event(&event);
        assert!(!directive.is_denied());
        assert_eq!(directive.decision, Decision::Allow);
    }

    struct DenyAllObserver;
    impl RuntimeObserver for DenyAllObserver {
        fn on_event(&mut self, _event: &RuntimeEvent<'_>) -> EventDirective {
            EventDirective::deny("blocked")
        }
    }

    struct CountingObserver(usize);
    impl RuntimeObserver for CountingObserver {
        fn on_event(&mut self, _event: &RuntimeEvent<'_>) -> EventDirective {
            self.0 += 1;
            EventDirective::allow()
        }
    }

    #[test]
    fn tuple_composition_deny_propagates() {
        let mut composed = (DenyAllObserver, CountingObserver(0));
        let event = RuntimeEvent::TurnStart {
            iteration: 0,
            turn: 0,
        };
        let directive = composed.on_event(&event);
        assert!(directive.is_denied());
    }

    #[test]
    fn tuple_composition_both_called() {
        let mut composed = (CountingObserver(0), CountingObserver(0));
        let event = RuntimeEvent::TurnStart {
            iteration: 0,
            turn: 0,
        };
        let _ = composed.on_event(&event);
        assert_eq!(composed.0 .0, 1);
        assert_eq!(composed.1 .0, 1);
    }

    #[test]
    fn emit_event_macro_works() {
        let mut obs = ();
        let directive = emit_event!(obs, RuntimeEvent::SessionStart { session_id: "test" });
        assert!(!directive.is_denied());
    }
}
