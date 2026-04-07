use std::collections::HashMap;
use std::ffi::OsStr;
use std::process::Command;

use serde_json::json;

use codineer_core::events::{EventKind, RuntimeEvent};
use codineer_core::observer::{Decision, EventDirective, RuntimeObserver};

use crate::config::{RuntimeFeatureConfig, RuntimeHookConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
}

impl HookEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
        }
    }
}

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

pub trait HookCommandSource {
    fn pre_tool_use_commands(&self) -> &[String];
    fn post_tool_use_commands(&self) -> &[String];
}

impl HookCommandSource for RuntimeHookConfig {
    fn pre_tool_use_commands(&self) -> &[String] {
        self.pre_tool_use()
    }

    fn post_tool_use_commands(&self) -> &[String] {
        self.post_tool_use()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookRunner<S: HookCommandSource> {
    source: S,
}

impl<S: HookCommandSource> HookRunner<S> {
    #[must_use]
    pub fn new(source: S) -> Self {
        Self { source }
    }

    #[must_use]
    pub fn run_pre_tool_use(&self, tool_name: &str, tool_input: &str) -> HookRunResult {
        run_hook_commands(
            HookEvent::PreToolUse,
            self.source.pre_tool_use_commands(),
            tool_name,
            tool_input,
            None,
            false,
        )
    }

    #[must_use]
    pub fn run_post_tool_use(
        &self,
        tool_name: &str,
        tool_input: &str,
        tool_output: &str,
        is_error: bool,
    ) -> HookRunResult {
        run_hook_commands(
            HookEvent::PostToolUse,
            self.source.post_tool_use_commands(),
            tool_name,
            tool_input,
            Some(tool_output),
            is_error,
        )
    }
}

impl HookRunner<RuntimeHookConfig> {
    #[must_use]
    pub fn from_feature_config(feature_config: &RuntimeFeatureConfig) -> Self {
        Self::new(feature_config.hooks().clone())
    }
}

impl<S: HookCommandSource + Default> Default for HookRunner<S> {
    fn default() -> Self {
        Self::new(S::default())
    }
}

#[derive(Debug, Clone, Copy)]
struct HookContext<'a> {
    event: HookEvent,
    tool_name: &'a str,
    tool_input: &'a str,
    tool_output: Option<&'a str>,
    is_error: bool,
    payload: &'a str,
}

pub fn run_hook_commands(
    event: HookEvent,
    commands: &[String],
    tool_name: &str,
    tool_input: &str,
    tool_output: Option<&str>,
    is_error: bool,
) -> HookRunResult {
    if commands.is_empty() {
        return HookRunResult::allow(Vec::new());
    }

    let payload = json!({
        "hook_event_name": event.as_str(),
        "tool_name": tool_name,
        "tool_input": parse_tool_input(tool_input),
        "tool_input_json": tool_input,
        "tool_output": tool_output,
        "tool_result_is_error": is_error,
    })
    .to_string();

    let ctx = HookContext {
        event,
        tool_name,
        tool_input,
        tool_output,
        is_error,
        payload: &payload,
    };

    let mut messages = Vec::new();

    for command in commands {
        match run_hook_command(command, &ctx) {
            HookCommandOutcome::Allow { message } => {
                if let Some(message) = message {
                    messages.push(message);
                }
            }
            HookCommandOutcome::Deny { message } => {
                messages.push(message.unwrap_or_else(|| {
                    format!("{} hook denied tool `{tool_name}`", event.as_str())
                }));
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

fn run_hook_command(command: &str, ctx: &HookContext<'_>) -> HookCommandOutcome {
    let mut child = shell_command(command);
    child.stdin(std::process::Stdio::piped());
    child.stdout(std::process::Stdio::piped());
    child.stderr(std::process::Stdio::piped());
    child.env("HOOK_EVENT", ctx.event.as_str());
    child.env("HOOK_TOOL_NAME", ctx.tool_name);
    child.env("HOOK_TOOL_INPUT", ctx.tool_input);
    child.env("HOOK_TOOL_IS_ERROR", if ctx.is_error { "1" } else { "0" });
    if let Some(tool_output) = ctx.tool_output {
        child.env("HOOK_TOOL_OUTPUT", tool_output);
    }

    match child.output_with_stdin(ctx.payload.as_bytes()) {
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
                    message: format!(
                        "{} hook `{command}` terminated by signal while handling `{}`",
                        ctx.event.as_str(),
                        ctx.tool_name,
                    ),
                },
            }
        }
        Err(error) => HookCommandOutcome::Warn {
            message: format!(
                "{} hook `{command}` failed to start for `{}`: {error}",
                ctx.event.as_str(),
                ctx.tool_name,
            ),
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
        format!("Hook `{command}` exited with status {code}; allowing tool execution to continue");
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
/// Bridges the old `HookRunner` mechanism with the new observer pattern.
#[derive(Debug)]
pub struct HookDispatcher {
    commands: HashMap<EventKind, Vec<String>>,
}

impl HookDispatcher {
    /// Build from a `RuntimeHookConfig`.
    ///
    /// Uses strum's `EventKind::from_str` to parse event names from the config map.
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

    /// Build from a `RuntimeFeatureConfig` (convenience).
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

impl Default for HookDispatcher {
    fn default() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }
}

impl RuntimeObserver for HookDispatcher {
    fn on_event(&mut self, event: &RuntimeEvent<'_>) -> EventDirective {
        let kind = event.kind();
        let Some(commands) = self.commands.get(&kind) else {
            return EventDirective::allow();
        };

        match event {
            RuntimeEvent::PreToolUse {
                tool_name, input, ..
            } => {
                let result = run_hook_commands(
                    HookEvent::PreToolUse,
                    commands,
                    tool_name,
                    input,
                    None,
                    false,
                );
                hook_result_to_directive(result)
            }
            RuntimeEvent::PostToolUse {
                tool_name,
                tool_use_id: _,
                output,
                is_error,
            } => {
                let result = run_hook_commands(
                    HookEvent::PostToolUse,
                    commands,
                    tool_name,
                    "",
                    Some(output),
                    *is_error,
                );
                hook_result_to_directive(result)
            }
            RuntimeEvent::PostToolUseFailure {
                tool_name, error, ..
            } => {
                let result = run_hook_commands(
                    HookEvent::PostToolUse,
                    commands,
                    tool_name,
                    "",
                    Some(error),
                    true,
                );
                hook_result_to_directive(result)
            }
            _ => {
                // For non-tool events with registered commands, we don't
                // have a standard input format yet — just allow.
                EventDirective::allow()
            }
        }
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
    use super::{HookDispatcher, HookRunResult, HookRunner};
    use crate::config::{RuntimeFeatureConfig, RuntimeHookConfig};
    use codineer_core::events::{EventKind, RuntimeEvent};
    use codineer_core::observer::RuntimeObserver;

    #[test]
    fn allows_exit_code_zero_and_captures_stdout() {
        let runner = HookRunner::new(RuntimeHookConfig::new(
            vec!["printf 'pre ok'".to_string()],
            Vec::new(),
        ));

        let result = runner.run_pre_tool_use("Read", r#"{"path":"README.md"}"#);

        assert_eq!(result, HookRunResult::allow(vec!["pre ok".to_string()]));
    }

    #[test]
    fn denies_exit_code_two() {
        let runner = HookRunner::new(RuntimeHookConfig::new(
            vec!["printf 'blocked by hook'; exit 2".to_string()],
            Vec::new(),
        ));

        let result = runner.run_pre_tool_use("Bash", r#"{"command":"pwd"}"#);

        assert!(result.is_denied());
        assert_eq!(result.messages(), &["blocked by hook".to_string()]);
    }

    #[test]
    fn warns_for_other_non_zero_statuses() {
        let runner = HookRunner::from_feature_config(&RuntimeFeatureConfig::default().with_hooks(
            RuntimeHookConfig::new(
                vec!["printf 'warning hook'; exit 1".to_string()],
                Vec::new(),
            ),
        ));

        let result = runner.run_pre_tool_use("Edit", r#"{"file":"src/lib.rs"}"#);

        assert!(!result.is_denied());
        assert!(result
            .messages()
            .iter()
            .any(|message| message.contains("allowing tool execution to continue")));
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
    fn dispatcher_allows_pre_tool_use() {
        let config = RuntimeHookConfig::new(
            vec!["printf 'allowed'".to_string()],
            Vec::new(),
        );
        let mut dispatcher = HookDispatcher::from_hook_config(&config);
        let event = RuntimeEvent::PreToolUse {
            tool_name: "Read",
            tool_use_id: "id-1",
            input: r#"{"path":"README.md"}"#,
        };
        let directive = dispatcher.on_event(&event);
        assert!(!directive.is_denied());
        assert_eq!(directive.messages, vec!["allowed".to_string()]);
    }

    #[test]
    fn dispatcher_denies_pre_tool_use() {
        let config = RuntimeHookConfig::new(
            vec!["printf 'blocked'; exit 2".to_string()],
            Vec::new(),
        );
        let mut dispatcher = HookDispatcher::from_hook_config(&config);
        let event = RuntimeEvent::PreToolUse {
            tool_name: "Bash",
            tool_use_id: "id-2",
            input: r#"{"command":"rm"}"#,
        };
        let directive = dispatcher.on_event(&event);
        assert!(directive.is_denied());
    }

    #[test]
    fn dispatcher_allows_unregistered_events() {
        let mut dispatcher = HookDispatcher::default();
        let event = RuntimeEvent::TurnStart { iteration: 0, turn: 0 };
        let directive = dispatcher.on_event(&event);
        assert!(!directive.is_denied());
    }

    #[test]
    fn dispatcher_register_custom_event() {
        let mut dispatcher = HookDispatcher::default();
        dispatcher.register(EventKind::SessionStart, vec!["echo hello".to_string()]);
        assert_eq!(dispatcher.registered_count(), 1);
    }
}
