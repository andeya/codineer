use codineer_core::events::RuntimeEvent;
use codineer_core::observer::{EventDirective, RuntimeObserver};

/// Observer that emits structured tracing events for runtime lifecycle.
#[derive(Debug, Default)]
pub struct TracingObserver;

impl RuntimeObserver for TracingObserver {
    fn on_event(&mut self, event: &RuntimeEvent<'_>) -> EventDirective {
        match event {
            RuntimeEvent::SessionStart { session_id } => {
                tracing::info!(session_id, "session started");
            }
            RuntimeEvent::SessionEnd { session_id } => {
                tracing::info!(session_id, "session ended");
            }
            RuntimeEvent::TurnStart { iteration, turn } => {
                tracing::debug!(iteration, turn, "turn started");
            }
            RuntimeEvent::TurnEnd { iteration, turn } => {
                tracing::debug!(iteration, turn, "turn completed");
            }
            RuntimeEvent::PreToolUse {
                tool_name,
                tool_use_id,
                ..
            } => {
                tracing::debug!(tool_name, tool_use_id, "pre tool use");
            }
            RuntimeEvent::PostToolUse {
                tool_name,
                tool_use_id,
                is_error,
                ..
            } => {
                if *is_error {
                    tracing::warn!(tool_name, tool_use_id, "tool completed with error");
                } else {
                    tracing::debug!(tool_name, tool_use_id, "tool completed");
                }
            }
            RuntimeEvent::PostToolUseFailure {
                tool_name,
                tool_use_id,
                error,
            } => {
                tracing::warn!(tool_name, tool_use_id, error, "tool execution failed");
            }
            RuntimeEvent::RecoveryAttempt { strategy, attempt } => {
                tracing::warn!(%strategy, attempt, "recovery attempt");
            }
            RuntimeEvent::Stop { reason } => {
                tracing::info!(%reason, "runtime stopped");
            }
            RuntimeEvent::SlashCommandStart { command } => {
                tracing::debug!(command, "slash command started");
            }
            RuntimeEvent::SlashCommandComplete { command, success } => {
                tracing::debug!(command, success, "slash command completed");
            }
            _ => {
                tracing::trace!(kind = event.kind().as_ref(), "runtime event");
            }
        }
        EventDirective::allow()
    }
}
