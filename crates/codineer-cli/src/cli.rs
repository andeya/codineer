use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use api::ToolDefinition;
use commands::SlashCommand;
use runtime::{ConfigLoader, McpServerManager, PermissionMode};
use serde_json::json;
use tools::GlobalToolRegistry;

use crate::help::append_slash_command_suggestions;
use crate::reports::normalize_permission_mode;
use crate::{build_plugin_manager, current_date, default_model};

pub(crate) type AllowedToolSet = BTreeSet<String>;
pub(crate) type SharedMcpManager = Arc<Mutex<McpServerManager>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliAction {
    Agents {
        args: Option<String>,
    },
    Skills {
        args: Option<String>,
    },
    PrintSystemPrompt {
        cwd: PathBuf,
        date: String,
    },
    Version,
    ResumeSession {
        session_path: PathBuf,
        commands: Vec<String>,
    },
    Prompt {
        prompt: String,
        model: String,
        output_format: CliOutputFormat,
        allowed_tools: Option<AllowedToolSet>,
        permission_mode: PermissionMode,
    },
    Login {
        provider: Option<String>,
        source: Option<String>,
    },
    Logout {
        provider: Option<String>,
        source: Option<String>,
    },
    Status {
        provider: Option<String>,
    },
    Models {
        provider: Option<String>,
    },
    ConfigSet {
        key: String,
        value: String,
    },
    ConfigGet {
        key: Option<String>,
    },
    ConfigList,
    Init,
    Repl {
        model: String,
        allowed_tools: Option<AllowedToolSet>,
        permission_mode: PermissionMode,
        /// Path to a session file to restore before entering the REPL.
        resume_path: Option<PathBuf>,
    },
    Help,
    SubcommandHelp {
        name: &'static str,
        summary: &'static str,
        usage: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliOutputFormat {
    Text,
    Json,
}

impl CliOutputFormat {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            other => Err(format!(
                "unsupported value for --output-format: {other} (expected text or json)"
            )),
        }
    }
}

pub(crate) struct ParsedFlags {
    pub(crate) model: String,
    pub(crate) output_format: CliOutputFormat,
    pub(crate) permission_mode: PermissionMode,
    pub(crate) wants_version: bool,
    pub(crate) allowed_tool_values: Vec<String>,
    pub(crate) rest: Vec<String>,
}

pub(crate) fn parse_flags(args: &[String]) -> Result<ParsedFlags, String> {
    let mut flags = ParsedFlags {
        model: default_model(),
        output_format: CliOutputFormat::Text,
        permission_mode: default_permission_mode(),
        wants_version: false,
        allowed_tool_values: Vec::new(),
        rest: Vec::new(),
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--version" | "-V" => {
                flags.wants_version = true;
                index += 1;
            }
            "--model" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --model".to_string())?;
                flags.model = value.trim().to_string();
                index += 2;
            }
            flag if flag.starts_with("--model=") => {
                flags.model = flag[8..].trim().to_string();
                index += 1;
            }
            "--output-format" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --output-format".to_string())?;
                flags.output_format = CliOutputFormat::parse(value)?;
                index += 2;
            }
            "--permission-mode" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --permission-mode".to_string())?;
                flags.permission_mode = parse_permission_mode_arg(value)?;
                index += 2;
            }
            flag if flag.starts_with("--output-format=") => {
                flags.output_format = CliOutputFormat::parse(&flag[16..])?;
                index += 1;
            }
            flag if flag.starts_with("--permission-mode=") => {
                flags.permission_mode = parse_permission_mode_arg(&flag[18..])?;
                index += 1;
            }
            "--dangerously-skip-permissions" => {
                flags.permission_mode = PermissionMode::DangerFullAccess;
                index += 1;
            }
            "-p" => {
                let prompt = args[index + 1..].join(" ");
                if prompt.trim().is_empty() {
                    return Err("-p requires a prompt string".to_string());
                }
                flags.rest = vec!["-p".to_string(), prompt];
                return Ok(flags);
            }
            "--print" => {
                flags.output_format = CliOutputFormat::Text;
                index += 1;
            }
            "--allowedTools" | "--allowed-tools" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --allowedTools".to_string())?;
                flags.allowed_tool_values.push(value.clone());
                index += 2;
            }
            flag if flag.starts_with("--allowedTools=") => {
                flags.allowed_tool_values.push(flag[15..].to_string());
                index += 1;
            }
            flag if flag.starts_with("--allowed-tools=") => {
                flags.allowed_tool_values.push(flag[16..].to_string());
                index += 1;
            }
            other => {
                flags.rest.push(other.to_string());
                index += 1;
            }
        }
    }
    Ok(flags)
}

