//! Runtime event types for the observer pattern.
//!
//! [`EventKind`] is a `Copy` enum used as an O(1) dispatch key.
//! [`RuntimeEvent`] carries typed, borrowed payloads for zero-copy emission.

use std::borrow::Cow;
use strum::{AsRefStr, EnumString};

/// Lightweight dispatch key for event routing.
///
/// `Copy` + `Hash` + `Eq` enables O(1) `HashMap` lookups.
/// `strum` derives enable bidirectional string conversion for config files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, AsRefStr, EnumString, strum::Display)]
pub enum EventKind {
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    TurnStart,
    TurnEnd,
    ApiStreamStart,
    ApiStreamEnd,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    PermissionRequest,
    PermissionDenied,
    PreCompact,
    PostCompact,
    SubagentStart,
    SubagentStop,
    RecoveryAttempt,
    Stop,
    MaxTurnsReached,
    TokenBudgetContinue,
    StreamingFallback,
    MicrocompactApplied,
    ToolResultPersisted,
    SlashCommandStart,
    SlashCommandComplete,
    Notification,
    CwdChanged,
    ConfigChange,
    FileChanged,
    Setup,
    WorktreeCreate,
    WorktreeRemove,
    InstructionsLoaded,
    TaskCreated,
    TaskCompleted,
}

/// Rich event payload with borrowed data for zero-copy emission.
///
/// Each variant carries only references into the runtime's owned state,
/// avoiding allocation on every event.
#[derive(Debug)]
pub enum RuntimeEvent<'a> {
    SessionStart {
        session_id: &'a str,
    },
    SessionEnd {
        session_id: &'a str,
    },
    UserPromptSubmit {
        prompt: &'a str,
    },
    TurnStart {
        iteration: usize,
        turn: usize,
    },
    TurnEnd {
        iteration: usize,
        turn: usize,
    },
    ApiStreamStart {
        model: &'a str,
    },
    ApiStreamEnd {
        model: &'a str,
        input_tokens: usize,
        output_tokens: usize,
    },
    PreToolUse {
        tool_name: &'a str,
        tool_use_id: &'a str,
        input: &'a str,
    },
    PostToolUse {
        tool_name: &'a str,
        tool_use_id: &'a str,
        output: &'a str,
        is_error: bool,
    },
    PostToolUseFailure {
        tool_name: &'a str,
        tool_use_id: &'a str,
        error: &'a str,
    },
    PermissionRequest {
        tool_name: &'a str,
        input: &'a str,
    },
    PermissionDenied {
        tool_name: &'a str,
        reason: &'a str,
    },
    PreCompact {
        strategy: &'a str,
        message_count: usize,
    },
    PostCompact {
        strategy: &'a str,
        messages_removed: usize,
    },
    SubagentStart {
        agent_id: &'a str,
        depth: usize,
    },
    SubagentStop {
        agent_id: &'a str,
        depth: usize,
    },
    RecoveryAttempt {
        strategy: Cow<'a, str>,
        attempt: usize,
    },
    Stop {
        reason: Cow<'a, str>,
    },
    MaxTurnsReached {
        limit: usize,
    },
    TokenBudgetContinue {
        continuation_count: usize,
    },
    StreamingFallback,
    MicrocompactApplied {
        cleared_count: usize,
    },
    ToolResultPersisted {
        tool_use_id: &'a str,
        original_size: usize,
    },
    SlashCommandStart {
        command: &'a str,
    },
    SlashCommandComplete {
        command: &'a str,
        success: bool,
    },
    Notification {
        message: Cow<'a, str>,
    },
    CwdChanged {
        old: &'a str,
        new: &'a str,
    },
    ConfigChange {
        key: &'a str,
    },
    FileChanged {
        path: &'a str,
    },
    Setup {
        stage: &'a str,
    },
    WorktreeCreate {
        path: &'a str,
    },
    WorktreeRemove {
        path: &'a str,
    },
    InstructionsLoaded {
        count: usize,
    },
    TaskCreated {
        task_id: &'a str,
    },
    TaskCompleted {
        task_id: &'a str,
        success: bool,
    },
}

