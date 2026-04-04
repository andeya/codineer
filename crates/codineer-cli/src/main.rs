mod auth;
mod cli;
mod config_cmd;
mod help;
mod init;
mod input;
mod live_cli;
mod models_cmd;
mod progress;
mod render;
mod reports;
mod resume;
mod runtime_client;
mod session_store;
mod style;
mod tool_display;
mod workspace;

use std::env;
use std::path::{Path, PathBuf};

use init::initialize_repo;
use plugins::{PluginManager, PluginManagerConfig};
use runtime::{load_system_prompt_with_lsp, ConfigLoader, LspContextEnrichment};
use tools::GlobalToolRegistry;

use auth::{run_login, run_logout, run_status};
use config_cmd::{run_config_get, run_config_list, run_config_set};

use cli::{parse_args, CliAction};
use help::print_help;
use live_cli::{run_repl, LiveCli};
use reports::render_version_report;
use resume::resume_session;

pub(crate) fn default_model() -> String {
    api::auto_detect_default_model()
        .unwrap_or("auto")
        .to_string()
}

pub(crate) fn max_tokens_for_model(model: &str) -> u32 {
    if model.contains("opus") {
        32_000
    } else {
        64_000
    }
}

pub(crate) fn current_date() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = i64::try_from(secs / 86400).unwrap_or(0);
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = u32::try_from(z - era * 146_097).unwrap_or(0);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = i32::try_from(i64::from(yoe) + era * 400).unwrap_or(0);
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const BUILD_TARGET: Option<&str> = option_env!("TARGET");
pub(crate) const GIT_SHA: Option<&str> = option_env!("GIT_SHA");

pub(crate) fn logo_ascii(color: bool) -> String {
    let p = style::Palette::new(color);
    if color {
        [
            format!("{}          ▄██▄{}", p.violet, p.r),
            format!("{}       ▄██▀  ▀██▄{}", p.violet, p.r),
            format!(
                "{}      ██  {}❯{}     ██{}     {}C O D I N E E R{}",
                p.violet, p.cyan_fg, p.violet, p.r, p.bold_white, p.r,
            ),
            format!(
                "{}      ██     {}▍{}  ██{}     {}Your local AI coding agent{}",
                p.violet, p.amber, p.violet, p.r, p.dim, p.r,
            ),
            format!("{}       ▀██▄  ▄██▀{}", p.violet, p.r),
            format!("{}          ▀██▀{}", p.violet, p.r),
        ]
        .join("\n")
    } else {
        [
            "          ▄██▄",
            "       ▄██▀  ▀██▄",
            "      ██  ❯     ██     C O D I N E E R",
            "      ██     ▍  ██     Your local AI coding agent",
            "       ▀██▄  ▄██▀",
            "          ▀██▀",
        ]
        .join("\n")
    }
}

pub(crate) fn logo_line(color: bool) -> String {
    let p = style::Palette::new(color);
    if color {
        format!(
            "{}◆{} {}❯{}▍{} {}Codineer{}",
            p.violet, p.r, p.cyan_fg, p.amber, p.r, p.bold_white, p.r,
        )
    } else {
        "◆ ❯▍ Codineer".to_string()
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{}", render_cli_error(&error.to_string()));
        std::process::exit(1);
    }
}

fn render_cli_error(problem: &str) -> String {
    use std::fmt::Write;
    let p = style::Palette::for_stderr();
    let mut out = String::from("\n");
    let mut lines = problem.lines();

    if let Some(summary) = lines.next() {
        let _ = writeln!(
            out,
            "  {}✖ Error:{} {}{}{}",
            p.bold_red, p.r, p.bold_white, summary, p.r
        );
    }
    for line in lines {
        if line.is_empty() {
            out.push('\n');
        } else {
            let _ = writeln!(out, "    {}", highlight_cli_hint(&p, line));
        }
    }
    let _ = writeln!(out, "\n  {}codineer --help{}", p.dim, p.r);
    out
}

fn highlight_cli_hint(p: &style::Palette, line: &str) -> String {
    if let Some(idx) = line.find("export ") {
        let (prefix, cmd) = line.split_at(idx);
        format!("{}{}{}{}", prefix, p.cyan_fg, cmd, p.r)
    } else if line.trim_start().starts_with("codineer ") {
        format!("{}{}{}", p.cyan_fg, line, p.r)
    } else {
        line.to_string()
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match parse_args(&args)? {
        CliAction::Agents { args } => LiveCli::print_agents(args.as_deref())?,
        CliAction::Skills { args } => LiveCli::print_skills(args.as_deref())?,
        CliAction::PrintSystemPrompt { cwd, date } => print_system_prompt(cwd, date)?,
        CliAction::Version => print_version(),
        CliAction::ResumeSession {
            session_path,
            commands,
        } => resume_session(&session_path, &commands),
        CliAction::Prompt {
            prompt,
            model,
            output_format,
            allowed_tools,
            permission_mode,
        } => LiveCli::new(model, true, allowed_tools, permission_mode)?
            .run_turn_with_output(&prompt, output_format)?,
        CliAction::Login { provider, source } => {
            run_login(provider.as_deref(), source.as_deref())?;
        }
        CliAction::Logout { provider, source } => {
            run_logout(provider.as_deref(), source.as_deref())?;
        }
        CliAction::Status { provider } => run_status(provider.as_deref())?,
        CliAction::Models { provider } => models_cmd::run_models(provider.as_deref())?,
        CliAction::ConfigSet { key, value } => run_config_set(&key, &value)?,
        CliAction::ConfigGet { key } => run_config_get(key.as_deref())?,
        CliAction::ConfigList => run_config_list()?,
        CliAction::Init => run_init()?,
        CliAction::Repl {
            model,
            allowed_tools,
            permission_mode,
        } => run_repl(model, allowed_tools, permission_mode)?,
        CliAction::Help => print_help(),
        CliAction::SubcommandHelp {
            name,
            summary,
            usage,
        } => {
            println!("codineer {name}\n  {summary}\n\nUsage:\n  {usage}");
        }
    }
    Ok(())
}

fn print_system_prompt(cwd: PathBuf, date: String) -> Result<(), Box<dyn std::error::Error>> {
    let sections = load_system_prompt_with_lsp(cwd, date, env::consts::OS, "unknown", None)?;
    println!("{}", sections.join("\n\n"));
    Ok(())
}

fn print_version() {
    println!("{}", render_version_report());
}

pub(crate) fn init_codineer_md() -> Result<String, Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    Ok(initialize_repo(&cwd)?.render())
}

fn run_init() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", init_codineer_md()?);
    Ok(())
}

