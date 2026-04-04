use std::io;

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
    let color = crate::style::color_for_stdout();
    writeln!(out, "{}", logo_ascii(color))?;
    writeln!(out, "  v{VERSION}")?;
    writeln!(
        out,
        "  Multi-provider AI coding agent — any model, zero lock-in."
    )?;
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
            "  codineer login [<provider>] [--source <id>] Start the login flow",
            "  codineer logout [<provider>] [--source <id>] Clear saved credentials",
            "  codineer status [<provider>]              Show authentication status",
            "  codineer models [<provider>]              List available models",
            "  codineer config set <key> <value>         Set a configuration value",
            "  codineer config get [<key>]               Show a configuration value",
            "  codineer config list                      List all configuration",
            "  codineer init                             Scaffold CODINEER.md + local files",
        ],
    )?;
    print_help_section(
        out,
        "Flags",
        &[
            "  -p TEXT                               Run a one-shot prompt (rest of line is the prompt)",
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
            "  ANTHROPIC_AUTH_TOKEN                   Bearer token (alternative to API key)",
            "  XAI_API_KEY                           API key for xAI (Grok) models",
            "  OPENAI_API_KEY                        API key for OpenAI-compatible models",
            "  OPENROUTER_API_KEY                    API key for OpenRouter (free models available)",
            "  GROQ_API_KEY                          API key for Groq Cloud (free tier available)",
            "  OLLAMA_HOST                           Ollama endpoint (e.g. http://192.168.1.100:11434)",
            "  CODINEER_WORKSPACE_ROOT               Override workspace root directory",
            "  CODINEER_CONFIG_HOME                  Override config directory (default: ~/.codineer)",
            "  CODINEER_PERMISSION_MODE              Default permission mode",
            "  NO_COLOR                              Disable colored output (no-color.org)",
            "  CLICOLOR=0                            Disable colored output (alternative)",
        ],
    )?;
    print_help_section(
        out,
        "Authentication sources (per-provider credential chain)",
        &[
            "  Anthropic (Claude):  env vars → Codineer OAuth → Claude Code auto-discover",
            "  xAI (Grok):          XAI_API_KEY env var",
            "  OpenAI:              OPENAI_API_KEY env var",
            "  Custom providers:    inline apiKey → apiKeyEnv",
            "",
            "  Claude Code auto-discovery: install Claude Code and `claude login`.",
            "  Codineer auto-detects saved Claude Code credentials (~/.claude/.credentials.json).",
            "  Configure in settings.json:  {\"credentials\": {\"claudeCode\": {\"enabled\": true}}}",
        ],
    )?;
    print_help_section(
        out,
        "Configuration files (highest to lowest precedence)",
        &[
            "  .codineer/settings.local.json         Local overrides (gitignored)",
            "  .codineer/settings.json               Project settings",
            "  .codineer.json                        Project flat config",
            "  ~/.codineer/settings.json             Global settings",
            "  ~/.codineer.json                      Global flat config",
            "  CODINEER.md                           Project context and instructions",
            "",
            "  Supported keys: model, fallbackModels, env, hooks, enabledPlugins, plugins, mcpServers, permissionMode, providers, credentials",
            "  Example: {\"model\": \"sonnet\", \"fallbackModels\": [\"ollama/qwen3-coder\"]}",
            "  Tip: use `codineer config set <key> <value>` instead of editing JSON manually.",
        ],
    )?;
    print_help_section(
        out,
        "Custom providers (OpenAI-compatible)",
        &[
            "  Built-in:  ollama, lmstudio, openrouter, groq",
            "  Usage:     codineer --model ollama/qwen3-coder",
            "             codineer --model groq/llama-3.3-70b-versatile",
            "  Auto:      codineer --model ollama   (picks best coding model)",
            "  Zero-config Ollama: if no API keys are found and Ollama is running,",
            "             Codineer auto-detects it and picks a coding model.",
            "",
            "  Ollama host resolution (highest priority first):",
            "    1. settings.json: {\"providers\": {\"ollama\": {\"baseUrl\": \"http://host:11434/v1\"}}}",
            "    2. Environment:   export OLLAMA_HOST=http://192.168.1.100:11434",
            "    3. Default:       http://localhost:11434",
            "",
            "  Configure custom providers in settings.json:",
            "    {\"providers\": {\"my-api\": {\"baseUrl\": \"https://...\", \"apiKeyEnv\": \"MY_KEY\"}}}",
            "",
            "  Models without function calling automatically fall back to text-only mode.",
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
            "  codineer login anthropic --source claude-code",
            "  codineer status",
            "  codineer models anthropic",
            "  codineer config set model sonnet",
            "  codineer config set fallbackModels '[\"ollama/qwen3-coder\"]'",
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
    const HEADER: &[&str] = &[
        "Interactive REPL",
        "  Quick start          Ask a task in plain English or use one of the core commands below.",
        "  Core commands        /help · /status · /model · /permissions · /compact",
        "  Exit                 /exit or /quit",
        "  Vim mode             /vim toggles modal editing",
        "  History              Up/Down recalls previous prompts",
        "  Completion           Tab cycles slash command matches",
        "  Cancel               Ctrl-C clears input (or exits on an empty prompt)",
        "  Multiline            Shift+Enter or Ctrl+J inserts a newline",
    ];
    let mut parts: Vec<&str> = HEADER.to_vec();
    parts.push("");
    let commands = render_slash_command_help();
    parts.push(&commands);
    parts.join("\n")
}

fn append_suggestions(lines: &mut Vec<String>, suggestions: Vec<String>) {
    lines.push("  Try              /help shows the full slash command map".to_string());
    if !suggestions.is_empty() {
        let p = crate::style::Palette::for_stdout();
        lines.push(p.title("Suggestions"));
        lines.extend(
            suggestions
                .into_iter()
                .map(|suggestion| format!("  {suggestion}")),
        );
    }
}

pub(crate) fn append_slash_command_suggestions(lines: &mut Vec<String>, name: &str) {
    append_suggestions(lines, suggest_slash_commands(name, 3));
}

pub(crate) fn render_unknown_repl_command(name: &str) -> String {
    let mut lines = vec![
        "Unknown slash command".to_string(),
        format!("  Command          /{name}"),
    ];
    append_suggestions(&mut lines, suggest_repl_commands(name));
    lines.join("\n")
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

/// Suggest the closest CLI subcommand for a typo'd input.
pub fn suggest_subcommand(input: &str) -> Option<String> {
    use crate::cli::subcommand_names;
    let names = subcommand_names();
    let normalized = input.to_ascii_lowercase();
    let mut best: Option<(usize, String)> = None;
    for cmd in &names {
        let d = edit_distance(&normalized, cmd);
        if d <= 2 && (best.is_none() || d < best.as_ref().unwrap().0) {
            best = Some((d, cmd.clone()));
        }
    }
    best.map(|(_, cmd)| cmd)
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
