use std::io::{self, IsTerminal};

use commands::{
    render_slash_command_help, resume_supported_slash_commands, slash_command_specs,
    suggest_slash_commands,
};

use crate::{logo_ascii, VERSION};

pub(crate) fn print_help_section(
    out: &mut impl io::Write,
    title: &str,
    entries: &[&str],
) -> io::Result<()> {
    writeln!(out, "{title}")?;
    for entry in entries {
        writeln!(out, "{entry}")?;
    }
    writeln!(out)
}

pub(crate) fn print_help_to(out: &mut impl io::Write) -> io::Result<()> {
    let color = io::stdout().is_terminal();
    writeln!(out, "{}", logo_ascii(color))?;
    writeln!(out, "  v{VERSION}")?;
    writeln!(out)?;
    print_help_section(
        out,
        "Quick start",
        &[
            "  codineer                                  Start the interactive REPL",
            "  codineer \"summarize this repo\"            Run one prompt and exit",
            "  codineer prompt \"explain src/main.rs\"     Explicit one-shot prompt",
            "  codineer --resume SESSION.json /status    Inspect a saved session",
        ],
    )?;
    print_help_section(
        out,
        "Interactive essentials",
        &[
            "  /help                                 Browse the full slash command map",
            "  /status                               Inspect session + workspace state",
            "  /model <name>                         Switch models mid-session",
            "  /permissions <mode>                   Adjust tool access",
            "  Tab                                   Complete slash commands",
            "  /vim                                  Toggle modal editing",
            "  Shift+Enter / Ctrl+J                  Insert a newline",
        ],
    )?;
    print_help_section(
        out,
        "Commands",
        &[
            "  codineer help                             Show this help message",
            "  codineer agents                           List configured agents",
            "  codineer skills                           List installed skills",
            "  codineer system-prompt [--cwd PATH] [--date YYYY-MM-DD]",
            "  codineer login                            Start the OAuth login flow",
            "  codineer logout                           Clear saved OAuth credentials",
            "  codineer init                             Scaffold CODINEER.md + local files",
        ],
    )?;
    print_help_section(
        out,
        "Flags",
        &[
            "  --model MODEL                         Override the active model",
            "  --output-format FORMAT                Non-interactive output: text or json",
            "  --permission-mode MODE                Set read-only, workspace-write, or danger-full-access",
            "  --dangerously-skip-permissions        Skip all permission checks",
            "  --allowedTools TOOLS                  Restrict enabled tools (repeatable; comma-separated aliases supported)",
            "  --version, -V                         Print version and build information",
        ],
    )?;
    print_help_section(
        out,
        "Environment variables",
        &[
            "  ANTHROPIC_API_KEY                     API key for Anthropic (Claude) models",
            "  ANTHROPIC_AUTH_TOKEN                  Bearer token (alternative to API key)",
            "  XAI_API_KEY                           API key for xAI (Grok) models",
            "  OPENAI_API_KEY                        API key for OpenAI-compatible models",
            "  CODINEER_WORKSPACE_ROOT               Override workspace root directory",
            "  CODINEER_CONFIG_HOME                  Override config directory (default: ~/.codineer)",
            "  NO_COLOR                              Disable colored output (no-color.org)",
            "  CLICOLOR=0                            Disable colored output (alternative)",
        ],
    )?;
    print_help_section(
        out,
        "Configuration files (merged in order)",
        &[
            "  ~/.codineer/settings.json             Global settings",
            "  .codineer.json                        Project-local settings",
            "  CODINEER.md                           Project context and instructions",
        ],
    )?;
    print_help_slash_reference(out)?;
    print_help_section(
        out,
        "Examples",
        &[
            "  codineer --model opus \"summarize this repo\"",
            "  codineer --output-format json prompt \"explain src/main.rs\"",
            "  codineer --allowedTools read,glob \"summarize Cargo.toml\"",
            "  codineer --resume session.json /status /diff /export notes.txt",
            "  codineer agents",
            "  codineer /skills",
            "  codineer login",
            "  codineer init",
        ],
    )
}

pub(crate) fn print_help_slash_reference(out: &mut impl io::Write) -> io::Result<()> {
    writeln!(out, "Slash command reference")?;
    writeln!(out, "{}", render_slash_command_help())?;
    writeln!(out)?;
    let resume_commands = resume_supported_slash_commands()
        .into_iter()
        .map(|spec| match spec.argument_hint {
            Some(hint) => format!("/{} {hint}", spec.name),
            None => format!("/{}", spec.name),
        })
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(out, "Resume-safe commands: {resume_commands}")
}

