mod auth;
mod auto_update;
mod banner;
mod bootstrap;
pub mod cli;
mod config_cmd;
pub mod error;
mod help;
mod image_util;
mod init;
mod input;
mod live_cli;
mod lsp_detect;
mod models_cmd;
mod platform;
mod progress;
mod render;
mod reports;
mod resume;
mod runtime_client;
mod session_store;
pub mod style;
mod terminal_width;
mod tool_display;
pub mod tracing_observer;
mod turn_helpers;
mod workspace;

use std::env;
use std::path::PathBuf;

use crate::error::CliResult;

use init::initialize_repo;

use auth::{run_login, run_logout, run_status};
use config_cmd::{run_config_get, run_config_list, run_config_set};

use cli::{parse_args, CliAction};
use help::print_help;
use live_cli::{run_repl, LiveCli};
use reports::render_version_report;
use resume::resume_session;

pub(crate) use bootstrap::{
    build_plugin_manager, build_runtime_plugin_state, build_system_prompt,
    build_system_prompt_with_lsp,
};

pub fn default_model() -> String {
    aineer_api::auto_detect_default_model()
        .unwrap_or("auto")
        .to_string()
}

pub fn max_tokens_for_model(model: &str) -> u32 {
    aineer_api::max_tokens_for_model(model)
}

pub fn current_date() -> String {
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

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD_TARGET: Option<&str> = option_env!("TARGET");
pub const GIT_SHA: Option<&str> = option_env!("GIT_SHA");

pub fn logo_ascii(color: bool) -> String {
    let p = style::Palette::new(color);
    if color {
        [
            format!("{}          ▄██▄{}", p.violet, p.r),
            format!("{}       ▄██▀  ▀██▄{}", p.violet, p.r),
            format!(
                "{}      ██  {}❯{}     ██{}     {}A I N E E R{}",
                p.violet, p.cyan_fg, p.violet, p.r, p.bold_white, p.r,
            ),
            format!(
                "{}      ██     {}▍{}  ██{}     {}Your local AI engineer{}",
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
            "      ██  ❯     ██     A I N E E R",
            "      ██     ▍  ██     Your local AI engineer",
            "       ▀██▄  ▄██▀",
            "          ▀██▀",
        ]
        .join("\n")
    }
}

pub fn render_cli_error(problem: &str) -> String {
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
    let _ = writeln!(out, "\n  {}aineer --help{}", p.dim, p.r);
    out
}

fn highlight_cli_hint(p: &style::Palette, line: &str) -> String {
    if let Some(idx) = line.find("export ") {
        let (prefix, cmd) = line.split_at(idx);
        format!("{}{}{}{}", prefix, p.cyan_fg, cmd, p.r)
    } else if line.trim_start().starts_with("aineer ") {
        format!("{}{}{}", p.cyan_fg, line, p.r)
    } else {
        line.to_string()
    }
}

/// Entry point for CLI mode. Call this from the unified `aineer` binary
/// when `--cli` is passed, or when running in a non-GUI environment.
pub fn run_cli() -> i32 {
    init_tracing();
    match run() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{}", render_cli_error(&error.to_string()));
            1
        }
    }
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_env("AINEER_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

fn run() -> CliResult<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    // Strip "--cli" if present (used by unified binary)
    let args: Vec<String> = args.into_iter().filter(|a| a != "--cli").collect();
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
        CliAction::Providers => models_cmd::run_providers()?,
        CliAction::ConfigSet { key, value } => run_config_set(&key, &value)?,
        CliAction::ConfigGet { key } => run_config_get(key.as_deref())?,
        CliAction::ConfigList => run_config_list()?,
        CliAction::Init => run_init()?,
        CliAction::Update => auto_update::run_update_command(),
        CliAction::Repl {
            model,
            allowed_tools,
            permission_mode,
            resume_path,
        } => run_repl(model, allowed_tools, permission_mode, resume_path)?,
        CliAction::Help => print_help(),
        CliAction::SubcommandHelp {
            name,
            summary,
            usage,
        } => {
            println!("aineer {name}\n  {summary}\n\nUsage:\n  {usage}");
        }
    }
    Ok(())
}

fn print_system_prompt(cwd: PathBuf, date: String) -> CliResult<()> {
    let blocks =
        aineer_engine::load_system_prompt_with_lsp(cwd, date, env::consts::OS, "unknown", None)?;
    let text: Vec<&str> = blocks.iter().map(|b| b.text.as_str()).collect();
    println!("{}", text.join("\n\n"));
    Ok(())
}

fn print_version() {
    println!("{}", render_version_report());
}

pub fn init_aineer_md() -> CliResult<String> {
    let cwd = env::current_dir()?;
    Ok(initialize_repo(&cwd)?.render())
}

fn run_init() -> CliResult<()> {
    println!("{}", init_aineer_md()?);
    Ok(())
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