fn build_system_prompt() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    build_system_prompt_with_lsp(None)
}

fn build_system_prompt_with_lsp(
    lsp_context: Option<&LspContextEnrichment>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    Ok(load_system_prompt_with_lsp(
        env::current_dir()?,
        current_date(),
        env::consts::OS,
        "unknown",
        lsp_context,
    )?)
}

pub(crate) fn build_runtime_plugin_state(
) -> Result<(runtime::RuntimeConfig, GlobalToolRegistry), Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let loader = ConfigLoader::default_for(&cwd);
    let runtime_config = loader.load()?;
    apply_config_env(&runtime_config);
    let plugin_manager = build_plugin_manager(&cwd, &loader, &runtime_config);
    let tool_registry = GlobalToolRegistry::with_plugin_tools(plugin_manager.aggregated_tools()?)?;
    Ok((runtime_config, tool_registry))
}

/// Apply the `"env"` section from config to process environment variables.
/// Only sets variables not already present so explicit shell exports take precedence.
fn apply_config_env(config: &runtime::RuntimeConfig) {
    for (key, value) in config.env_section() {
        if env::var_os(&key).is_none() {
            env::set_var(&key, &value);
        }
    }
}

pub(crate) fn build_plugin_manager(
    cwd: &Path,
    loader: &ConfigLoader,
    runtime_config: &runtime::RuntimeConfig,
) -> PluginManager {
    let plugin_settings = runtime_config.plugins();
    let mut plugin_config = PluginManagerConfig::new(loader.config_home().to_path_buf());
    plugin_config.enabled_plugins = plugin_settings.enabled_plugins().clone();
    plugin_config.external_dirs = plugin_settings
        .external_directories()
        .iter()
        .map(|path| resolve_plugin_path(cwd, loader.config_home(), path))
        .collect();
    plugin_config.install_root = plugin_settings
        .install_root()
        .map(|path| resolve_plugin_path(cwd, loader.config_home(), path));
    plugin_config.registry_path = plugin_settings
        .registry_path()
        .map(|path| resolve_plugin_path(cwd, loader.config_home(), path));
    plugin_config.bundled_root = plugin_settings
        .bundled_root()
        .map(|path| resolve_plugin_path(cwd, loader.config_home(), path));
    PluginManager::new(plugin_config)
}