pub(crate) fn print_help() {
    let _ = print_help_to(&mut io::stdout());
}

pub(crate) fn render_repl_help() -> String {
    [
        "Interactive REPL".to_string(),
        "  Quick start          Ask a task in plain English or use one of the core commands below."
            .to_string(),
        "  Core commands        /help · /status · /model · /permissions · /compact".to_string(),
        "  Exit                 /exit or /quit".to_string(),
        "  Vim mode             /vim toggles modal editing".to_string(),
        "  History              Up/Down recalls previous prompts".to_string(),
        "  Completion           Tab cycles slash command matches".to_string(),
        "  Cancel               Ctrl-C clears input (or exits on an empty prompt)".to_string(),
        "  Multiline            Shift+Enter or Ctrl+J inserts a newline".to_string(),
        String::new(),
        render_slash_command_help(),
    ]
    .join(
        "
",
    )
}

pub(crate) fn append_slash_command_suggestions(lines: &mut Vec<String>, name: &str) {
    let suggestions = suggest_slash_commands(name, 3);
    if suggestions.is_empty() {
        lines.push("  Try              /help shows the full slash command map".to_string());
        return;
    }

    lines.push("  Try              /help shows the full slash command map".to_string());
    lines.push("Suggestions".to_string());
    lines.extend(
        suggestions
            .into_iter()
            .map(|suggestion| format!("  {suggestion}")),
    );
}

pub(crate) fn render_unknown_repl_command(name: &str) -> String {
    let mut lines = vec![
        "Unknown slash command".to_string(),
        format!("  Command          /{name}"),
    ];
    append_repl_command_suggestions(&mut lines, name);
    lines.join("\n")
}

pub(crate) fn append_repl_command_suggestions(lines: &mut Vec<String>, name: &str) {
    let suggestions = suggest_repl_commands(name);
    if suggestions.is_empty() {
        lines.push("  Try              /help shows the full slash command map".to_string());
        return;
    }

    lines.push("  Try              /help shows the full slash command map".to_string());
    lines.push("Suggestions".to_string());
    lines.extend(
        suggestions
            .into_iter()
            .map(|suggestion| format!("  {suggestion}")),
    );
}

pub(crate) fn render_mode_unavailable(command: &str, label: &str) -> String {
    [
        "Command unavailable in this REPL mode".to_string(),
        format!("  Command          /{command}"),
        format!("  Feature          {label}"),
        "  Tip              Use /help to find currently wired REPL commands".to_string(),
    ]
    .join("\n")
}

pub(crate) fn slash_command_completion_candidates() -> Vec<String> {
    let mut candidates = slash_command_specs()
        .iter()
        .flat_map(|spec| {
            std::iter::once(spec.name)
                .chain(spec.aliases.iter().copied())
                .map(|name| format!("/{name}"))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    candidates.extend([
        String::from("/vim"),
        String::from("/exit"),
        String::from("/quit"),
    ]);
    candidates.sort();
    candidates.dedup();
    candidates
}

fn suggest_repl_commands(name: &str) -> Vec<String> {
    let normalized = name.trim().trim_start_matches('/').to_ascii_lowercase();
    if normalized.is_empty() {
        return Vec::new();
    }

    let mut ranked = slash_command_completion_candidates()
        .into_iter()
        .filter_map(|candidate| {
            let raw = candidate.trim_start_matches('/').to_ascii_lowercase();
            let distance = edit_distance(&normalized, &raw);
            let prefix_match = raw.starts_with(&normalized) || normalized.starts_with(&raw);
            let near_match = distance <= 2;
            (prefix_match || near_match).then_some((distance, candidate))
        })
        .collect::<Vec<_>>();
    ranked.sort();
    ranked.dedup_by(|left, right| left.1 == right.1);
    ranked
        .into_iter()
        .map(|(_, candidate)| candidate)
        .take(3)
        .collect()
}

fn edit_distance(left: &str, right: &str) -> usize {
    if left == right {
        return 0;
    }
    if left.is_empty() {
        return right.chars().count();
    }
    if right.is_empty() {
        return left.chars().count();
    }

    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_index, left_char) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != *right_char);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution_cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous[right_chars.len()]
}
