<p align="center">
  <img src="https://raw.githubusercontent.com/andeya/codineer/main/assets/logo.svg" width="96" alt="">
</p>
<h1 align="center">codineer</h1>
<p align="center">
  <em>Your multi-provider AI coding agent — one binary, any model, zero lock-in.</em>
</p>

<p align="center">
  <a href="https://github.com/andeya/codineer/actions"><img src="https://github.com/andeya/codineer/workflows/CI/badge.svg" alt="CI"></a>
  <a href="https://github.com/andeya/codineer/releases"><img src="https://img.shields.io/github/v/release/andeya/codineer" alt="Release"></a>
  <a href="https://crates.io/crates/codineer-cli"><img src="https://img.shields.io/crates/v/codineer-cli.svg" alt="crates.io"></a>
  <img src="https://raw.githubusercontent.com/andeya/codineer/main/assets/badge-platforms.svg" alt="macOS | Linux | Windows">
  <br>
  <a href="README_CN.md">中文文档</a>
</p>

---

**Codineer** turns your terminal into an AI coding companion. It reads your workspace, understands project context, and helps you write, refactor, debug, and ship code — without leaving the command line.

Built in safe Rust. Ships as a **single binary**. No daemon, no runtime dependency — bring any model and go.

## Why Codineer?

Most AI coding CLIs lock you into a single provider. Claude Code requires Anthropic. Codex CLI requires OpenAI. **Codineer works with all of them — and local models too.**

| | Codineer | Claude Code | Codex CLI | Aider |
|---|:---:|:---:|:---:|:---:|
| **Multi-provider** (Anthropic, OpenAI, xAI, Ollama, …) | **All built-in** | Anthropic only | OpenAI + Ollama | Yes |
| **Zero-config local AI** (auto-detect Ollama) | **Yes** | No | `--oss` flag | Manual setup |
| **Single binary** (no runtime deps) | **Rust** | Node.js | Node.js | Python |
| **MCP protocol** (external tool integration) | **Yes** | Yes | Yes | Yes |
| **Plugin system** + agents + skills | **Yes** | Yes | No | No |
| **Permission modes** (read-only → full access) | **Yes** | Yes | Yes | Partial |
| **Tool-use fallback** (graceful degradation) | **Yes** | N/A | N/A | N/A |
| **Git workflow** (/commit, /pr, /diff, /branch) | **Built-in** | Via tools | Via tools | Auto-commit |
| **Vim mode** in REPL | **Yes** | No | No | No |
| **CI/CD ready** (JSON output, tool allowlists) | **Yes** | Yes | Yes | Limited |

**Key advantages:**

- **Provider freedom** — switch between Claude, GPT, Grok, Ollama, LM Studio, OpenRouter, Groq, or any OpenAI-compatible API with a single `--model` flag. No vendor lock-in.
- **Zero-config local AI** — start Ollama, run `codineer`. No API keys, no flags, no config. Codineer auto-detects your local models and picks the best one for coding.
- **Instant setup** — one `cargo install` or `brew install`. No Node.js, no Python, no Docker. A single ~15 MB Rust binary that runs anywhere.
- **Graceful degradation** — models without function calling automatically fall back to text-only mode. Every model works, even simple ones.
- **Project memory** — `CODINEER.md` gives the AI persistent context about your codebase, conventions, and workflows. Commit it to share with your team.

## Table of Contents