const SUBCOMMAND_HELP: &[(&str, &str, &str)] = &[
    (
        "agents",
        "List configured agents. Pass an optional query to filter.",
        "codineer agents [query]",
    ),
    (
        "skills",
        "List available skills. Pass an optional query to filter.",
        "codineer skills [query]",
    ),
    (
        "system-prompt",
        "Print the system prompt that would be sent to the model.",
        "codineer system-prompt [--cwd PATH] [--date YYYY-MM-DD]",
    ),
    (
        "login",
        "Start the login flow for a provider.",
        "codineer login [<provider>] [--source <id>]",
    ),
    (
        "logout",
        "Clear saved credentials for a provider.",
        "codineer logout [<provider>] [--source <id>]",
    ),
    (
        "status",
        "Show authentication status.",
        "codineer status [<provider>]",
    ),
    (
        "models",
        "List available models across providers.",
        "codineer models [<provider>]",
    ),
    (
        "config",
        "Manage settings (set, get, list).",
        "codineer config <set|get|list> [<key>] [<value>]",
    ),
    (
        "init",
        "Scaffold a CODINEER.md project context file in the current directory.",
        "codineer init",
    ),
];

/// All known CLI subcommand names (single source of truth for suggestion matching).
pub(crate) fn subcommand_names() -> Vec<String> {
    let mut names: Vec<String> = SUBCOMMAND_HELP
        .iter()
        .map(|(name, _, _)| (*name).to_string())
        .collect();
    names.push("help".to_string());
    names.push("prompt".to_string());
    names
}

pub(crate) fn parse_args(args: &[String]) -> Result<CliAction, String> {
    let flags = parse_flags(args)?;
    if flags.wants_version {
        return Ok(CliAction::Version);
    }

    let model = flags.model;
    let output_format = flags.output_format;
    let permission_mode = flags.permission_mode;
    let allowed_tools = normalize_allowed_tools(&flags.allowed_tool_values)?;
    let rest = flags.rest;

    if rest.first().map(String::as_str) == Some("-p") {
        return Ok(CliAction::Prompt {
            prompt: rest[1..].join(" "),
            model: model.clone(),
            output_format,
            allowed_tools,
            permission_mode,
        });
    }

    if rest.is_empty() {
        return Ok(CliAction::Repl {
            model,
            allowed_tools,
            permission_mode,
            resume_path: None,
        });
    }
    if is_help_flag(rest.first()) {
        return Ok(CliAction::Help);
    }
    if rest.first().map(String::as_str) == Some("--resume") {
        return parse_resume_args(&rest[1..], model, allowed_tools, permission_mode);
    }

    if let Some(&(name, summary, usage)) = SUBCOMMAND_HELP
        .iter()
        .find(|(n, _, _)| *n == rest[0].as_str())
    {
        if rest[1..].iter().any(|a| is_help_flag(Some(a))) {
            return Ok(CliAction::SubcommandHelp {
                name,
                summary,
                usage,
            });
        }
    }

    match rest[0].as_str() {
        "help" => Ok(CliAction::Help),
        "agents" => Ok(CliAction::Agents {
            args: join_optional_args(&rest[1..]),
        }),
        "skills" => Ok(CliAction::Skills {
            args: join_optional_args(&rest[1..]),
        }),
        "system-prompt" => parse_system_prompt_args(&rest[1..]),
        "login" => parse_auth_args(&rest[1..], |provider, source| CliAction::Login {
            provider,
            source,
        }),
        "logout" => parse_auth_args(&rest[1..], |provider, source| CliAction::Logout {
            provider,
            source,
        }),
        "status" => Ok(CliAction::Status {
            provider: parse_positional_arg(&rest[1..]),
        }),
        "models" => Ok(CliAction::Models {
            provider: parse_positional_arg(&rest[1..]),
        }),
        "config" => parse_config_args(&rest[1..]),
        "init" => Ok(CliAction::Init),
        "prompt" => {
            let prompt = rest[1..].join(" ");
            if prompt.trim().is_empty() {
                return Err("prompt subcommand requires a prompt string".to_string());
            }
            Ok(CliAction::Prompt {
                prompt,
                model,
                output_format,
                allowed_tools,
                permission_mode,
            })
        }
        other if other.starts_with('/') => parse_direct_slash_cli_action(&rest),
        other => {
            if let Some(suggestion) = crate::help::suggest_subcommand(other) {
                if suggestion != other {
                    eprintln!(
                        "\x1b[33mhint\x1b[0m: unknown command '{other}'. Did you mean '{suggestion}'?"
                    );
                }
            }
            Ok(CliAction::Prompt {
                prompt: rest.join(" "),
                model,
                output_format,
                allowed_tools,
                permission_mode,
            })
        }
    }
}

