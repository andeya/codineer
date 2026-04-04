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

Built in safe Rust. Ships as a **single ~15 MB binary**. No daemon, no runtime dependency — bring any model and go.

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

- **Provider freedom** — switch between Claude, GPT, Grok, Ollama, LM Studio, OpenRouter, Groq, or any OpenAI-compatible API with `--model`. No vendor lock-in.
- **Zero-config local AI** — start Ollama, run `codineer`. Auto-detects local models and picks the best one.
- **Instant setup** — `brew install` or `cargo install`. One Rust binary, no runtime dependencies.
- **Graceful degradation** — models without function calling automatically fall back to text-only mode.
- **Project memory** — `CODINEER.md` gives the AI persistent context about your codebase. Commit it to share with your team.

## Table of Contents

- [Install](#install)
- [Quick Start](#quick-start)
- [Models & Providers](#models--providers)
- [Usage](#usage)
- [Configuration](#configuration)
- [Project Context](#project-context)
- [Extending Codineer](#extending-codineer)
- [Reference](#reference)
- [License](#license)

---

## Install

```bash
brew install andeya/codineer/codineer            # Homebrew (macOS / Linux)
cargo install codineer-cli                        # Cargo (from crates.io)
```

Or download a prebuilt binary from [Releases](https://github.com/andeya/codineer/releases):

| Platform              | File                                          |
| --------------------- | --------------------------------------------- |
| macOS (Apple Silicon) | `codineer-*-aarch64-apple-darwin.tar.gz`      |
| macOS (Intel)         | `codineer-*-x86_64-apple-darwin.tar.gz`       |
| Linux (x86_64)        | `codineer-*-x86_64-unknown-linux-gnu.tar.gz`  |
| Linux (ARM64)         | `codineer-*-aarch64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64)      | `codineer-*-x86_64-pc-windows-msvc.zip`       |

<details><summary>Build from source</summary>

```bash
git clone https://github.com/andeya/codineer.git
cd codineer
cargo install --path crates/codineer-cli --locked
```

</details>

---

## Quick Start

```bash
# 1. Pick a provider — any one of these:
export ANTHROPIC_API_KEY="sk-ant-..."             # Claude
export OPENAI_API_KEY="sk-..."                    # GPT
export XAI_API_KEY="xai-..."                      # Grok
export OPENROUTER_API_KEY="..."                   # OpenRouter (free models)
export GROQ_API_KEY="..."                         # Groq Cloud (free tier)
ollama serve                                      # Local AI (no key needed)
codineer login                                    # Or OAuth

# 2. Initialize project context (optional)
codineer init

# 3. Start coding
codineer                                          # Interactive REPL
codineer "explain this project"                   # One-shot prompt
```

Codineer auto-detects your provider. No extra flags needed. All credentials can also go in [settings.json](#configuration) instead of shell exports.

---

## Models & Providers

### Built-in aliases

| Alias       | Model                         | Provider  |
| ----------- | ----------------------------- | --------- |
| `opus`      | `claude-opus-4-6`             | Anthropic |
| `sonnet`    | `claude-sonnet-4-6`           | Anthropic |
| `haiku`     | `claude-haiku-4-5-20251213`   | Anthropic |
| `grok`      | `grok-3`                      | xAI       |
| `grok-mini` | `grok-3-mini`                 | xAI       |
| `grok-2`    | `grok-2`                      | xAI       |
| `gpt`       | `gpt-4o`                      | OpenAI    |
| `mini`      | `gpt-4o-mini`                 | OpenAI    |
| `o3`        | `o3`                          | OpenAI    |
| `o3-mini`   | `o3-mini`                     | OpenAI    |

```bash
codineer --model opus "review my changes"
codineer --model grok-mini "quick question"
```

### Custom providers (OpenAI-compatible)

Use any OpenAI-compatible API with the `provider/model` syntax:

| Prefix                  | Provider    | API key        |
| ----------------------- | ----------- | -------------- |
| `ollama/<model>`        | Ollama      | —              |
| `lmstudio/<model>`      | LM Studio   | —              |
| `groq/<model>`          | Groq Cloud  | `GROQ_API_KEY` |
| `openrouter/<model>`    | OpenRouter  | `OPENROUTER_API_KEY` |

```bash
codineer --model ollama/qwen3-coder "refactor this module"
codineer --model groq/llama-3.3-70b-versatile "explain this"
codineer --model ollama              # auto-pick best coding model
```

**Zero-config Ollama**: when no API keys are found and Ollama is running, Codineer auto-detects it and picks the best coding model. Supports `OLLAMA_HOST` env var and remote instances (see [Configuration](#environment-variables)).

> Models without function calling automatically fall back to text-only mode — every model works.

### Model resolution order

When no `--model` flag is given:
1. `model` field in settings.json
2. Auto-detect from available API credentials
3. Auto-detect running Ollama instance

Switch model mid-session: `/model <name>`

---

## Usage

### Interactive REPL

```bash
codineer
```

Type naturally. Use **slash commands** (Tab-autocomplete supported):

| Category | Commands |
| -------- | -------- |
| **Info** | `/help` `/status` `/version` `/model` `/cost` `/config` `/memory` |
| **Session** | `/compact` `/clear` `/session` `/resume` `/export` |
| **Git** | `/diff` `/branch` `/commit` `/commit-push-pr` `/pr` `/issue` `/worktree` |
| **Agents** | `/agents` `/skills` `/plugin` |
| **Advanced** | `/ultraplan` `/bughunter` `/teleport` `/debug-tool-call` `/vim` |
| **Navigation** | `/init` `/permissions` `/exit` |

**Keyboard shortcuts:** `Up`/`Down` history, `Tab` completion, `Shift+Enter` newline, `Ctrl+C` cancel.

### One-shot prompts

```bash
codineer "explain this project's architecture"
codineer -p "list all TODO comments" --output-format json
codineer --model sonnet --permission-mode read-only "audit the codebase"
```

| Flag | Description |
| ---- | ----------- |
| `-p <text>` | One-shot prompt |
| `--model <name>` | Choose model |
| `--output-format text\|json` | Output format |
| `--allowedTools <list>` | Restrict tool access (comma-separated) |
| `--permission-mode <mode>` | `read-only`, `workspace-write`, `danger-full-access` |
| `--resume <file>` | Resume a saved session |
| `-V`, `--version` | Print version |

### Permission modes

| Mode | Allows |
| ---- | ------ |
| `read-only` | Read and search only — no writes |
| `workspace-write` | Edit files inside the workspace (default) |
| `danger-full-access` | Unrestricted access including system commands |

### Scripting & CI

```bash
codineer -p "check for security issues" \
  --permission-mode read-only \
  --allowedTools read_file,grep_search \
  --output-format json | jq '.content[0].text'
```

---

## Configuration

### Settings files

Codineer merges JSON settings from multiple files (highest to lowest precedence):

| File | Scope | Committed? |
| ---- | ----- | ---------- |
| `.codineer/settings.local.json` | Project — local overrides | No (gitignored) |
| `.codineer/settings.json` | Project — team settings | Yes |
| `.codineer.json` | Project — flat config | Yes |
| `~/.codineer/settings.json` | User — global | — |
| `~/.codineer.json` | User — global flat config | — |

All files use the same schema. Objects like `env`, `providers`, and `mcpServers` are deep-merged across layers.

### Settings reference

```json
{
  "model": "sonnet",
  "permissionMode": "workspace-write",
  "env": {
    "ANTHROPIC_API_KEY": "sk-ant-...",
    "OLLAMA_HOST": "http://192.168.1.100:11434"
  },
  "providers": {
    "ollama": { "baseUrl": "http://my-server:11434/v1" },
    "my-api": { "baseUrl": "https://api.example.com/v1", "apiKeyEnv": "MY_KEY" }
  },
  "mcpServers": { "my-server": { "command": "node", "args": ["server.js"] } },
  "plugins": ["my-plugin"],
  "hooks": { "PreToolUse": ["lint-check"], "PostToolUse": ["notify"] }
}
```

| Key | Type | Description |
| --- | ---- | ----------- |
| `model` | string | Default model (e.g. `"sonnet"`, `"ollama/qwen3-coder"`) |
| `permissionMode` | string | `"read-only"`, `"workspace-write"`, or `"danger-full-access"` |
| `env` | object | Environment variables injected at startup. Shell exports take precedence. |
| `providers` | object | Custom OpenAI-compatible provider endpoints |
| `mcpServers` | object | MCP server definitions (stdio, sse, http, ws) |
| `plugins` | array | Plugin names to load |
| `hooks` | object | Shell commands for `PreToolUse` / `PostToolUse` hooks |

Inspect merged config at runtime: `/config`, `/config env`, `/config model`

### Environment variables

Set via shell export **or** the `"env"` section in settings.json (shell exports take precedence):

| Variable | Purpose |
| -------- | ------- |
| `ANTHROPIC_API_KEY` | Claude API key |
| `ANTHROPIC_AUTH_TOKEN` | Bearer token (alternative) |
| `XAI_API_KEY` | xAI / Grok API key |
| `OPENAI_API_KEY` | OpenAI API key |
| `OPENROUTER_API_KEY` | OpenRouter API key |
| `GROQ_API_KEY` | Groq Cloud API key |
| `OLLAMA_HOST` | Ollama endpoint (e.g. `http://192.168.1.100:11434`) |
| `CODINEER_WORKSPACE_ROOT` | Override workspace root |
| `CODINEER_CONFIG_HOME` | Override config dir (`~/.codineer`) |
| `CODINEER_PERMISSION_MODE` | Default permission mode |
| `NO_COLOR` | Disable ANSI colors |
| `CLICOLOR=0` | Disable ANSI colors (alternative) |

**Credential precedence:** shell environment → settings.json `"env"` → OAuth (`codineer login`)

---

## Project Context

`CODINEER.md` is the project memory file. It tells the AI about your codebase, conventions, and workflows.

```bash
codineer init        # auto-generate from detected stack
```

Codineer walks up the directory tree and loads all matching instruction files:

| File | Purpose |
| ---- | ------- |
| `CODINEER.md` | Primary context (commit this) |
| `CODINEER.local.md` | Personal overrides (gitignore this) |
| `.codineer/CODINEER.md` | Alternative location |
| `.codineer/instructions.md` | Additional instructions |

---

## Extending Codineer

### MCP servers

Connect external tools via the [Model Context Protocol](https://modelcontextprotocol.io):

```json
{
  "mcpServers": {
    "my-server": { "command": "node", "args": ["mcp-server.js"] }
  }
}
```

Transports: `stdio` (default), `sse`, `http`, `ws`.

### Plugins

```bash
/plugin list                        # list installed
/plugin install ./path/to/plugin    # install local
/plugin enable my-plugin            # enable
```

### Agents & skills

**Agents** are named sub-agent configs for specialized tasks. **Skills** are reusable prompt templates.

```bash
codineer agents          # list agents
codineer skills          # list skills
/agents                  # inside REPL
/skills                  # inside REPL
```

Skills are discovered from `.codineer/skills/`, `~/.codineer/skills/`, and `$CODINEER_CONFIG_HOME/skills/`.

---

## Reference

### Built-in tools

| Tool | Description |
| ---- | ----------- |
| `bash` | Execute shell commands |
| `PowerShell` | Execute PowerShell commands (Windows) |
| `read_file` | Read file contents |
| `write_file` | Create or overwrite files |
| `edit_file` | Targeted string replacement |
| `glob_search` | Find files by glob pattern |
| `grep_search` | Search file contents with regex |
| `WebFetch` | Fetch and summarize web pages |
| `WebSearch` | Web search via DuckDuckGo |
| `NotebookEdit` | Edit Jupyter notebook cells |
| `TodoWrite` | Manage structured task lists |
| `Agent` | Launch sub-agents |
| `Skill` | Execute skill prompts |
| `REPL` | Run code in Python, Node, or shell |
| `ToolSearch` | Search available tools |
| `Sleep` | Pause execution for a duration |
| `SendUserMessage` | Send a message to the user |
| `Config` | Read/write config values |
| `StructuredOutput` | Return structured JSON |

### Crate structure

All crates are published to crates.io. Install `codineer-cli` — the others are internal dependencies.

| Crate | Role |
| ----- | ---- |
| `codineer-cli` | CLI binary (**install this**) |
| `codineer-runtime` | Core runtime engine |
| `codineer-api` | AI provider API clients |
| `codineer-tools` | Tool definitions & execution |
| `codineer-plugins` | Plugin system and hooks |
| `codineer-commands` | Slash commands |
| `codineer-lsp` | LSP client integration |

---

## License

[MIT](LICENSE)

---

<p align="center">
  Made with 🦀 by <a href="https://github.com/andeya">andeya</a>
</p>
