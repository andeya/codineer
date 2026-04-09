use std::collections::HashMap;
use std::ffi::OsStr;
use std::process::Command;

use serde_json::{json, Value};

use protocol::events::{EventKind, RuntimeEvent};
use protocol::observer::{Decision, EventDirective, RuntimeObserver};

use crate::config::{RuntimeFeatureConfig, RuntimeHookConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRunResult {
    denied: bool,
    messages: Vec<String>,
}

impl HookRunResult {
    #[must_use]
    pub fn allow(messages: Vec<String>) -> Self {
        Self {
            denied: false,
            messages,
        }
    }

    #[must_use]
    pub fn is_denied(&self) -> bool {
        self.denied
    }

    #[must_use]
    pub fn messages(&self) -> &[String] {
        &self.messages
    }
}

enum ToolHookBody<'a> {
    Pre { input: &'a str },
    Post { output: &'a str, is_error: bool },
    Failure { error: &'a str },
}

fn tool_event_payload(tool_name: &str, tool_use_id: &str, body: ToolHookBody<'_>) -> Value {
    match body {
        ToolHookBody::Pre { input } => json!({
            "event": "PreToolUse",
            "tool_name": tool_name,
            "tool_use_id": tool_use_id,
            "tool_input": parse_tool_input(input),
            "tool_input_json": input,
        }),
        ToolHookBody::Post { output, is_error } => json!({
            "event": "PostToolUse",
            "tool_name": tool_name,
            "tool_use_id": tool_use_id,
            "tool_output": output,
            "tool_result_is_error": is_error,
        }),
        ToolHookBody::Failure { error } => json!({
            "event": "PostToolUseFailure",
            "tool_name": tool_name,
            "tool_use_id": tool_use_id,
            "tool_output": error,
            "tool_result_is_error": true,
        }),
    }
}

fn turn_event_payload(event_name: &str, iteration: usize, turn: usize) -> Value {
    json!({
        "event": event_name,
        "iteration": iteration,
        "turn": turn,
    })
}

/// Build a JSON payload from any [`RuntimeEvent`] for hook stdin.
fn build_hook_payload(event: &RuntimeEvent<'_>) -> Value {
    match event {
        RuntimeEvent::PreToolUse {
            tool_name,
            tool_use_id,
            input,
        } => tool_event_payload(tool_name, tool_use_id, ToolHookBody::Pre { input }),
        RuntimeEvent::PostToolUse {
            tool_name,
            tool_use_id,
            output,
            is_error,
        } => tool_event_payload(
            tool_name,
            tool_use_id,
            ToolHookBody::Post {
                output,
                is_error: *is_error,
            },
        ),
        RuntimeEvent::PostToolUseFailure {
            tool_name,
            tool_use_id,
            error,
        } => tool_event_payload(tool_name, tool_use_id, ToolHookBody::Failure { error }),
        RuntimeEvent::SessionStart { session_id } => json!({
            "event": "SessionStart",
            "session_id": session_id,
        }),
        RuntimeEvent::SessionEnd { session_id } => json!({
            "event": "SessionEnd",
            "session_id": session_id,
        }),
        RuntimeEvent::UserPromptSubmit { prompt } => json!({
            "event": "UserPromptSubmit",
            "prompt": prompt,
        }),
        RuntimeEvent::TurnStart { iteration, turn } => {
            turn_event_payload("TurnStart", *iteration, *turn)
        }
        RuntimeEvent::TurnEnd { iteration, turn } => {
            turn_event_payload("TurnEnd", *iteration, *turn)
        }
        RuntimeEvent::SubagentStart { agent_id, depth } => json!({
            "event": "SubagentStart",
            "agent_id": agent_id,
            "depth": depth,
        }),
        RuntimeEvent::SubagentStop { agent_id, depth } => json!({
            "event": "SubagentStop",
            "agent_id": agent_id,
            "depth": depth,
        }),
        RuntimeEvent::Stop { reason } => json!({
            "event": "Stop",
            "reason": reason.as_ref(),
        }),
        RuntimeEvent::Notification { message } => json!({
            "event": "Notification",
            "message": message.as_ref(),
        }),
        RuntimeEvent::CwdChanged { old, new } => json!({
            "event": "CwdChanged",
            "old_cwd": old,
            "new_cwd": new,
        }),
        RuntimeEvent::ConfigChange { key } => json!({
            "event": "ConfigChange",
            "key": key,
        }),
        RuntimeEvent::FileChanged { path } => json!({
            "event": "FileChanged",
            "path": path,
        }),
        RuntimeEvent::TaskCreated { task_id } => json!({
            "event": "TaskCreated",
            "task_id": task_id,
        }),
        RuntimeEvent::TaskCompleted { task_id, success } => json!({
            "event": "TaskCompleted",
            "task_id": task_id,
            "success": success,
        }),
        RuntimeEvent::ToolProgress {
            tool_use_id,
            tool_name,
            progress,
        } => json!({
            "event": "ToolProgress",
            "tool_use_id": tool_use_id,
            "tool_name": tool_name,
            "progress": progress,
        }),
        _ => json!({ "event": event.kind().as_ref() }),
    }
}

fn run_commands(event_kind: EventKind, commands: &[String], payload: &Value) -> HookRunResult {
    if commands.is_empty() {
        return HookRunResult::allow(Vec::new());
    }
    let payload_str = payload.to_string();
    let event_name = event_kind.as_ref();

    let mut messages = Vec::new();

    for command in commands {
        match run_hook_command(command, event_name, &payload_str) {
            HookCommandOutcome::Allow { message } => {
                if let Some(message) = message {
                    messages.push(message);
                }
            }
            HookCommandOutcome::Deny { message } => {
                messages
                    .push(message.unwrap_or_else(|| format!("{event_name} hook denied execution")));
                return HookRunResult {
                    denied: true,
                    messages,
                };
            }
            HookCommandOutcome::Warn { message } => messages.push(message),
        }
    }

    HookRunResult::allow(messages)
}

fn run_hook_command(command: &str, event_name: &str, payload: &str) -> HookCommandOutcome {
    let mut child = shell_command(command);
    child.stdin(std::process::Stdio::piped());
    child.stdout(std::process::Stdio::piped());
    child.stderr(std::process::Stdio::piped());
    child.env("HOOK_EVENT", event_name);

    match child.output_with_stdin(payload.as_bytes()) {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let message = (!stdout.is_empty()).then_some(stdout);
            const HOOK_EXIT_ALLOW: i32 = 0;
            const HOOK_EXIT_DENY: i32 = 2;
            match output.status.code() {
                Some(HOOK_EXIT_ALLOW) => HookCommandOutcome::Allow { message },
                Some(HOOK_EXIT_DENY) => HookCommandOutcome::Deny { message },
                Some(code) => HookCommandOutcome::Warn {
                    message: format_hook_warning(
                        command,
                        code,
                        message.as_deref(),
                        stderr.as_str(),
                    ),
                },
                None => HookCommandOutcome::Warn {
                    message: format!("{event_name} hook `{command}` terminated by signal",),
                },
            }
        }
        Err(error) => HookCommandOutcome::Warn {
            message: format!("{event_name} hook `{command}` failed to start: {error}",),
        },
    }
}

enum HookCommandOutcome {
    Allow { message: Option<String> },
    Deny { message: Option<String> },
    Warn { message: String },
}

fn parse_tool_input(tool_input: &str) -> serde_json::Value {
    serde_json::from_str(tool_input).unwrap_or_else(|_| json!({ "raw": tool_input }))
}

fn format_hook_warning(command: &str, code: i32, stdout: Option<&str>, stderr: &str) -> String {
    let mut message =
        format!("Hook `{command}` exited with status {code}; allowing execution to continue");
    if let Some(stdout) = stdout.filter(|stdout| !stdout.is_empty()) {
        message.push_str(": ");
        message.push_str(stdout);
    } else if !stderr.is_empty() {
        message.push_str(": ");
        message.push_str(stderr);
    }
    message
}

fn shell_command(command: &str) -> CommandWithStdin {
    #[cfg(windows)]
    let command_builder = {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        CommandWithStdin::new(cmd)
    };

    #[cfg(not(windows))]
    let command_builder = if std::path::Path::new(command).exists() {
        let mut cmd = Command::new("sh");
        cmd.arg(command);
        CommandWithStdin::new(cmd)
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-lc").arg(command);
        CommandWithStdin::new(cmd)
    };

    command_builder
}

struct CommandWithStdin {
    command: Command,
}

impl CommandWithStdin {
    fn new(command: Command) -> Self {
        Self { command }
    }

    fn stdin(&mut self, cfg: std::process::Stdio) -> &mut Self {
        self.command.stdin(cfg);
        self
    }

    fn stdout(&mut self, cfg: std::process::Stdio) -> &mut Self {
        self.command.stdout(cfg);
        self
    }

    fn stderr(&mut self, cfg: std::process::Stdio) -> &mut Self {
        self.command.stderr(cfg);
        self
    }

    fn env<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.command.env(key, value);
        self
    }

    fn output_with_stdin(&mut self, stdin: &[u8]) -> std::io::Result<std::process::Output> {
        let mut child = self.command.spawn()?;
        if let Some(mut child_stdin) = child.stdin.take() {
            use std::io::Write as _;
            if let Err(error) = child_stdin.write_all(stdin) {
                if error.kind() != std::io::ErrorKind::BrokenPipe {
                    return Err(error);
                }
            }
        }
        child.wait_with_output()
    }
}

/// Event-driven hook dispatcher implementing [`RuntimeObserver`].
///
/// Routes [`RuntimeEvent`]s to shell commands via `HashMap<EventKind>` for O(1) lookup.
/// All events are dispatched uniformly with a generic JSON payload on stdin.
#[derive(Debug, Default)]
pub struct HookDispatcher {
    commands: HashMap<EventKind, Vec<String>>,
}

impl HookDispatcher {
    /// Build from a [`RuntimeHookConfig`].
    ///
    /// Parses event names from the config map via `EventKind::from_str`.
    /// Unrecognized event names are silently ignored.
    #[must_use]
    pub fn from_hook_config(config: &RuntimeHookConfig) -> Self {
        let mut commands = HashMap::new();
        for (event_name, cmds) in config.commands() {
            if let Ok(kind) = event_name.parse::<EventKind>() {
                if !cmds.is_empty() {
                    commands.insert(kind, cmds.clone());
                }
            }
        }
        Self { commands }
    }

    /// Build from a [`RuntimeFeatureConfig`] (convenience).
    #[must_use]
    pub fn from_feature_config(feature_config: &RuntimeFeatureConfig) -> Self {
        Self::from_hook_config(feature_config.hooks())
    }

    /// Register commands for an arbitrary [`EventKind`].
    pub fn register(&mut self, kind: EventKind, cmds: Vec<String>) {
        if !cmds.is_empty() {
            self.commands.insert(kind, cmds);
        }
    }

    /// How many event kinds have registered commands.
    #[must_use]
    pub fn registered_count(&self) -> usize {
        self.commands.len()
    }
}

impl RuntimeObserver for HookDispatcher {
    fn on_event(&mut self, event: &RuntimeEvent<'_>) -> EventDirective {
        let kind = event.kind();
        let Some(commands) = self.commands.get(&kind) else {
            return EventDirective::allow();
        };
        let payload = build_hook_payload(event);
        let result = run_commands(kind, commands, &payload);
        hook_result_to_directive(result)
    }
}

fn hook_result_to_directive(result: HookRunResult) -> EventDirective {
    let messages: Vec<String> = result.messages().to_vec();
    if result.is_denied() {
        let reason = messages
            .first()
            .cloned()
            .unwrap_or_else(|| "hook denied".to_string());
        EventDirective {
            decision: Decision::Deny(reason),
            messages,
            additional_context: None,
        }
    } else if messages.is_empty() {
        EventDirective::allow()
    } else {
        EventDirective {
            decision: Decision::Allow,
            messages,
            additional_context: None,
        }
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::{HookDispatcher, HookRunResult};
    use crate::config::RuntimeHookConfig;
    use protocol::events::{EventKind, RuntimeEvent};
    use protocol::observer::RuntimeObserver;

    #[test]
    fn allows_exit_code_zero_and_captures_stdout() {
        let config = RuntimeHookConfig::new(vec!["printf 'pre ok'".to_string()], Vec::new());
        let mut dispatcher = HookDispatcher::from_hook_config(&config);
        let event = RuntimeEvent::PreToolUse {
            tool_name: "Read",
            tool_use_id: "id-1",
            input: r#"{"path":"README.md"}"#,
        };
        let directive = dispatcher.on_event(&event);
        assert!(!directive.is_denied());
        assert_eq!(directive.messages, vec!["pre ok".to_string()]);
    }

    #[test]
    fn denies_exit_code_two() {
        let config = RuntimeHookConfig::new(
            vec!["printf 'blocked by hook'; exit 2".to_string()],
            Vec::new(),
        );
        let mut dispatcher = HookDispatcher::from_hook_config(&config);
        let event = RuntimeEvent::PreToolUse {
            tool_name: "Bash",
            tool_use_id: "id-2",
            input: r#"{"command":"pwd"}"#,
        };
        let directive = dispatcher.on_event(&event);
        assert!(directive.is_denied());
        assert!(directive
            .messages
            .iter()
            .any(|m| m.contains("blocked by hook")));
    }

    #[test]
    fn warns_for_other_non_zero_statuses() {
        let config = RuntimeHookConfig::new(
            vec!["printf 'warning hook'; exit 1".to_string()],
            Vec::new(),
        );
        let mut dispatcher = HookDispatcher::from_hook_config(&config);
        let event = RuntimeEvent::PreToolUse {
            tool_name: "Edit",
            tool_use_id: "id-3",
            input: r#"{"file":"src/lib.rs"}"#,
        };
        let directive = dispatcher.on_event(&event);
        assert!(!directive.is_denied());
        assert!(directive
            .messages
            .iter()
            .any(|m| m.contains("allowing execution to continue")));
    }

    #[test]
    fn dispatcher_from_hook_config() {
        let config = RuntimeHookConfig::new(
            vec!["printf 'pre ok'".to_string()],
            vec!["printf 'post ok'".to_string()],
        );
        let dispatcher = HookDispatcher::from_hook_config(&config);
        assert_eq!(dispatcher.registered_count(), 2);
    }

    #[test]
    fn dispatcher_allows_unregistered_events() {
        let mut dispatcher = HookDispatcher::default();
        let event = RuntimeEvent::TurnStart {
            iteration: 0,
            turn: 0,
        };
        let directive = dispatcher.on_event(&event);
        assert!(!directive.is_denied());
    }

    #[test]
    fn dispatcher_register_custom_event() {
        let mut dispatcher = HookDispatcher::default();
        dispatcher.register(EventKind::SessionStart, vec!["echo hello".to_string()]);
        assert_eq!(dispatcher.registered_count(), 1);
    }

    #[test]
    fn generic_dispatch_for_session_start() {
        let mut dispatcher = HookDispatcher::default();
        dispatcher.register(
            EventKind::SessionStart,
            vec!["printf 'session started'".to_string()],
        );
        let event = RuntimeEvent::SessionStart { session_id: "s-1" };
        let directive = dispatcher.on_event(&event);
        assert!(!directive.is_denied());
        assert_eq!(directive.messages, vec!["session started".to_string()]);
    }

    #[test]
    fn generic_dispatch_for_stop() {
        let mut dispatcher = HookDispatcher::default();
        dispatcher.register(EventKind::Stop, vec!["printf 'stopping'".to_string()]);
        let event = RuntimeEvent::Stop {
            reason: "user request".into(),
        };
        let directive = dispatcher.on_event(&event);
        assert!(!directive.is_denied());
        assert_eq!(directive.messages, vec!["stopping".to_string()]);
    }

    #[test]
    fn generic_dispatch_for_user_prompt() {
        let mut dispatcher = HookDispatcher::default();
        dispatcher.register(
            EventKind::UserPromptSubmit,
            vec!["printf 'prompt received'".to_string()],
        );
        let event = RuntimeEvent::UserPromptSubmit { prompt: "hello" };
        let directive = dispatcher.on_event(&event);
        assert!(!directive.is_denied());
        assert_eq!(directive.messages, vec!["prompt received".to_string()]);
    }

    #[test]
    fn generic_dispatch_for_subagent_start() {
        let mut dispatcher = HookDispatcher::default();
        dispatcher.register(
            EventKind::SubagentStart,
            vec!["printf 'agent launched'".to_string()],
        );
        let event = RuntimeEvent::SubagentStart {
            agent_id: "a-1",
            depth: 0,
        };
        let directive = dispatcher.on_event(&event);
        assert!(!directive.is_denied());
        assert_eq!(directive.messages, vec!["agent launched".to_string()]);
    }

    #[test]
    fn low_level_run_commands_allow() {
        let payload = serde_json::json!({"event": "PreToolUse", "tool_name": "Read"});
        let result = super::run_commands(
            EventKind::PreToolUse,
            &["printf 'pre ok'".to_string()],
            &payload,
        );
        assert_eq!(result, HookRunResult::allow(vec!["pre ok".to_string()]));
    }

    #[test]
    fn low_level_run_commands_deny() {
        let payload = serde_json::json!({"event": "PreToolUse", "tool_name": "Bash"});
        let result = super::run_commands(
            EventKind::PreToolUse,
            &["printf 'blocked by hook'; exit 2".to_string()],
            &payload,
        );
        assert!(result.is_denied());
        assert_eq!(result.messages(), &["blocked by hook".to_string()]);
    }
}