impl RuntimeEvent<'_> {
    /// Get the corresponding [`EventKind`] for dispatch.
    #[must_use]
    pub fn kind(&self) -> EventKind {
        match self {
            Self::SessionStart { .. } => EventKind::SessionStart,
            Self::SessionEnd { .. } => EventKind::SessionEnd,
            Self::UserPromptSubmit { .. } => EventKind::UserPromptSubmit,
            Self::TurnStart { .. } => EventKind::TurnStart,
            Self::TurnEnd { .. } => EventKind::TurnEnd,
            Self::ApiStreamStart { .. } => EventKind::ApiStreamStart,
            Self::ApiStreamEnd { .. } => EventKind::ApiStreamEnd,
            Self::PreToolUse { .. } => EventKind::PreToolUse,
            Self::PostToolUse { .. } => EventKind::PostToolUse,
            Self::PostToolUseFailure { .. } => EventKind::PostToolUseFailure,
            Self::PermissionRequest { .. } => EventKind::PermissionRequest,
            Self::PermissionDenied { .. } => EventKind::PermissionDenied,
            Self::PreCompact { .. } => EventKind::PreCompact,
            Self::PostCompact { .. } => EventKind::PostCompact,
            Self::SubagentStart { .. } => EventKind::SubagentStart,
            Self::SubagentStop { .. } => EventKind::SubagentStop,
            Self::RecoveryAttempt { .. } => EventKind::RecoveryAttempt,
            Self::Stop { .. } => EventKind::Stop,
            Self::MaxTurnsReached { .. } => EventKind::MaxTurnsReached,
            Self::TokenBudgetContinue { .. } => EventKind::TokenBudgetContinue,
            Self::StreamingFallback => EventKind::StreamingFallback,
            Self::MicrocompactApplied { .. } => EventKind::MicrocompactApplied,
            Self::ToolResultPersisted { .. } => EventKind::ToolResultPersisted,
            Self::SlashCommandStart { .. } => EventKind::SlashCommandStart,
            Self::SlashCommandComplete { .. } => EventKind::SlashCommandComplete,
            Self::Notification { .. } => EventKind::Notification,
            Self::CwdChanged { .. } => EventKind::CwdChanged,
            Self::ConfigChange { .. } => EventKind::ConfigChange,
            Self::FileChanged { .. } => EventKind::FileChanged,
            Self::Setup { .. } => EventKind::Setup,
            Self::WorktreeCreate { .. } => EventKind::WorktreeCreate,
            Self::WorktreeRemove { .. } => EventKind::WorktreeRemove,
            Self::InstructionsLoaded { .. } => EventKind::InstructionsLoaded,
            Self::TaskCreated { .. } => EventKind::TaskCreated,
            Self::TaskCompleted { .. } => EventKind::TaskCompleted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_kind_string_round_trip() {
        let kind = EventKind::PreToolUse;
        let s: &str = kind.as_ref();
        assert_eq!(s, "PreToolUse");
        let parsed: EventKind = "PreToolUse".parse().unwrap();
        assert_eq!(parsed, kind);
    }

    #[test]
    fn event_kind_is_copy() {
        let a = EventKind::TurnStart;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn runtime_event_kind_mapping() {
        let event = RuntimeEvent::TurnStart {
            iteration: 1,
            turn: 1,
        };
        assert_eq!(event.kind(), EventKind::TurnStart);

        let event = RuntimeEvent::Stop {
            reason: "test".into(),
        };
        assert_eq!(event.kind(), EventKind::Stop);
    }

    #[test]
    fn all_event_kinds_parseable() {
        let kinds = [
            "SessionStart",
            "SessionEnd",
            "UserPromptSubmit",
            "TurnStart",
            "TurnEnd",
            "ApiStreamStart",
            "ApiStreamEnd",
            "PreToolUse",
            "PostToolUse",
            "PostToolUseFailure",
            "PermissionRequest",
            "PermissionDenied",
            "PreCompact",
            "PostCompact",
            "SubagentStart",
            "SubagentStop",
            "RecoveryAttempt",
            "Stop",
            "MaxTurnsReached",
            "TokenBudgetContinue",
            "StreamingFallback",
            "MicrocompactApplied",
            "ToolResultPersisted",
            "SlashCommandStart",
            "SlashCommandComplete",
            "Notification",
            "CwdChanged",
            "ConfigChange",
            "FileChanged",
            "Setup",
            "WorktreeCreate",
            "WorktreeRemove",
            "InstructionsLoaded",
            "TaskCreated",
            "TaskCompleted",
        ];
        for kind_str in &kinds {
            let parsed: Result<EventKind, _> = kind_str.parse();
            assert!(parsed.is_ok(), "Failed to parse: {kind_str}");
        }
        assert_eq!(kinds.len(), 35);
    }
}