fn join_optional_args(args: &[String]) -> Option<String> {
    let joined = args.join(" ");
    let trimmed = joined.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub(crate) fn parse_direct_slash_cli_action(rest: &[String]) -> Result<CliAction, String> {
    let raw = rest.join(" ");
    match SlashCommand::parse(&raw) {
        Some(SlashCommand::Help) => Ok(CliAction::Help),
        Some(SlashCommand::Agents { args }) => Ok(CliAction::Agents { args }),
        Some(SlashCommand::Skills { args }) => Ok(CliAction::Skills { args }),
        Some(command) => Err(format_direct_slash_command_error(
            match &command {
                SlashCommand::Unknown(name) => format!("/{name}"),
                _ => rest[0].clone(),
            }
            .as_str(),
            matches!(command, SlashCommand::Unknown(_)),
        )),
        None => Err(format!("unknown subcommand: {}", rest[0])),
    }
}

pub(crate) fn format_direct_slash_command_error(command: &str, is_unknown: bool) -> String {
    let trimmed = command.trim().trim_start_matches('/');
    let mut lines = vec![
        "Direct slash command unavailable".to_string(),
        format!("  Command          /{trimmed}"),
    ];
    if is_unknown {
        append_slash_command_suggestions(&mut lines, trimmed);
    } else {
        lines.push(
            "  Try              Start `codineer` to use interactive slash commands".to_string(),
        );
        lines.push(
            "  Tip              Resume-safe commands also work with `codineer --resume SESSION.json ...`"
                .to_string(),
        );
    }
    lines.join("\n")
}

pub(crate) fn resolve_model_alias(
    model: &str,
    aliases: &std::collections::BTreeMap<String, String>,
) -> String {
    api::resolve_model_alias(model, aliases)
}

pub(crate) fn normalize_allowed_tools(values: &[String]) -> Result<Option<AllowedToolSet>, String> {
    if values.is_empty() {
        return Ok(None);
    }
    current_tool_registry()?.normalize_allowed_tools(values)
}

pub(crate) fn current_tool_registry() -> Result<GlobalToolRegistry, String> {
    let cwd = env::current_dir().map_err(|error| error.to_string())?;
    let loader = ConfigLoader::default_for(&cwd);
    let runtime_config = loader.load().map_err(|error| error.to_string())?;
    let plugin_manager = build_plugin_manager(&cwd, &loader, &runtime_config);
    let plugin_tools = plugin_manager
        .aggregated_tools()
        .map_err(|error| error.to_string())?;
    GlobalToolRegistry::with_plugin_tools(plugin_tools)
}

pub(crate) fn parse_permission_mode_arg(value: &str) -> Result<PermissionMode, String> {
    normalize_permission_mode(value)
        .ok_or_else(|| {
            format!(
                "unsupported permission mode '{value}'. Use read-only, workspace-write, or danger-full-access."
            )
        })
        .and_then(permission_mode_from_label)
}

pub(crate) fn permission_mode_from_label(mode: &str) -> Result<PermissionMode, String> {
    match mode {
        "read-only" => Ok(PermissionMode::ReadOnly),
        "workspace-write" => Ok(PermissionMode::WorkspaceWrite),
        "danger-full-access" => Ok(PermissionMode::DangerFullAccess),
        other => Err(format!(
            "unsupported permission mode '{other}'. Use read-only, workspace-write, or danger-full-access."
        )),
    }
}

pub(crate) fn default_permission_mode() -> PermissionMode {
    env::var("CODINEER_PERMISSION_MODE")
        .ok()
        .as_deref()
        .and_then(normalize_permission_mode)
        .and_then(|label| permission_mode_from_label(label).ok())
        .unwrap_or(PermissionMode::WorkspaceWrite)
}

pub(crate) fn filter_tool_specs(
    tool_registry: &GlobalToolRegistry,
    allowed_tools: Option<&AllowedToolSet>,
) -> Vec<ToolDefinition> {
    tool_registry.definitions(allowed_tools)
}

pub(crate) fn discover_mcp_tools(
    rt: &tokio::runtime::Runtime,
    mcp: &SharedMcpManager,
) -> Vec<ToolDefinition> {
    let Ok(mut guard) = mcp.lock() else {
        return Vec::new();
    };
    rt.block_on(guard.discover_tools())
        .unwrap_or_default()
        .into_iter()
        .map(|managed| ToolDefinition {
            name: managed.qualified_name,
            description: managed.tool.description,
            input_schema: managed
                .tool
                .input_schema
                .unwrap_or(json!({"type": "object"})),
        })
        .collect()
}

pub(crate) fn create_mcp_manager() -> SharedMcpManager {
    let cwd = env::current_dir().unwrap_or_default();
    let loader = ConfigLoader::default_for(&cwd);
    match loader.load() {
        Ok(config) => Arc::new(Mutex::new(McpServerManager::from_runtime_config(&config))),
        Err(_) => Arc::new(Mutex::new(McpServerManager::from_servers(
            &std::collections::BTreeMap::new(),
        ))),
    }
}

pub(crate) fn parse_system_prompt_args(args: &[String]) -> Result<CliAction, String> {
    let mut cwd = env::current_dir().map_err(|error| error.to_string())?;
    let mut date = current_date();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--cwd" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --cwd".to_string())?;
                cwd = PathBuf::from(value);
                index += 2;
            }
            "--date" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for --date".to_string())?;
                date.clone_from(value);
                index += 2;
            }
            other => return Err(format!("unknown system-prompt option: {other}")),
        }
    }

    Ok(CliAction::PrintSystemPrompt { cwd, date })
}