- [Why Codineer?](#why-codineer)
- [Install](#install)
- [Quick Start](#quick-start)
- [Usage Guide](#usage-guide)
  - [Interactive REPL](#interactive-repl)
  - [One-shot Prompts](#one-shot-prompts)
  - [Session Management](#session-management)
  - [Model Selection](#model-selection)
  - [Permission Modes](#permission-modes)
  - [Scripting & Automation](#scripting--automation)
- [Project Setup](#project-setup)
- [Configuration](#configuration)
  - [Config file hierarchy](#config-file-hierarchy)
  - [Settings reference](#settings-reference)
  - [Environment variables](#environment-variables)
- [Extending Codineer](#extending-codineer)
  - [MCP Servers](#mcp-servers)
  - [Plugins](#plugins)
  - [Agents & Skills](#agents--skills)
- [Built-in Tools](#built-in-tools)
- [Publishing to crates.io](#publishing-to-cratesio)
- [License](#license)

---

## Install

### Homebrew (macOS / Linux)

```bash
brew install andeya/codineer/codineer
```

### Cargo (from crates.io)

```bash
cargo install codineer-cli
```

### Download binary

Grab the prebuilt binary for your platform from the **[Releases](https://github.com/andeya/codineer/releases)** page:

| Platform              | File                                          |
| --------------------- | --------------------------------------------- |
| macOS (Apple Silicon) | `codineer-*-aarch64-apple-darwin.tar.gz`      |
| macOS (Intel)         | `codineer-*-x86_64-apple-darwin.tar.gz`       |
| Linux (x86_64)        | `codineer-*-x86_64-unknown-linux-gnu.tar.gz`  |
| Linux (ARM64)         | `codineer-*-aarch64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64)      | `codineer-*-x86_64-pc-windows-msvc.zip`       |

### Build from source

```bash
git clone https://github.com/andeya/codineer.git
cd codineer
cargo install --path crates/codineer-cli --locked
```

---

## Quick Start

**1. Authenticate** — pick one method:

```bash
# Cloud providers (requires API key)
export ANTHROPIC_API_KEY="sk-ant-..."   # Claude (recommended)
export XAI_API_KEY="xai-..."            # Grok
export OPENAI_API_KEY="sk-..."          # GPT / OpenAI-compatible

# Free cloud providers
export OPENROUTER_API_KEY="..."         # OpenRouter (free models available)
export GROQ_API_KEY="..."               # Groq Cloud (generous free tier)

# Local models (no API key needed)
ollama serve                            # Start Ollama, then run codineer
codineer --model ollama/qwen3-coder     # Specify model explicitly

# Or configure everything in settings.json (see Configuration section)
# ~/.codineer/settings.json:
#   { "model": "sonnet", "env": { "ANTHROPIC_API_KEY": "sk-ant-..." } }

# OAuth login (stored in keyring)
codineer login
```

**2. Initialize your project** (optional but recommended):

```bash
cd your-project
codineer init        # generates CODINEER.md with project context
```

**3. Start coding:**

```bash
codineer             # open interactive REPL
```

Codineer auto-detects which provider to use from your environment — no extra configuration needed.

---

## Usage Guide

### Interactive REPL

The default mode. Launch with no arguments, then type naturally:

```bash
codineer
```

Inside the REPL you can use **slash commands** (Tab-autocomplete supported):

**Session & info**

| Command                                | Description                                          |
| -------------------------------------- | ---------------------------------------------------- |
| `/help`                                | Show all available commands                          |
| `/status`                              | Session info: model, tokens, git branch, config      |
| `/version`                             | Print Codineer version                               |
| `/model [name]`                        | View or switch the active model                      |
| `/permissions [mode]`                  | View or change permission mode                       |
| `/cost`                                | Show token usage and estimated cost                  |
| `/compact`                             | Compress conversation history to save tokens         |
| `/clear [--confirm]`                   | Reset conversation (requires `--confirm` to execute) |
| `/session [list\|switch <id>]`         | List or switch named sessions                        |
| `/resume <file>`                       | Resume a saved session file                          |
| `/export [file]`                       | Export conversation to Markdown                      |
| `/memory`                              | Inspect loaded CODINEER.md memory files              |
| `/config [env\|hooks\|model\|plugins]` | Inspect merged configuration                         |
| `/init`                                | Re-generate CODINEER.md for current project          |

**Git & workflow**

| Command              | Description                                    |
| -------------------- | ---------------------------------------------- |
| `/diff`              | Show workspace git diff                        |
| `/branch`            | Show or manage git branches                    |
| `/commit`            | Create a git commit                            |
| `/commit-push-pr`    | Commit, push, and create a pull request        |
| `/pr`                | Create or manage pull requests                 |
| `/issue`             | Create or browse GitHub issues                 |
| `/worktree`          | Manage git worktrees                           |

**Agents & plugins**

| Command                              | Description                              |
| ------------------------------------ | ---------------------------------------- |
| `/plugin list\|install\|enable\|...` | Manage plugins                           |
| `/agents`                            | List configured sub-agents               |
| `/skills`                            | List available skills                    |

**Advanced**

| Command              | Description                                    |
| -------------------- | ---------------------------------------------- |
| `/ultraplan`         | Generate a detailed implementation plan        |
| `/bughunter`         | Systematic bug hunting mode                    |
| `/teleport`          | Jump to a specific file or symbol              |
| `/debug-tool-call`   | Debug the last tool call                       |
| `/vim`               | Toggle Vim-style modal editing                 |
| `/exit` or `/quit`   | Exit the REPL                                  |

**Keyboard shortcuts:**

| Key                       | Action                                         |
| ------------------------- | ---------------------------------------------- |
| `Up` / `Down`             | Browse input history                           |
| `Tab`                     | Cycle slash command completions                |
| `Shift+Enter` or `Ctrl+J` | Insert newline (multi-line input)              |
| `Ctrl+C`                  | Cancel current input or interrupt running tool |

### One-shot Prompts

Run a single prompt and exit — perfect for scripts and CI:

```bash
codineer "explain this project's architecture"
codineer prompt "list all TODO comments" --output-format json
codineer -p "summarize Cargo.toml" --model sonnet
```

Flags:

| Flag                              | Description                                                  |
| --------------------------------- | ------------------------------------------------------------ |
| `-p <text>`                       | One-shot prompt (rest of line is the prompt)                 |
| `--model <name>`                  | Choose model (see [Model Selection](#model-selection))       |
| `--output-format text\|json`      | Output format (default: `text`)                              |
| `--allowedTools <list>`           | Comma-separated tool allowlist (repeatable)                  |
| `--permission-mode <mode>`        | Permission level (see [Permission Modes](#permission-modes)) |
| `--dangerously-skip-permissions`  | Skip all permission checks                                   |
| `--version`, `-V`                 | Print version and build info                                 |

### Session Management

Save and restore conversations across terminal sessions:

```bash
# Inside the REPL — export to a file
/export session.json

# Resume later, optionally running slash commands immediately
codineer --resume session.json
codineer --resume session.json /status /compact /cost
```

### Model Selection

Codineer supports short aliases for popular models:

| Alias       | Resolves to                   | Provider  |
| ----------- | ----------------------------- | --------- |
| `opus`      | `claude-opus-4-6`             | Anthropic |
| `sonnet`    | `claude-sonnet-4-6`           | Anthropic |
| `haiku`     | `claude-haiku-4-5-20251213`   | Anthropic |
| `grok`      | `grok-3`                      | xAI       |
| `grok-mini` | `grok-3-mini`                 | xAI       |
| `gpt`       | `gpt-4o`                      | OpenAI    |
| `mini`      | `gpt-4o-mini`                 | OpenAI    |
| `o3`        | `o3`                          | OpenAI    |

```bash
codineer --model opus "review my changes"
codineer --model grok-mini "quick question"
```

#### Custom Providers (OpenAI-compatible)

Use any OpenAI-compatible API via the `provider/model` syntax:

| Prefix                       | Provider    | API key needed? |
| ---------------------------- | ----------- | --------------- |
| `ollama/<model>`             | Ollama      | No              |
| `lmstudio/<model>`           | LM Studio   | No              |
| `groq/<model>`               | Groq Cloud  | `GROQ_API_KEY`  |
| `openrouter/<model>`         | OpenRouter  | `OPENROUTER_API_KEY` |

```bash
codineer --model ollama/qwen3-coder "refactor this module"
codineer --model groq/llama-3.3-70b-versatile "explain this function"
codineer --model ollama   # auto-selects the best coding model from Ollama
```

**Zero-config Ollama**: if no API keys are found and Ollama is running locally, Codineer auto-detects it and picks the best available coding model.

**Ollama host resolution** (highest priority first):

1. **settings.json**: `{"providers": {"ollama": {"baseUrl": "http://my-server:11434/v1"}}}`
2. **Environment variable**: `export OLLAMA_HOST=http://192.168.1.100:11434`
3. **Default**: `http://localhost:11434`

This means remote Ollama instances and non-default ports are fully supported.

Configure custom providers in settings:

```json
{
  "providers": {
    "ollama": { "baseUrl": "http://192.168.1.100:11434/v1" },
    "my-api": { "baseUrl": "https://my-endpoint.com/v1", "apiKeyEnv": "MY_API_KEY" }
  }
}
```

> **Note**: models that do not support function calling will automatically fall back to text-only mode.

Switch model mid-session with `/model <name>`.

Set a persistent default model in your settings file:

```json
{ "model": "sonnet" }
```

When no `--model` flag is given, Codineer checks the config `model` field, then auto-detects from available provider credentials, then checks for a running Ollama instance.

### Permission Modes

Control what tools the agent can use:

| Mode                 | What it allows                                     |
| -------------------- | -------------------------------------------------- |
| `read-only`          | Read and search tools only — no writes             |
| `workspace-write`    | Edit files inside the workspace (default)          |
| `danger-full-access` | Unrestricted tool access including system commands |

```bash
codineer --permission-mode read-only "audit the codebase"
codineer --permission-mode danger-full-access "run full test suite and fix failures"
```

Switch permission mid-session with `/permissions <mode>`.

### Scripting & Automation

Use `--output-format json` and pipe output for integration with other tools:

```bash
# Extract structured data
codineer -p "list all public functions in src/" --output-format json | jq '.content[0].text'

# CI pipeline example
codineer -p "check for security issues" \
  --permission-mode read-only \
  --allowedTools read_file,grep_search \
  --output-format json
```

---

## Project Setup

**`CODINEER.md`** is the project memory file — it tells Codineer about your codebase, conventions, and workflows. Generate one automatically:

```bash
codineer init
```

This creates a `CODINEER.md` in your project root with detected stack, verification commands, and repository shape. Commit it to share context with your whole team.

Example `CODINEER.md`:

```markdown
# CODINEER.md

## Detected stack

- Languages: Rust, TypeScript

## Verification

- `cargo test --workspace`
- `npm test`

## Working agreement

- All PRs require passing CI
- Use conventional commits
```

Codineer walks up the directory tree from your workspace root and loads all matching instruction files:

| File                        | Purpose                                       |
| --------------------------- | --------------------------------------------- |
| `CODINEER.md`               | Primary project context (commit this)         |
| `CODINEER.local.md`         | Personal overrides (gitignore this)           |
| `.codineer/CODINEER.md`     | Alternative location inside `.codineer/` dir  |
| `.codineer/instructions.md` | Additional instructions                       |

---

## Configuration

### Config file hierarchy

Codineer merges settings from multiple files (highest to lowest precedence):

| File | Scope | Committed? |
| ---- | ----- | ---------- |
| `.codineer/settings.local.json` | Project — local overrides | No (gitignored) |
| `.codineer/settings.json` | Project — shared team settings | Yes |
| `.codineer.json` | Project — flat config (alternative) | Yes |
| `~/.codineer/settings.json` | User — global settings | — |
| `~/.codineer.json` | User — global flat config (alternative) | — |

All files share the same JSON schema. Values in higher-priority files override lower ones; objects like `env`, `providers`, and `mcpServers` are deep-merged.

### Settings reference

A settings file is a JSON object with the following optional keys:

```json
{
  "model": "sonnet",
  "permissionMode": "workspace-write",
  "env": {
    "ANTHROPIC_API_KEY": "sk-ant-...",
    "GROQ_API_KEY": "gsk_...",
    "OLLAMA_HOST": "http://192.168.1.100:11434"
  },
  "providers": {
    "ollama": { "baseUrl": "http://my-server:11434/v1" },
    "my-api": { "baseUrl": "https://api.example.com/v1", "apiKeyEnv": "MY_KEY" }
  },
  "mcpServers": {
    "my-server": { "command": "node", "args": ["server.js"] }
  },
  "plugins": ["my-plugin"],
  "hooks": {
    "PreToolUse": ["lint-check"],
    "PostToolUse": ["notify"]
  }
}
```

| Key | Type | Description |
| --- | ---- | ----------- |
| `model` | string | Default model (e.g. `"sonnet"`, `"ollama/qwen3-coder"`) |
| `permissionMode` | string | `"read-only"`, `"workspace-write"`, or `"danger-full-access"` |
| `env` | object | Key-value pairs injected as environment variables at startup. Shell exports take precedence over values here. Accepts any variable: API keys, `OLLAMA_HOST`, `NO_COLOR`, etc. |
| `providers` | object | Custom OpenAI-compatible providers (see [Custom Providers](#custom-providers-openai-compatible)) |
| `mcpServers` | object | MCP server definitions (see [MCP Servers](#mcp-servers)) |
| `plugins` | array | Plugin names to load (see [Plugins](#plugins)) |
| `hooks` | object | Shell commands for `PreToolUse` / `PostToolUse` lifecycle hooks |

### Inspecting merged config

```bash
/config          # full merged config
/config env      # environment variables section
/config model    # model settings
/config plugins  # plugin configuration
/config hooks    # hook configuration
```

### Environment variables

These variables can be set via shell export **or** the `"env"` section of settings.json (shell exports always take precedence):

| Variable                    | Purpose                                    |
| --------------------------- | ------------------------------------------ |
| `ANTHROPIC_API_KEY`         | Claude API key                             |
| `ANTHROPIC_AUTH_TOKEN`      | Bearer token (alternative to API key)      |
| `XAI_API_KEY`               | xAI / Grok API key                         |
| `OPENAI_API_KEY`            | OpenAI API key                             |
| `OPENROUTER_API_KEY`        | OpenRouter API key (free models available) |
| `GROQ_API_KEY`              | Groq Cloud API key (free tier available)   |
| `OLLAMA_HOST`               | Ollama endpoint (e.g. `http://192.168.1.100:11434`) |
| `CODINEER_WORKSPACE_ROOT`   | Override workspace root path               |
| `CODINEER_CONFIG_HOME`      | Override config directory (`~/.codineer`)  |
| `CODINEER_PERMISSION_MODE`  | Default permission mode                    |
| `NO_COLOR`                  | Disable ANSI color output                  |

Three ways to provide API credentials (in order of precedence):
1. **Shell environment** — `export ANTHROPIC_API_KEY=sk-ant-...`
2. **Settings file** — `{"env": {"ANTHROPIC_API_KEY": "sk-ant-..."}}`
3. **OAuth** — `codineer login` (stored in system keyring)

---

## Extending Codineer

### MCP Servers

Codineer supports the [Model Context Protocol](https://modelcontextprotocol.io) for connecting external tools. Configure MCP servers in your settings:

```json
{
  "mcpServers": {
    "my-server": {
      "command": "node",
      "args": ["path/to/mcp-server.js"],
      "type": "stdio"
    }
  }
}
```

Supported transport types: `stdio` (default), `sse`, `http`, `ws` (or `websocket`).

### Plugins

Plugins add custom tools and hooks. Manage them from the REPL:

```bash
/plugin list                          # list installed plugins
/plugin install ./path/to/plugin      # install a local plugin
/plugin enable my-plugin              # enable a plugin
/plugin disable my-plugin             # disable a plugin
/plugin update my-plugin              # update to latest
/plugin uninstall my-plugin-id        # remove a plugin
```

### Agents & Skills

**Agents** are named sub-agent configurations for specialized tasks:

```bash
codineer agents          # list configured agents
codineer agents --help   # show agent options
/agents                  # same, inside the REPL
```

**Skills** are reusable prompt templates. Codineer searches for skills in `.codineer/skills/`, `$CODINEER_CONFIG_HOME/skills/`, and `~/.codineer/skills/`:

```bash
codineer skills          # list available skills
codineer /skills help    # show skill details
/skills                  # same, inside the REPL
```

---

## Built-in Tools

Codineer ships with a rich set of tools the AI can invoke:

| Tool               | Description                                         |
| ------------------ | --------------------------------------------------- |
| `bash`             | Execute shell commands                              |
| `PowerShell`       | Execute PowerShell commands (Windows)               |
| `read_file`        | Read file contents with optional offset/limit       |
| `write_file`       | Create or overwrite files                           |
| `edit_file`        | Targeted string replacement in files                |
| `glob_search`      | Find files matching a glob pattern                  |
| `grep_search`      | Search file contents with regex                     |
| `WebFetch`         | Fetch and summarize a web page                      |
| `WebSearch`        | Search the web via DuckDuckGo                       |
| `NotebookEdit`     | Edit Jupyter notebook cells                         |
| `TodoWrite`        | Manage a structured task list                       |
| `Agent`            | Launch a sub-agent for complex tasks                |
| `Skill`            | Load and execute a skill prompt                     |
| `ToolSearch`       | Search available tools by keyword                   |
| `REPL`             | Run a persistent language REPL (Python, Node, etc.) |
| `Sleep`            | Pause execution for a given duration                |
| `SendUserMessage`  | Send a message to the user                          |
| `Config`           | Read or write configuration values                  |
| `StructuredOutput` | Return structured JSON output                       |

---

## Publishing to crates.io

All crates are published to crates.io with a `codineer-` prefix. Releases are automated via GitHub Actions — tag a version to trigger the release pipeline:

```bash
git tag v0.6.0
git push origin v0.6.0
```

| Crate               | Description                            |
| -------------------- | -------------------------------------- |
| `codineer-cli`       | CLI binary — **install this one**      |
| `codineer-runtime`   | Core runtime engine                    |
| `codineer-api`       | AI provider API clients                |
| `codineer-tools`     | Built-in tool definitions & execution  |
| `codineer-plugins`   | Plugin system and hooks                |
| `codineer-commands`  | Slash commands and discovery           |
| `codineer-lsp`       | LSP client integration                 |

> **Note:** The library crates are internal implementation details of `codineer-cli`. They are published to satisfy crates.io dependency requirements but their APIs are not guaranteed stable for external use.

---

## License

[MIT](LICENSE)

---

<p align="center">
  Made with 🦀 by <a href="https://github.com/andeya">andeya</a>
</p>
