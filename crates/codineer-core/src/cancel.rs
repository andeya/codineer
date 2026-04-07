//! Cooperative cancellation primitives.
//!
//! [`CancelToken`] is a lightweight, cloneable flag backed by `Arc<AtomicBool>`.
//! [`CancelGuard`] ensures the token is set on drop (RAII pattern).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A cloneable, thread-safe cancellation signal.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    /// Create a guard that cancels this token when dropped.
    #[must_use]
    pub fn guard(&self) -> CancelGuard {
        CancelGuard(self.clone())
    }
}

/// RAII guard that cancels the associated [`CancelToken`] on drop.
pub struct CancelGuard(CancelToken);

impl Drop for CancelGuard {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_starts_not_cancelled() {
        let token = CancelToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancel_sets_flag() {
        let token = CancelToken::new();
        let clone = token.clone();
        token.cancel();
        assert!(clone.is_cancelled());
    }

    #[test]
    fn guard_cancels_on_drop() {
        let token = CancelToken::new();
        {
            let _guard = token.guard();
            assert!(!token.is_cancelled());
        }
        assert!(token.is_cancelled());
    }
}