/// Parse `[<provider>] [--source <id>]` from auth command args.
fn parse_auth_args(
    args: &[String],
    build: impl FnOnce(Option<String>, Option<String>) -> CliAction,
) -> Result<CliAction, String> {
    let mut provider = None;
    let mut source = None;
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--source" {
            source = args.get(index + 1).cloned();
            index += 2;
        } else if is_help_flag(Some(&args[index])) {
            provider = None;
            source = None;
            break;
        } else if args[index].starts_with('-') {
            return Err(format!("unknown flag: {}", args[index]));
        } else if provider.is_none() {
            provider = Some(args[index].clone());
            index += 1;
        } else {
            return Err(format!("unexpected argument: {}", args[index]));
        }
    }
    Ok(build(provider, source))
}

fn parse_positional_arg(args: &[String]) -> Option<String> {
    args.first().filter(|s| !s.starts_with('-')).cloned()
}

fn parse_config_args(args: &[String]) -> Result<CliAction, String> {
    let subcmd = args.first().map(String::as_str).unwrap_or("list");
    match subcmd {
        "set" => {
            let key = args
                .get(1)
                .ok_or("usage: codineer config set <key> <value>")?;
            let value = args
                .get(2)
                .ok_or("usage: codineer config set <key> <value>")?;
            Ok(CliAction::ConfigSet {
                key: key.clone(),
                value: value.clone(),
            })
        }
        "get" => Ok(CliAction::ConfigGet {
            key: args.get(1).cloned(),
        }),
        "list" => Ok(CliAction::ConfigList),
        other => Err(format!(
            "unknown config subcommand: {other}\nusage: codineer config <set|get|list>"
        )),
    }
}

fn is_help_flag(arg: Option<&String>) -> bool {
    matches!(arg.map(String::as_str), Some("--help" | "-h"))
}

/// Parse arguments following `--resume <session-path> [/slash …]`.
///
/// `model`, `allowed_tools`, and `permission_mode` are the values already
/// resolved from the flags that preceded `--resume` so they are not silently
/// discarded when the user combines `--model`/`--permission-mode` with
/// `--resume`.
pub(crate) fn parse_resume_args(
    args: &[String],
    model: String,
    allowed_tools: Option<AllowedToolSet>,
    permission_mode: PermissionMode,
) -> Result<CliAction, String> {
    let session_path = args
        .first()
        .ok_or_else(|| "missing session path for --resume".to_string())
        .map(PathBuf::from)?;
    let commands = args[1..].to_vec();
    if commands
        .iter()
        .any(|command| !command.trim_start().starts_with('/'))
    {
        return Err("--resume trailing arguments must be slash commands".to_string());
    }
    // No slash commands → open the interactive REPL with the session restored.
    // With slash commands → run them non-interactively (batch / CI mode).
    if commands.is_empty() {
        return Ok(CliAction::Repl {
            model,
            allowed_tools,
            permission_mode,
            resume_path: Some(session_path),
        });
    }
    Ok(CliAction::ResumeSession {
        session_path,
        commands,
    })
}