fn resolve_plugin_path(cwd: &Path, config_home: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else if value.starts_with('.') {
        cwd.join(path)
    } else {
        config_home.join(path)
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::{
        filter_tool_specs, parse_args, resolve_model_alias, CliAction, CliOutputFormat,
    };
    use crate::default_model;
    use crate::help::{
        print_help_to, render_repl_help, render_unknown_repl_command,
        slash_command_completion_candidates,
    };
    use crate::progress::{
        describe_tool_progress, format_internal_prompt_progress_line, InternalPromptProgressEvent,
        InternalPromptProgressState,
    };
    use crate::reports::{
        format_compact_report, format_cost_report, format_model_report, format_model_switch_report,
        format_permissions_report, format_permissions_switch_report, format_resume_report,
        format_status_report, normalize_permission_mode, render_config_report,
        render_memory_report, status_context, StatusContext, StatusUsage,
    };
    use crate::runtime_client::{
        convert_messages, permission_policy, push_output_block, response_to_events,
    };
    use crate::tool_display::{format_tool_call_start, format_tool_result};
    use crate::workspace::parse_git_status_metadata;

    use api::{MessageResponse, OutputContentBlock, Usage};
    use commands::{resume_supported_slash_commands, SlashCommand};
    use plugins::{PluginTool, PluginToolDefinition, PluginToolPermission};
    use runtime::{AssistantEvent, ContentBlock, ConversationMessage, MessageRole, PermissionMode};
    use serde_json::json;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;
    use tools::GlobalToolRegistry;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn registry_with_plugin_tool() -> GlobalToolRegistry {
        GlobalToolRegistry::with_plugin_tools(vec![PluginTool::new(
            "plugin-demo@external",
            "plugin-demo",
            PluginToolDefinition {
                name: "plugin_echo".to_string(),
                description: Some("Echo plugin payload".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    },
                    "required": ["message"],
                    "additionalProperties": false
                }),
            },
            "echo".to_string(),
            Vec::new(),
            PluginToolPermission::WorkspaceWrite,
            None,
        )])
        .expect("plugin tool registry should build")
    }

    #[test]
    fn defaults_to_repl_when_no_args() {
        assert_eq!(
            parse_args(&[]).expect("args should parse"),
            CliAction::Repl {
                model: default_model(),
                allowed_tools: None,
                permission_mode: PermissionMode::WorkspaceWrite,
            }
        );
    }

    #[test]
    fn parses_prompt_subcommand() {
        let args = vec![
            "prompt".to_string(),
            "hello".to_string(),
            "world".to_string(),
        ];
        assert_eq!(
            parse_args(&args).expect("args should parse"),
            CliAction::Prompt {
                prompt: "hello world".to_string(),
                model: default_model(),
                output_format: CliOutputFormat::Text,
                allowed_tools: None,
                permission_mode: PermissionMode::WorkspaceWrite,
            }
        );
    }

    #[test]
    fn parses_bare_prompt_and_json_output_flag() {
        let args = vec![
            "--output-format=json".to_string(),
            "--model".to_string(),
            "custom-opus".to_string(),
            "explain".to_string(),
            "this".to_string(),
        ];
        assert_eq!(
            parse_args(&args).expect("args should parse"),
            CliAction::Prompt {
                prompt: "explain this".to_string(),
                model: "custom-opus".to_string(),
                output_format: CliOutputFormat::Json,
                allowed_tools: None,
                permission_mode: PermissionMode::WorkspaceWrite,
            }
        );
    }

    #[test]
    fn resolves_model_aliases_in_args() {
        let args = vec![
            "--model".to_string(),
            "opus".to_string(),
            "explain".to_string(),
            "this".to_string(),
        ];
        assert_eq!(
            parse_args(&args).expect("args should parse"),
            CliAction::Prompt {
                prompt: "explain this".to_string(),
                model: "claude-opus-4-6".to_string(),
                output_format: CliOutputFormat::Text,
                allowed_tools: None,
                permission_mode: PermissionMode::WorkspaceWrite,
            }
        );
    }

    #[test]
    fn resolves_known_model_aliases() {
        assert_eq!(resolve_model_alias("opus"), "claude-opus-4-6");
        assert_eq!(resolve_model_alias("sonnet"), "claude-sonnet-4-6");
        assert_eq!(resolve_model_alias("haiku"), "claude-haiku-4-5-20251213");
        assert_eq!(resolve_model_alias("grok"), "grok-3");
        assert_eq!(resolve_model_alias("grok-mini"), "grok-3-mini");
        assert_eq!(resolve_model_alias("custom-opus"), "custom-opus");
    }

    #[test]
    fn parses_version_flags_without_initializing_prompt_mode() {
        assert_eq!(
            parse_args(&["--version".to_string()]).expect("args should parse"),
            CliAction::Version
        );
        assert_eq!(
            parse_args(&["-V".to_string()]).expect("args should parse"),
            CliAction::Version
        );
    }

    #[test]
    fn parses_permission_mode_flag() {
        let args = vec!["--permission-mode=read-only".to_string()];
        assert_eq!(
            parse_args(&args).expect("args should parse"),
            CliAction::Repl {
                model: default_model(),
                allowed_tools: None,
                permission_mode: PermissionMode::ReadOnly,
            }
        );
    }

    #[test]
    fn parses_allowed_tools_flags_with_aliases_and_lists() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let tmp = std::env::temp_dir().join("codineer-test-allowed-tools");
        let _ = std::fs::create_dir_all(&tmp);
        let prev = std::env::var("CODINEER_CONFIG_HOME").ok();
        std::env::set_var("CODINEER_CONFIG_HOME", &tmp);

        let args = vec![
            "--allowedTools".to_string(),
            "read,glob".to_string(),
            "--allowed-tools=write_file".to_string(),
        ];
        let result = parse_args(&args);

        if let Some(v) = prev {
            std::env::set_var("CODINEER_CONFIG_HOME", v);
        } else {
            std::env::remove_var("CODINEER_CONFIG_HOME");
        }
        let _ = std::fs::remove_dir_all(tmp);

        assert_eq!(
            result.expect("args should parse"),
            CliAction::Repl {
                model: default_model(),
                allowed_tools: Some(
                    ["glob_search", "read_file", "write_file"]
                        .into_iter()
                        .map(str::to_string)
                        .collect()
                ),
                permission_mode: PermissionMode::WorkspaceWrite,
            }
        );
    }

    #[test]
    fn rejects_unknown_allowed_tools() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let tmp = std::env::temp_dir().join("codineer-test-unknown-tools");
        let _ = std::fs::create_dir_all(&tmp);
        let prev = std::env::var("CODINEER_CONFIG_HOME").ok();
        std::env::set_var("CODINEER_CONFIG_HOME", &tmp);

        let result = parse_args(&["--allowedTools".to_string(), "teleport".to_string()]);

        if let Some(v) = prev {
            std::env::set_var("CODINEER_CONFIG_HOME", v);
        } else {
            std::env::remove_var("CODINEER_CONFIG_HOME");
        }
        let _ = std::fs::remove_dir_all(tmp);

        let error = result.expect_err("tool should be rejected");
        assert!(error.contains("unsupported tool in --allowedTools: teleport"));
    }

    #[test]
    fn parses_system_prompt_options() {
        let args = vec![
            "system-prompt".to_string(),
            "--cwd".to_string(),
            "/tmp/project".to_string(),
            "--date".to_string(),
            "2026-04-01".to_string(),
        ];
        assert_eq!(
            parse_args(&args).expect("args should parse"),
            CliAction::PrintSystemPrompt {
                cwd: PathBuf::from("/tmp/project"),
                date: "2026-04-01".to_string(),
            }
        );
    }

    #[test]
    fn parses_help_subcommand() {
        assert_eq!(
            parse_args(&["help".to_string()]).expect("help should parse"),
            CliAction::Help
        );
        assert_eq!(
            parse_args(&["--help".to_string()]).expect("--help should parse"),
            CliAction::Help
        );
        assert_eq!(
            parse_args(&["-h".to_string()]).expect("-h should parse"),
            CliAction::Help
        );
    }

    #[test]
    fn help_output_includes_environment_variables_section() {
        let mut output = Vec::new();
        print_help_to(&mut output).expect("help should write");
        let text = String::from_utf8(output).expect("valid utf-8");
        assert!(text.contains("Codineer"));
        assert!(text.contains("Environment variables"));
        assert!(text.contains("ANTHROPIC_API_KEY"));
        assert!(text.contains("XAI_API_KEY"));
        assert!(text.contains("OPENAI_API_KEY"));
        assert!(text.contains("CODINEER_WORKSPACE_ROOT"));
        assert!(text.contains("NO_COLOR"));
        assert!(text.contains("Configuration files"));
        assert!(text.contains("codineer help"));
        assert!(text.contains("Authentication sources"));
        assert!(text.contains("codineer status"));
        assert!(text.contains("Claude Code"));
        assert!(text.contains("credentials"));
    }

    #[test]
    fn parses_login_and_logout_subcommands() {
        assert_eq!(
            parse_args(&["login".to_string()]).expect("login should parse"),
            CliAction::Login {
                provider: None,
                source: None
            }
        );
        assert_eq!(
            parse_args(&["logout".to_string()]).expect("logout should parse"),
            CliAction::Logout {
                provider: None,
                source: None
            }
        );
        assert_eq!(
            parse_args(&["login".to_string(), "anthropic".to_string()])
                .expect("login <provider> should parse"),
            CliAction::Login {
                provider: Some("anthropic".to_string()),
                source: None,
            }
        );
        assert_eq!(
            parse_args(&[
                "login".to_string(),
                "--source".to_string(),
                "claude-code".to_string()
            ])
            .expect("login --source should parse"),
            CliAction::Login {
                provider: None,
                source: Some("claude-code".to_string()),
            }
        );
        assert_eq!(
            parse_args(&[
                "login".to_string(),
                "anthropic".to_string(),
                "--source".to_string(),
                "claude-code".to_string()
            ])
            .expect("login <provider> --source should parse"),
            CliAction::Login {
                provider: Some("anthropic".to_string()),
                source: Some("claude-code".to_string()),
            }
        );
        assert_eq!(
            parse_args(&[
                "logout".to_string(),
                "--source".to_string(),
                "codineer-oauth".to_string()
            ])
            .expect("logout --source should parse"),
            CliAction::Logout {
                provider: None,
                source: Some("codineer-oauth".to_string()),
            }
        );
        assert_eq!(
            parse_args(&["status".to_string()]).expect("status should parse"),
            CliAction::Status { provider: None }
        );
        assert_eq!(
            parse_args(&["status".to_string(), "anthropic".to_string()])
                .expect("status <provider> should parse"),
            CliAction::Status {
                provider: Some("anthropic".to_string())
            }
        );
        assert_eq!(
            parse_args(&["init".to_string()]).expect("init should parse"),
            CliAction::Init
        );
        assert_eq!(
            parse_args(&["agents".to_string()]).expect("agents should parse"),
            CliAction::Agents { args: None }
        );
        assert_eq!(
            parse_args(&["skills".to_string()]).expect("skills should parse"),
            CliAction::Skills { args: None }
        );
        assert_eq!(
            parse_args(&["agents".to_string(), "--help".to_string()])
                .expect("agents help should parse"),
            CliAction::SubcommandHelp {
                name: "agents",
                summary: "List configured agents. Pass an optional query to filter.",
                usage: "codineer agents [query]",
            }
        );
    }

    #[test]
    fn parses_config_subcommands() {
        assert_eq!(
            parse_args(&[
                "config".to_string(),
                "set".to_string(),
                "defaultModel".to_string(),
                "opus".to_string()
            ])
            .expect("config set should parse"),
            CliAction::ConfigSet {
                key: "defaultModel".to_string(),
                value: "opus".to_string(),
            }
        );
        assert_eq!(
            parse_args(&[
                "config".to_string(),
                "get".to_string(),
                "defaultModel".to_string()
            ])
            .expect("config get should parse"),
            CliAction::ConfigGet {
                key: Some("defaultModel".to_string()),
            }
        );
        assert_eq!(
            parse_args(&["config".to_string(), "get".to_string()])
                .expect("config get (no key) should parse"),
            CliAction::ConfigGet { key: None }
        );
        assert_eq!(
            parse_args(&["config".to_string(), "list".to_string()])
                .expect("config list should parse"),
            CliAction::ConfigList
        );
        assert_eq!(
            parse_args(&["config".to_string()]).expect("config (bare) should default to list"),
            CliAction::ConfigList
        );
    }

    #[test]
    fn help_output_includes_config_section() {
        let mut output = Vec::new();
        print_help_to(&mut output).expect("help should write");
        let text = String::from_utf8(output).expect("valid utf-8");
        assert!(text.contains("codineer config set"));
        assert!(text.contains("codineer config get"));
        assert!(text.contains("codineer config list"));
    }

    #[test]
    fn help_output_includes_models_command() {
        let mut output = Vec::new();
        print_help_to(&mut output).expect("help should write");
        let text = String::from_utf8(output).expect("valid utf-8");
        assert!(text.contains("codineer models"));
        assert!(text.contains("fallbackModels"));
    }

    #[test]
    fn parses_models_subcommand() {
        assert_eq!(
            parse_args(&["models".into()]).unwrap(),
            CliAction::Models { provider: None }
        );
        assert_eq!(
            parse_args(&["models".into(), "anthropic".into()]).unwrap(),
            CliAction::Models {
                provider: Some("anthropic".into())
            }
        );
    }

    #[test]
    fn parses_direct_agents_and_skills_slash_commands() {
        assert_eq!(
            parse_args(&["/agents".to_string()]).expect("/agents should parse"),
            CliAction::Agents { args: None }
        );
        assert_eq!(
            parse_args(&["/skills".to_string()]).expect("/skills should parse"),
            CliAction::Skills { args: None }
        );
        assert_eq!(
            parse_args(&["/skills".to_string(), "help".to_string()])
                .expect("/skills help should parse"),
            CliAction::Skills {
                args: Some("help".to_string())
            }
        );
        let error = parse_args(&["/status".to_string()])
            .expect_err("/status should remain REPL-only when invoked directly");
        assert!(error.contains("Direct slash command unavailable"));
        assert!(error.contains("/status"));
    }

    #[test]
    fn parses_resume_flag_with_slash_command() {
        let args = vec![
            "--resume".to_string(),
            "session.json".to_string(),
            "/compact".to_string(),
        ];
        assert_eq!(
            parse_args(&args).expect("args should parse"),
            CliAction::ResumeSession {
                session_path: PathBuf::from("session.json"),
                commands: vec!["/compact".to_string()],
            }
        );
    }

    #[test]
    fn parses_resume_flag_with_multiple_slash_commands() {
        let args = vec![
            "--resume".to_string(),
            "session.json".to_string(),
            "/status".to_string(),
            "/compact".to_string(),
            "/cost".to_string(),
        ];
        assert_eq!(
            parse_args(&args).expect("args should parse"),
            CliAction::ResumeSession {
                session_path: PathBuf::from("session.json"),
                commands: vec![
                    "/status".to_string(),
                    "/compact".to_string(),
                    "/cost".to_string(),
                ],
            }
        );
    }

    #[test]
    fn filtered_tool_specs_respect_allowlist() {
        let allowed = ["read_file", "grep_search"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let filtered = filter_tool_specs(&GlobalToolRegistry::builtin(), Some(&allowed));
        let names = filtered
            .into_iter()
            .map(|spec| spec.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["read_file", "grep_search"]);
    }

    #[test]
    fn filtered_tool_specs_include_plugin_tools() {
        let filtered = filter_tool_specs(&registry_with_plugin_tool(), None);
        let names = filtered
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();
        assert!(names.contains(&"bash".to_string()));
        assert!(names.contains(&"plugin_echo".to_string()));
    }

    #[test]
    fn permission_policy_uses_plugin_tool_permissions() {
        let policy = permission_policy(PermissionMode::ReadOnly, &registry_with_plugin_tool());
        let required = policy.required_mode_for("plugin_echo");
        assert_eq!(required, PermissionMode::WorkspaceWrite);
    }

    #[test]
    fn shared_help_uses_resume_annotation_copy() {
        let help = commands::render_slash_command_help();
        assert!(help.contains("Slash commands"));
        assert!(help.contains("Tab completes commands inside the REPL."));
        assert!(help.contains("available via codineer --resume SESSION.json"));
    }

    #[test]
    fn repl_help_includes_shared_commands_and_exit() {
        let help = render_repl_help();
        assert!(help.contains("Interactive REPL"));
        assert!(help.contains("/help"));
        assert!(help.contains("/status"));
        assert!(help.contains("/model [model]"));
        assert!(help.contains("/permissions [read-only|workspace-write|danger-full-access]"));
        assert!(help.contains("/clear [--confirm]"));
        assert!(help.contains("/cost"));
        assert!(help.contains("/resume <session-path>"));
        assert!(help.contains("/config [env|hooks|model|plugins]"));
        assert!(help.contains("/memory"));
        assert!(help.contains("/init"));
        assert!(help.contains("/diff"));
        assert!(help.contains("/version"));
        assert!(help.contains("/export [file]"));
        assert!(help.contains("/session [list|switch <session-id>]"));
        assert!(help.contains(
            "/plugin [list|install <path>|enable <name>|disable <name>|uninstall <id>|update <id>]"
        ));
        assert!(help.contains("aliases: /plugins, /marketplace"));
        assert!(help.contains("/agents"));
        assert!(help.contains("/skills"));
        assert!(help.contains("/exit"));
        assert!(help.contains("Tab cycles slash command matches"));
    }

    #[test]
    fn completion_candidates_include_repl_only_exit_commands() {
        let candidates = slash_command_completion_candidates();
        assert!(candidates.contains(&"/help".to_string()));
        assert!(candidates.contains(&"/vim".to_string()));
        assert!(candidates.contains(&"/exit".to_string()));
        assert!(candidates.contains(&"/quit".to_string()));
    }

    #[test]
    fn unknown_repl_command_suggestions_include_repl_shortcuts() {
        let rendered = render_unknown_repl_command("exi");
        assert!(rendered.contains("Unknown slash command"));
        assert!(rendered.contains("/exit"));
        assert!(rendered.contains("/help"));
    }

    #[test]
    fn resume_supported_command_list_matches_expected_surface() {
        let names = resume_supported_slash_commands()
            .into_iter()
            .map(|spec| spec.name)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "help", "status", "compact", "clear", "cost", "config", "memory", "init", "diff",
                "version", "export", "agents", "skills",
            ]
        );
    }

    #[test]
    fn resume_report_uses_sectioned_layout() {
        let report = format_resume_report("session.json", 14, 6);
        assert!(report.contains("Session resumed"));
        assert!(report.contains("Session file     session.json"));
        assert!(report.contains("History          14 messages · 6 turns"));
        assert!(report.contains("/status · /diff · /export"));
    }

    #[test]
    fn compact_report_uses_structured_output() {
        let compacted = format_compact_report(8, 5, false);
        assert!(compacted.contains("Compact"));
        assert!(compacted.contains("Result           compacted"));
        assert!(compacted.contains("Messages removed 8"));
        assert!(compacted.contains("Use /status"));
        let skipped = format_compact_report(0, 3, true);
        assert!(skipped.contains("Result           skipped"));
    }

    #[test]
    fn cost_report_uses_sectioned_layout() {
        let report = format_cost_report(runtime::TokenUsage {
            input_tokens: 20,
            output_tokens: 8,
            cache_creation_input_tokens: 3,
            cache_read_input_tokens: 1,
        });
        assert!(report.contains("Cost"));
        assert!(report.contains("Input tokens     20"));
        assert!(report.contains("Output tokens    8"));
        assert!(report.contains("Cache create     3"));
        assert!(report.contains("Cache read       1"));
        assert!(report.contains("Total tokens     32"));
        assert!(report.contains("/compact"));
    }

    #[test]
    fn permissions_report_uses_sectioned_layout() {
        let report = format_permissions_report("workspace-write");
        assert!(report.contains("Permissions"));
        assert!(report.contains("Active mode      workspace-write"));
        assert!(report.contains("Effect           Editing tools can modify files in the workspace"));
        assert!(report.contains("Modes"));
        assert!(report.contains("read-only          ○ available Read/search tools only"));
        assert!(report.contains("workspace-write    ● current   Edit files inside the workspace"));
        assert!(report.contains("danger-full-access ○ available Unrestricted tool access"));
    }

    #[test]
    fn permissions_switch_report_is_structured() {
        let report = format_permissions_switch_report("read-only", "workspace-write");
        assert!(report.contains("Permissions updated"));
        assert!(report.contains("Previous mode    read-only"));
        assert!(report.contains("Active mode      workspace-write"));
        assert!(report.contains("Applies to       Subsequent tool calls in this REPL"));
    }

    #[test]
    fn init_help_mentions_direct_subcommand() {
        let mut help = Vec::new();
        print_help_to(&mut help).expect("help should render");
        let help = String::from_utf8(help).expect("help should be utf8");
        assert!(help.contains("codineer init"));
        assert!(help.contains("codineer agents"));
        assert!(help.contains("codineer skills"));
        assert!(help.contains("codineer /skills"));
    }

    #[test]
    fn model_report_uses_sectioned_layout() {
        let report = format_model_report("sonnet", 12, 4);
        assert!(report.contains("Model"));
        assert!(report.contains("Current          sonnet"));
        assert!(report.contains("Session          12 messages · 4 turns"));
        assert!(report.contains("Aliases"));
        assert!(report.contains("/model <name>    Switch models for this REPL session"));
    }

    #[test]
    fn model_switch_report_preserves_context_summary() {
        let report = format_model_switch_report("sonnet", "opus", 9);
        assert!(report.contains("Model updated"));
        assert!(report.contains("Previous         sonnet"));
        assert!(report.contains("Current          opus"));
        assert!(report.contains("Preserved        9 messages"));
    }

    #[test]
    fn status_line_reports_model_and_token_totals() {
        let status = format_status_report(
            "sonnet",
            StatusUsage {
                message_count: 7,
                turns: 3,
                latest: runtime::TokenUsage {
                    input_tokens: 5,
                    output_tokens: 4,
                    cache_creation_input_tokens: 1,
                    cache_read_input_tokens: 0,
                },
                cumulative: runtime::TokenUsage {
                    input_tokens: 20,
                    output_tokens: 8,
                    cache_creation_input_tokens: 2,
                    cache_read_input_tokens: 1,
                },
                estimated_tokens: 128,
            },
            "workspace-write",
            &StatusContext {
                cwd: PathBuf::from("/tmp/project"),
                session_path: Some(PathBuf::from("session.json")),
                loaded_config_files: 2,
                discovered_config_files: 3,
                memory_file_count: 4,
                project_root: Some(PathBuf::from("/tmp")),
                git_branch: Some("main".to_string()),
                remote_session: None,
            },
        );
        assert!(status.contains("Session"));
        assert!(status.contains("Model            sonnet"));
        assert!(status.contains("Permissions      workspace-write"));
        assert!(status.contains("Activity         7 messages · 3 turns"));
        assert!(status.contains("Tokens           est 128 · latest 10 · total 31"));
        assert!(status.contains("Folder           /tmp/project"));
        assert!(status.contains("Project root     /tmp"));
        assert!(status.contains("Git branch       main"));
        assert!(status.contains("Session file     session.json"));
        assert!(status.contains("Config files     loaded 2/3"));
        assert!(status.contains("Memory files     4"));
        assert!(status.contains("/session list"));
    }

    #[test]
    fn config_report_supports_section_views() {
        let report = render_config_report(Some("env")).expect("config report should render");
        assert!(report.contains("Merged section: env"));
        let plugins_report =
            render_config_report(Some("plugins")).expect("plugins config report should render");
        assert!(plugins_report.contains("Merged section: plugins"));
    }

    #[test]
    fn memory_report_uses_sectioned_layout() {
        let report = render_memory_report().expect("memory report should render");
        assert!(report.contains("Memory"));
        assert!(report.contains("Working directory"));
        assert!(report.contains("Instruction files"));
        assert!(report.contains("Discovered files"));
    }

    #[test]
    fn config_report_uses_sectioned_layout() {
        let report = render_config_report(None).expect("config report should render");
        assert!(report.contains("Config"));
        assert!(report.contains("Discovered files"));
        assert!(report.contains("Merged JSON"));
    }

    #[test]
    fn parses_git_status_metadata() {
        let (root, branch) = parse_git_status_metadata(Some(
            "## rcc/cli...origin/rcc/cli
 M src/main.rs",
        ));
        assert_eq!(branch.as_deref(), Some("rcc/cli"));
        let _ = root;
    }

    #[test]
    fn status_context_reads_real_workspace_metadata() {
        let context = status_context(None).expect("status context should load");
        assert!(context.cwd.is_absolute());
        assert_eq!(context.discovered_config_files, 5);
        assert!(context.loaded_config_files <= context.discovered_config_files);
    }

    #[test]
    fn normalizes_supported_permission_modes() {
        assert_eq!(normalize_permission_mode("read-only"), Some("read-only"));
        assert_eq!(
            normalize_permission_mode("workspace-write"),
            Some("workspace-write")
        );
        assert_eq!(
            normalize_permission_mode("danger-full-access"),
            Some("danger-full-access")
        );
        assert_eq!(normalize_permission_mode("unknown"), None);
    }

    #[test]
    fn clear_command_requires_explicit_confirmation_flag() {
        assert_eq!(
            SlashCommand::parse("/clear"),
            Some(SlashCommand::Clear { confirm: false })
        );
        assert_eq!(
            SlashCommand::parse("/clear --confirm"),
            Some(SlashCommand::Clear { confirm: true })
        );
    }

    #[test]
    fn parses_resume_and_config_slash_commands() {
        assert_eq!(
            SlashCommand::parse("/resume saved-session.json"),
            Some(SlashCommand::Resume {
                session_path: Some("saved-session.json".to_string())
            })
        );
        assert_eq!(
            SlashCommand::parse("/clear --confirm"),
            Some(SlashCommand::Clear { confirm: true })
        );
        assert_eq!(
            SlashCommand::parse("/config"),
            Some(SlashCommand::Config { section: None })
        );
        assert_eq!(
            SlashCommand::parse("/config env"),
            Some(SlashCommand::Config {
                section: Some("env".to_string())
            })
        );
        assert_eq!(SlashCommand::parse("/memory"), Some(SlashCommand::Memory));
        assert_eq!(SlashCommand::parse("/init"), Some(SlashCommand::Init));
    }

    #[test]
    fn init_template_mentions_detected_rust_workspace() {
        let rendered = crate::init::render_init_codineer_md(std::path::Path::new("."));
        assert!(rendered.contains("# CODINEER.md"));
        assert!(rendered.contains("cargo clippy --workspace --all-targets -- -D warnings"));
    }

    #[test]
    fn converts_tool_roundtrip_messages() {
        let messages = vec![
            ConversationMessage::user_text("hello"),
            ConversationMessage::assistant(vec![ContentBlock::ToolUse {
                id: "tool-1".to_string(),
                name: "bash".to_string(),
                input: "{\"command\":\"pwd\"}".to_string(),
            }]),
            ConversationMessage {
                role: MessageRole::Tool,
                blocks: vec![ContentBlock::ToolResult {
                    tool_use_id: "tool-1".to_string(),
                    tool_name: "bash".to_string(),
                    output: "ok".to_string(),
                    is_error: false,
                }],
                usage: None,
            },
        ];

        let converted = convert_messages(&messages);
        assert_eq!(converted.len(), 3);
        assert_eq!(converted[1].role, "assistant");
        assert_eq!(converted[2].role, "user");
    }

    #[test]
    fn repl_help_mentions_history_completion_and_multiline() {
        let help = render_repl_help();
        assert!(help.contains("Up/Down"));
        assert!(help.contains("Tab cycles"));
        assert!(help.contains("Shift+Enter or Ctrl+J"));
    }

    #[test]
    fn tool_rendering_helpers_compact_output() {
        let start = format_tool_call_start("read_file", r#"{"path":"src/main.rs"}"#);
        assert!(start.contains("read_file"));
        assert!(start.contains("src/main.rs"));

        let done = format_tool_result(
            "read_file",
            r#"{"file":{"filePath":"src/main.rs","content":"hello","numLines":1,"startLine":1,"totalLines":1}}"#,
            false,
        );
        assert!(done.contains("Read src/main.rs"));
        assert!(done.contains("hello"));
    }

    #[test]
    fn tool_rendering_truncates_large_read_output_for_display_only() {
        let content = (0..200)
            .map(|index| format!("line {index:03}"))
            .collect::<Vec<_>>()
            .join("\n");
        let output = json!({
            "file": {
                "filePath": "src/main.rs",
                "content": content,
                "numLines": 200,
                "startLine": 1,
                "totalLines": 200
            }
        })
        .to_string();

        let rendered = format_tool_result("read_file", &output, false);

        assert!(rendered.contains("line 000"));
        assert!(rendered.contains("line 079"));
        assert!(!rendered.contains("line 199"));
        assert!(rendered.contains("full result preserved in session"));
        assert!(output.contains("line 199"));
    }

    #[test]
    fn tool_rendering_truncates_large_bash_output_for_display_only() {
        let stdout = (0..120)
            .map(|index| format!("stdout {index:03}"))
            .collect::<Vec<_>>()
            .join("\n");
        let output = json!({
            "stdout": stdout,
            "stderr": "",
            "returnCodeInterpretation": "completed successfully"
        })
        .to_string();

        let rendered = format_tool_result("bash", &output, false);

        assert!(rendered.contains("stdout 000"));
        assert!(rendered.contains("stdout 059"));
        assert!(!rendered.contains("stdout 119"));
        assert!(rendered.contains("full result preserved in session"));
        assert!(output.contains("stdout 119"));
    }

    #[test]
    fn tool_rendering_truncates_generic_long_output_for_display_only() {
        let items = (0..120)
            .map(|index| format!("payload {index:03}"))
            .collect::<Vec<_>>();
        let output = json!({
            "summary": "plugin payload",
            "items": items,
        })
        .to_string();

        let rendered = format_tool_result("plugin_echo", &output, false);

        assert!(rendered.contains("plugin_echo"));
        assert!(rendered.contains("payload 000"));
        assert!(rendered.contains("payload 040"));
        assert!(!rendered.contains("payload 080"));
        assert!(!rendered.contains("payload 119"));
        assert!(rendered.contains("full result preserved in session"));
        assert!(output.contains("payload 119"));
    }

    #[test]
    fn tool_rendering_truncates_raw_generic_output_for_display_only() {
        let output = (0..120)
            .map(|index| format!("raw {index:03}"))
            .collect::<Vec<_>>()
            .join("\n");

        let rendered = format_tool_result("plugin_echo", &output, false);

        assert!(rendered.contains("plugin_echo"));
        assert!(rendered.contains("raw 000"));
        assert!(rendered.contains("raw 059"));
        assert!(!rendered.contains("raw 119"));
        assert!(rendered.contains("full result preserved in session"));
        assert!(output.contains("raw 119"));
    }

    #[test]
    fn ultraplan_progress_lines_include_phase_step_and_elapsed_status() {
        let snapshot = InternalPromptProgressState {
            command_label: "Ultraplan",
            task_label: "ship plugin progress".to_string(),
            step: 3,
            phase: "running read_file".to_string(),
            detail: Some("reading rust/crates/codineer-cli/src/main.rs".to_string()),
            saw_final_text: false,
        };

        let started = format_internal_prompt_progress_line(
            InternalPromptProgressEvent::Started,
            &snapshot,
            Duration::from_secs(0),
            None,
        );
        let heartbeat = format_internal_prompt_progress_line(
            InternalPromptProgressEvent::Heartbeat,
            &snapshot,
            Duration::from_secs(9),
            None,
        );
        let completed = format_internal_prompt_progress_line(
            InternalPromptProgressEvent::Complete,
            &snapshot,
            Duration::from_secs(12),
            None,
        );
        let failed = format_internal_prompt_progress_line(
            InternalPromptProgressEvent::Failed,
            &snapshot,
            Duration::from_secs(12),
            Some("network timeout"),
        );

        assert!(started.contains("planning started"));
        assert!(started.contains("current step 3"));
        assert!(heartbeat.contains("heartbeat"));
        assert!(heartbeat.contains("9s elapsed"));
        assert!(heartbeat.contains("phase running read_file"));
        assert!(completed.contains("completed"));
        assert!(completed.contains("3 steps total"));
        assert!(failed.contains("failed"));
        assert!(failed.contains("network timeout"));
    }

    #[test]
    fn describe_tool_progress_summarizes_known_tools() {
        assert_eq!(
            describe_tool_progress("read_file", r#"{"path":"src/main.rs"}"#),
            "reading src/main.rs"
        );
        assert!(
            describe_tool_progress("bash", r#"{"command":"cargo test -p codineer-cli"}"#)
                .contains("cargo test -p codineer-cli")
        );
        assert_eq!(
            describe_tool_progress("grep_search", r#"{"pattern":"ultraplan","path":"rust"}"#),
            "grep `ultraplan` in rust"
        );
    }

    #[test]
    fn push_output_block_renders_markdown_text() {
        let mut out = Vec::new();
        let mut events = Vec::new();
        let mut pending_tool = None;

        push_output_block(
            OutputContentBlock::Text {
                text: "# Heading".to_string(),
            },
            &mut out,
            &mut events,
            &mut pending_tool,
            false,
        )
        .expect("text block should render");

        let rendered = String::from_utf8(out).expect("utf8");
        assert!(rendered.contains("Heading"));
        if crate::style::color_for_stdout() {
            assert!(rendered.contains('\u{1b}'));
        }
    }

    #[test]
    fn push_output_block_skips_empty_object_prefix_for_tool_streams() {
        let mut out = Vec::new();
        let mut events = Vec::new();
        let mut pending_tool = None;

        push_output_block(
            OutputContentBlock::ToolUse {
                id: "tool-1".to_string(),
                name: "read_file".to_string(),
                input: json!({}),
            },
            &mut out,
            &mut events,
            &mut pending_tool,
            true,
        )
        .expect("tool block should accumulate");

        assert!(events.is_empty());
        assert_eq!(
            pending_tool,
            Some(("tool-1".to_string(), "read_file".to_string(), String::new(),))
        );
    }

    #[test]
    fn response_to_events_preserves_empty_object_json_input_outside_streaming() {
        let mut out = Vec::new();
        let events = response_to_events(
            MessageResponse {
                id: "msg-1".to_string(),
                kind: "message".to_string(),
                model: "claude-opus-4-6".to_string(),
                role: "assistant".to_string(),
                content: vec![OutputContentBlock::ToolUse {
                    id: "tool-1".to_string(),
                    name: "read_file".to_string(),
                    input: json!({}),
                }],
                stop_reason: Some("tool_use".to_string()),
                stop_sequence: None,
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
                request_id: None,
            },
            &mut out,
        )
        .expect("response conversion should succeed");

        assert!(matches!(
            &events[0],
            AssistantEvent::ToolUse { name, input, .. }
                if name == "read_file" && input == "{}"
        ));
    }

    #[test]
    fn response_to_events_preserves_non_empty_json_input_outside_streaming() {
        let mut out = Vec::new();
        let events = response_to_events(
            MessageResponse {
                id: "msg-2".to_string(),
                kind: "message".to_string(),
                model: "claude-opus-4-6".to_string(),
                role: "assistant".to_string(),
                content: vec![OutputContentBlock::ToolUse {
                    id: "tool-2".to_string(),
                    name: "read_file".to_string(),
                    input: json!({ "path": "rust/Cargo.toml" }),
                }],
                stop_reason: Some("tool_use".to_string()),
                stop_sequence: None,
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
                request_id: None,
            },
            &mut out,
        )
        .expect("response conversion should succeed");

        assert!(matches!(
            &events[0],
            AssistantEvent::ToolUse { name, input, .. }
                if name == "read_file" && input == "{\"path\":\"rust/Cargo.toml\"}"
        ));
    }

    #[test]
    fn response_to_events_ignores_thinking_blocks() {
        let mut out = Vec::new();
        let events = response_to_events(
            MessageResponse {
                id: "msg-3".to_string(),
                kind: "message".to_string(),
                model: "claude-opus-4-6".to_string(),
                role: "assistant".to_string(),
                content: vec![
                    OutputContentBlock::Thinking {
                        thinking: "step 1".to_string(),
                        signature: Some("sig_123".to_string()),
                    },
                    OutputContentBlock::Text {
                        text: "Final answer".to_string(),
                    },
                ],
                stop_reason: Some("end_turn".to_string()),
                stop_sequence: None,
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
                request_id: None,
            },
            &mut out,
        )
        .expect("response conversion should succeed");

        assert!(matches!(
            &events[0],
            AssistantEvent::TextDelta(text) if text == "Final answer"
        ));
        assert!(!String::from_utf8(out).expect("utf8").contains("step 1"));
    }

    #[test]
    fn resolve_export_path_rejects_traversal() {
        use crate::reports::resolve_export_path;
        use runtime::Session;
        let session = Session::default();

        let err = resolve_export_path(Some("../../../etc/passwd"), &session);
        assert!(err.is_err(), "should reject path traversal");

        let err = resolve_export_path(Some("/tmp/evil.txt"), &session);
        assert!(err.is_err(), "should reject absolute path");

        let ok = resolve_export_path(Some("my-export"), &session);
        assert!(ok.is_ok(), "simple name should succeed");
        assert!(ok
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("my-export.txt"));
    }

    // -----------------------------------------------------------------------
    // parse_auth_args validation
    // -----------------------------------------------------------------------

    #[test]
    fn login_rejects_unknown_flag() {
        let err = parse_args(&["login".into(), "--unknown".into()])
            .expect_err("should reject unknown flag");
        assert!(err.contains("unknown flag: --unknown"));
    }

    #[test]
    fn login_rejects_extra_positional_arg() {
        let err = parse_args(&["login".into(), "anthropic".into(), "extra".into()])
            .expect_err("should reject extra arg");
        assert!(err.contains("unexpected argument: extra"));
    }

    #[test]
    fn logout_rejects_unknown_flag() {
        let err = parse_args(&["logout".into(), "--bad-flag".into()])
            .expect_err("should reject unknown flag");
        assert!(err.contains("unknown flag: --bad-flag"));
    }

    #[test]
    fn login_help_flag_returns_subcommand_help() {
        let action = parse_args(&["login".into(), "--help".into()]).unwrap();
        assert!(matches!(
            action,
            CliAction::SubcommandHelp { name: "login", .. }
        ));
    }

    #[test]
    fn logout_help_flag_returns_subcommand_help() {
        let action = parse_args(&["logout".into(), "-h".into()]).unwrap();
        assert!(matches!(
            action,
            CliAction::SubcommandHelp { name: "logout", .. }
        ));
    }

    // -----------------------------------------------------------------------
    // subcommand_names
    // -----------------------------------------------------------------------

    #[test]
    fn subcommand_names_includes_all_commands() {
        use crate::cli::subcommand_names;
        let names = subcommand_names();
        assert!(names.contains(&"login".to_string()));
        assert!(names.contains(&"logout".to_string()));
        assert!(names.contains(&"status".to_string()));
        assert!(names.contains(&"models".to_string()));
        assert!(names.contains(&"config".to_string()));
        assert!(names.contains(&"init".to_string()));
        assert!(names.contains(&"help".to_string()));
        assert!(names.contains(&"agents".to_string()));
        assert!(names.contains(&"skills".to_string()));
    }

    // -----------------------------------------------------------------------
    // suggest_subcommand
    // -----------------------------------------------------------------------

    #[test]
    fn suggest_subcommand_finds_typos() {
        use crate::help::suggest_subcommand;
        assert_eq!(suggest_subcommand("logi"), Some("login".to_string()));
        assert_eq!(suggest_subcommand("logut"), Some("logout".to_string()));
        assert_eq!(suggest_subcommand("statu"), Some("status".to_string()));
        assert_eq!(suggest_subcommand("modles"), Some("models".to_string()));
        assert_eq!(suggest_subcommand("confg"), Some("config".to_string()));
    }

    #[test]
    fn suggest_subcommand_returns_none_for_distant_input() {
        use crate::help::suggest_subcommand;
        assert_eq!(suggest_subcommand("xyzzy"), None);
        assert_eq!(suggest_subcommand("abcdefgh"), None);
    }

    #[test]
    fn suggest_subcommand_exact_match() {
        use crate::help::suggest_subcommand;
        assert_eq!(suggest_subcommand("login"), Some("login".to_string()));
        assert_eq!(suggest_subcommand("models"), Some("models".to_string()));
    }

    // -----------------------------------------------------------------------
    // config subcommand edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn config_set_missing_value_returns_error() {
        let err = parse_args(&["config".into(), "set".into(), "key".into()])
            .expect_err("should need value");
        assert!(err.contains("usage"));
    }

    #[test]
    fn config_set_missing_key_returns_error() {
        let err = parse_args(&["config".into(), "set".into()]).expect_err("should need key");
        assert!(err.contains("usage"));
    }

    #[test]
    fn config_unknown_subcommand_returns_error() {
        let err = parse_args(&["config".into(), "unknown".into()])
            .expect_err("should reject unknown subcommand");
        assert!(err.contains("unknown config subcommand"));
    }

    // -----------------------------------------------------------------------
    // models subcommand with help flag
    // -----------------------------------------------------------------------

    #[test]
    fn models_help_returns_subcommand_help() {
        let action = parse_args(&["models".into(), "--help".into()]).unwrap();
        assert!(matches!(
            action,
            CliAction::SubcommandHelp { name: "models", .. }
        ));
    }

    // -----------------------------------------------------------------------
    // help output comprehensive checks
    // -----------------------------------------------------------------------

    #[test]
    fn help_output_includes_auth_sources() {
        let mut output = Vec::new();
        print_help_to(&mut output).expect("help should write");
        let text = String::from_utf8(output).expect("valid utf-8");
        assert!(text.contains("Authentication sources"));
        assert!(text.contains("Claude Code auto-discover"));
        assert!(text.contains("OLLAMA_HOST"));
    }

    #[test]
    fn help_output_includes_provider_section() {
        let mut output = Vec::new();
        print_help_to(&mut output).expect("help should write");
        let text = String::from_utf8(output).expect("valid utf-8");
        assert!(text.contains("Custom providers (OpenAI-compatible)"));
        assert!(text.contains("ollama"));
        assert!(text.contains("lmstudio"));
        assert!(text.contains("openrouter"));
        assert!(text.contains("groq"));
    }

    #[test]
    fn help_output_includes_config_files_section() {
        let mut output = Vec::new();
        print_help_to(&mut output).expect("help should write");
        let text = String::from_utf8(output).expect("valid utf-8");
        assert!(text.contains("Configuration files"));
        assert!(text.contains("settings.local.json"));
        assert!(text.contains("settings.json"));
        assert!(text.contains("CODINEER.md"));
    }
}
