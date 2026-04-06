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

<p align="center">
  <img src="https://raw.githubusercontent.com/andeya/codineer/main/assets/ScreenShot_01.png" alt="Codineer REPL screenshot" width="780">
</p>

## Why Codineer?

Most AI coding CLIs lock you into a single provider. Claude Code requires Anthropic. Codex CLI requires OpenAI. **Codineer works with all of them — and local models too.**

|                                                                                                          |     Codineer     |  Claude Code   |    Codex CLI    |    Aider     |
| -------------------------------------------------------------------------------------------------------- | :--------------: | :------------: | :-------------: | :----------: |
| **Multi-provider** (Anthropic, OpenAI, xAI, Ollama, …)                                                   | **All built-in** | Anthropic only | OpenAI + Ollama |     Yes      |
| **Zero-token-cost** ([free access to major models](#openclaw-zero-token-free-access-to-major-ai-models)) |     **Yes**      |       No       |       No        |      No      |
| **Zero-config local AI** (auto-detect Ollama)                                                            |     **Yes**      |       No       |  `--oss` flag   | Manual setup |
| **Single binary** (no runtime deps)                                                                      |     **Rust**     |    Node.js     |     Node.js     |    Python    |
| **Multimodal input** (`@image.png`, clipboard paste, drag-and-drop)                                      |     **Yes**      |      Yes       |     Limited     |   Limited    |
| **MCP protocol** (external tool integration)                                                             |     **Yes**      |      Yes       |       Yes       |     Yes      |
| **Plugin system** + agents + skills                                                                      |     **Yes**      |      Yes       |       No        |      No      |
| **Permission modes** (read-only → full access)                                                           |     **Yes**      |      Yes       |       Yes       |   Partial    |
| **Tool-use fallback** (graceful degradation)                                                             |     **Yes**      |      N/A       |       N/A       |     N/A      |
| **Git workflow** (/commit, /pr, /diff, /branch)                                                          |   **Built-in**   |   Via tools    |    Via tools    | Auto-commit  |
| **Vim mode** in REPL                                                                                     |     **Yes**      |       No       |       No        |      No      |
| **CI/CD ready** (JSON output, tool allowlists)                                                           |     **Yes**      |      Yes       |       Yes       |   Limited    |

**Key advantages:**

- **Provider freedom** — switch between Claude, GPT, Grok, Ollama, LM Studio, OpenRouter, Groq, or any OpenAI-compatible API with `--model`. No vendor lock-in.
- **Zero token cost** — pair with [OpenClaw Zero Token](https://github.com/andeya/openclaw-zero-token) to use Claude, ChatGPT, Gemini, DeepSeek, and 10+ more models for free — no API key purchase needed.
- **Zero-config local AI** — start Ollama, run `codineer`. Auto-detects local models and picks the best one.
- **Instant setup** — `brew install` or `cargo install`. One Rust binary, no runtime dependencies.
- **Multimodal input** — attach images via `@photo.png`, paste from clipboard (`Ctrl+V` / `/image`), or drag-and-drop into the terminal. Works with Anthropic, OpenAI, and any multimodal-capable provider.
- **Graceful degradation** — models without function calling automatically fall back to text-only mode.
- **Project memory** — `CODINEER.md` gives the AI persistent context about your codebase. Commit it to share with your team.
- **Adaptive terminal UI** — welcome panel and separator line reflow in real time as the window resizes. Ultra-narrow terminals collapse to a single-column layout; the input line redraws in place without flicker. Works on macOS, Linux, and Windows (Windows Terminal / ConPTY).

## Table of Contents

- [Install](#install)
- [Quick Start](#quick-start)
- [Models & Providers](#models--providers)
- [Usage](#usage)
  - [File & Image Attachments](#file--image-attachments)
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
export GEMINI_API_KEY="AIzaSy..."                  # Google Gemini (free key from aistudio.google.com)
export DASHSCOPE_API_KEY="sk-..."                 # Alibaba DashScope (OpenAI-compatible)
ollama serve                                      # Local AI (no key needed)
# Or use OpenClaw Zero Token gateway for free access to all major models (see below)
codineer login                                    # Or OAuth login (default provider)
codineer login anthropic --source claude-code     # Use Claude Code credentials
codineer status                                   # Check authentication status
codineer config set model sonnet                   # Set default model (alias or full name)

# 2. Initialize project context (optional)
codineer init

# 3. Start coding
codineer                                          # Interactive REPL
codineer "explain this project"                   # One-shot prompt
```

Codineer auto-detects your provider. No extra flags needed. All credentials can also go in [settings.json](#configuration) instead of shell exports.

---

## Models & Providers

### Model aliases

Define your own short model names in `settings.json`:

```json
{
  "modelAliases": {
    "sonnet": "claude-sonnet-4-6",
    "opus": "claude-opus-4-6",
    "haiku": "claude-haiku-4-5-20251213",
    "grok": "grok-3",
    "gpt": "gpt-4o",
    "flash": "gemini/gemini-2.5-flash"
  }
}
```

```bash
codineer --model sonnet "review my changes"
codineer --model flash "quick question"
```

No aliases are built in — you decide what short names make sense for your workflow. View your configured aliases: `codineer models`.

### Custom providers (OpenAI-compatible)

Use any OpenAI-compatible API with the `provider/model` syntax:

| Prefix               | Provider   | API key              |
| -------------------- | ---------- | -------------------- |
| `ollama/<model>`     | Ollama     | —                    |
| `lmstudio/<model>`   | LM Studio  | —                    |
| `groq/<model>`       | Groq Cloud | `GROQ_API_KEY`       |
| `openrouter/<model>` | OpenRouter | `OPENROUTER_API_KEY` |

```bash
codineer --model ollama/qwen3-coder "refactor this module"
codineer --model groq/llama-3.3-70b-versatile "explain this"
codineer --model ollama              # auto-pick best coding model
```

**Zero-config Ollama**: when no API keys are found and Ollama is running, Codineer auto-detects it and picks the best coding model. Supports `OLLAMA_HOST` env var and remote instances (see [Configuration](#environment-variables)).

> Models without function calling automatically fall back to text-only mode — every model works.

### Google Gemini (OpenAI-compatible, free API key)

Get a **free** API key from [Google AI Studio](https://aistudio.google.com/apikey) — no credit card required. Configure the provider in `settings.json`:

```json
{
  "model": "gemini/gemini-2.5-flash",
  "env": {
    "GEMINI_API_KEY": "AIzaSy..."
  },
  "providers": {
    "gemini": {
      "baseUrl": "https://generativelanguage.googleapis.com/v1beta/openai",
      "apiKeyEnv": "GEMINI_API_KEY",
      "defaultModel": "gemini-2.5-flash",
      "models": ["gemini-2.5-flash", "gemini-2.5-pro"]
    }
  }
}
```

```bash
codineer --model gemini/gemini-2.5-flash "explain this code"
codineer --model gemini/gemini-2.5-pro "review my architecture"
```

> **Important:** Use the OpenAI-compatible endpoint (`/v1beta/openai`), not the native Gemini REST API (`/v1beta/models/...:generateContent`). Codineer appends `/chat/completions` to the base URL.

### Alibaba Cloud DashScope (OpenAI-compatible)

Use `provider/model` and configure `providers` in `settings.json` with the correct `baseUrl` (see [Model Studio docs](https://www.alibabacloud.com/help/en/model-studio/)):

```bash
export DASHSCOPE_API_KEY="sk-..."
codineer --model dashscope/qwen-plus-2025-07-28 "one-line Rust ownership"
```

Streaming deltas may use `reasoning_content`, `thought`, or array-shaped `content`; Codineer normalizes these. If you still see **assistant stream produced no content**, the CLI automatically retries **once** with a non-streaming request. Use a current build from source or the latest release.

### Azure OpenAI

Set optional `apiVersion` (e.g. `2024-02-15-preview`) on a `providers.<name>` entry; Codineer appends `api-version=…` to the `chat/completions` URL. See [`settings.example.json`](https://github.com/andeya/codineer/blob/main/settings.example.json) (`azure-openai`).

### List available models

```bash
codineer models               # All providers
codineer models anthropic     # Filter by provider
codineer models ollama        # Show local Ollama models
```

### Model resolution order

When no `--model` flag is given:

1. `model` field in settings.json
2. Auto-detect from available API credentials
3. Auto-detect running Ollama instance

If the resolved model lacks credentials, Codineer tries each entry in `fallbackModels` before giving up.

Switch model mid-session: `/model <name>`

### Fallback models

Set an ordered list of fallback models in `settings.json`. If the primary model is unavailable (e.g. missing API key), Codineer tries each fallback in order:

```json
{
  "model": "sonnet",
  "modelAliases": { "sonnet": "claude-sonnet-4-6" },
  "fallbackModels": ["ollama/qwen3-coder", "groq/llama-3.3-70b-versatile"]
}
```

`model` and `fallbackModels` both support your custom aliases from `modelAliases`. This is especially useful for zero-cost setups: set a cloud model as primary and local models as fallback.

### OpenClaw Zero Token (Free Access to Major AI Models)

> **Zero API token cost** — log in via browser once, then call Claude, ChatGPT, Gemini, DeepSeek, Qwen, Kimi, Grok, and more through a single unified gateway — completely free.

[OpenClaw Zero Token](https://github.com/andeya/openclaw-zero-token) is an AI gateway that drives official model web UIs (browser login) instead of paid API keys. If you can use a model in the browser, you can call it through Codineer — no API token purchase required.

**Highlights:**

| Traditional approach | OpenClaw Zero Token way  |
| -------------------- | ------------------------ |
| Buy API tokens       | **Completely free**      |
| Pay per request      | No enforced quota        |
| Credit card required | Browser login only       |
| API tokens may leak  | Credentials stored local |

**Supported models:** Claude Web, ChatGPT Web, Gemini Web, DeepSeek Web, Qwen Web (intl/cn), Kimi, Doubao, Grok Web, GLM Web, Xiaomi MiMo, and more. 11 out of 13 web models support tool calling.

**Setup:**

1. Deploy and start the [OpenClaw Zero Token](https://github.com/andeya/openclaw-zero-token) gateway (default port 3001)
2. Add the provider to `settings.json`:

```json
{
  "model": "openclaw-zero/openclaw",
  "env": {
    "OPENCLAW_ZERO_API_KEY": "your-gateway-token"
  },
  "providers": {
    "openclaw-zero": {
      "baseUrl": "http://127.0.0.1:3001/v1",
      "apiKeyEnv": "OPENCLAW_ZERO_API_KEY",
      "defaultModel": "openclaw",
      "models": ["openclaw"],
      "headers": {
        "x-openclaw-scopes": "operator.write"
      }
    }
  }
}
```

3. Start coding:

```bash
codineer --model openclaw-zero/openclaw "refactor this module"
codineer --model openclaw-zero/claude-web/claude-sonnet-4-6 "code review"
codineer --model openclaw-zero/deepseek-web/deepseek-chat "explain this code"
```

> The `headers` field is optional and lets you attach custom HTTP headers to every request sent to a provider. In the OpenClaw Zero Token scenario, `x-openclaw-scopes` controls the permission scope on the gateway.

---

## Usage

### Interactive REPL

```bash
codineer
```

A **framed welcome banner** shows workspace, directory, model, session, and a copy-paste `codineer --resume …` line. The banner and separator adapt to the current terminal width in real time — narrow terminals switch to a single-column layout automatically. The prompt is **`❯`**. Type naturally. Use **slash commands** (Tab-autocomplete supported):

| Category       | Commands                                                                 |
| -------------- | ------------------------------------------------------------------------ |
| **Info**       | `/help` `/status` `/version` `/model` `/cost` `/config` `/memory`        |
| **Session**    | `/compact` `/clear` `/session` `/resume` `/export`                       |
| **Git**        | `/diff` `/branch` `/commit` `/commit-push-pr` `/pr` `/issue` `/worktree` |
| **Agents**     | `/agents` `/skills` `/plugin`                                            |
| **Advanced**   | `/ultraplan` `/bughunter` `/teleport` `/debug-tool-call` `/vim`          |
| **Navigation** | `/init` `/permissions` `/exit`                                           |

**Keyboard shortcuts:**

| Shortcut                             | Action                                                                                                        |
| ------------------------------------ | ------------------------------------------------------------------------------------------------------------- |
| `?`                                  | Inline shortcuts reference panel                                                                              |
| `!<cmd>`                             | Bash mode — sends a shell command request to the AI                                                           |
| `@`                                  | File / image attachment (Tab-complete path; → image block)                                                    |
| `Ctrl+V` / `/image`                  | Paste clipboard image → inserts `[Image #N]` placeholder (`Ctrl+V` on macOS/Linux; `/image` on all platforms) |
| `/`                                  | Slash command completion (with Tab)                                                                           |
| `Up` / `Down`                        | History recall                                                                                                |
| `Shift+Enter`, `Ctrl+J`, `\ + Enter` | Insert newline                                                                                                |
| `Ctrl+C`                             | Cancel input; press twice on empty prompt to exit                                                             |
| `Ctrl+D`                             | Exit (on empty prompt)                                                                                        |
| `Double-tap Esc`                     | Clear input                                                                                                   |
| `/vim`                               | Toggle Vim modal editing                                                                                      |

### File & image attachments

Use the `@` prefix to attach context directly to your message:

| Syntax               | What happens                                                |
| -------------------- | ----------------------------------------------------------- |
| `@src/main.rs`       | Inject file content (up to 2000 lines)                      |
| `@src/main.rs:10-50` | Inject a specific line range                                |
| `@src/`              | List directory entries                                      |
| `@photo.png`         | Attach as a multimodal image block (base64)                 |
| `@archive.bin`       | Inject binary file metadata (size, type) — content not read |

**Clipboard & drag-and-drop images:**

| Input                                     | Result                                                                                |
| ----------------------------------------- | ------------------------------------------------------------------------------------- |
| `Ctrl+V` when clipboard contains an image | Inserts `[Image #N]` placeholder; image is sent on submit (macOS/Linux)               |
| `/image`                                  | Read clipboard image and insert `[Image #N]` placeholder — works on **all** platforms |
| Drag an image file path into the terminal | Auto-detected and attached as image block                                             |

> **Platform notes:**
>
> - **macOS** — use `Ctrl+V` (not `Cmd+V`). Terminal.app and iTerm2 intercept `Cmd+V` and cannot forward image data to the app.
> - **Linux** — `Ctrl+V` works in most terminals (gnome-terminal, konsole, kitty, etc.).
> - **Windows** — Windows Terminal intercepts `Ctrl+V` for text paste; use `/image` instead.

Images are transmitted as proper multimodal content — `source: base64` for Anthropic and `image_url` data-URLs for OpenAI-compatible providers. Supported formats: PNG, JPEG, GIF, WebP, BMP. Maximum size: 20 MB per image.

### One-shot prompts

```bash
codineer "explain this project's architecture"
codineer -p "list all TODO comments" --output-format json
codineer --model sonnet --permission-mode read-only "audit the codebase"
```

| Flag                         | Description                                          |
| ---------------------------- | ---------------------------------------------------- |
| `-p <text>`                  | One-shot prompt                                      |
| `--model <name>`             | Choose model                                         |
| `--output-format text\|json` | Output format                                        |
| `--allowedTools <list>`      | Restrict tool access (comma-separated)               |
| `--permission-mode <mode>`   | `read-only`, `workspace-write`, `danger-full-access` |
| `--resume <file>`            | Resume a saved session                               |
| `-V`, `--version`            | Print version                                        |

### Permission modes

| Mode                 | Allows                                        |
| -------------------- | --------------------------------------------- |
| `read-only`          | Read and search only — no writes              |
| `workspace-write`    | Edit files inside the workspace (default)     |
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

| File                            | Actual path                                    | Scope                     | Committed?      |
| ------------------------------- | ---------------------------------------------- | ------------------------- | --------------- |
| `.codineer/settings.local.json` | `<project root>/.codineer/settings.local.json` | Project — local overrides | No (gitignored) |
| `.codineer/settings.json`       | `<project root>/.codineer/settings.json`       | Project settings          | Yes             |
| `~/.codineer/settings.json`     | `$HOME/.codineer/settings.json`                | User — global settings    | —               |

All files use the same schema. `env`, `providers`, and `mcpServers` objects are deep-merged across layers. For `mcpServers`, a server name defined in a later file replaces (not deep-merges) the earlier definition. `codineer config set` writes to the global file (`~/.codineer/settings.json`).

### Settings reference

> **Full example with all fields:** [`settings.example.json`](https://github.com/andeya/codineer/blob/main/settings.example.json)

```json
{
  "model": "sonnet",
  "modelAliases": {
    "sonnet": "claude-sonnet-4-6",
    "flash": "gemini/gemini-2.5-flash"
  },
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
  "enabledPlugins": { "my-plugin@external": true },
  "hooks": { "PreToolUse": ["lint-check"], "PostToolUse": ["notify"] }
}
```

| Key              | Type     | Description                                                                                                                                                                                                        |
| ---------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `model`          | string   | Default model — alias or full name (e.g. `"sonnet"`, `"claude-sonnet-4-6"`, `"ollama/qwen3-coder"`)                                                                                                                |
| `modelAliases`   | object   | Custom short names mapping to full model IDs (e.g. `{"sonnet": "claude-sonnet-4-6"}`)                                                                                                                              |
| `fallbackModels` | string[] | Ordered list of fallback models when the primary is unavailable                                                                                                                                                    |
| `permissionMode` | string   | `"read-only"`, `"workspace-write"`, or `"danger-full-access"`                                                                                                                                                      |
| `env`            | object   | Environment variables injected at startup. Shell exports take precedence.                                                                                                                                          |
| `providers`      | object   | Custom OpenAI-compatible endpoints: `baseUrl`, `apiKey` / `apiKeyEnv`, optional **`apiVersion`** (Azure), `defaultModel`, etc. (see [example](https://github.com/andeya/codineer/blob/main/settings.example.json)) |
| `oauth`          | object   | Custom OAuth config (clientId, authorizeUrl, tokenUrl, scopes, etc.)                                                                                                                                               |
| `credentials`    | object   | Credential chain config (defaultSource, autoDiscover, claudeCode)                                                                                                                                                  |
| `mcpServers`     | object   | MCP server definitions (stdio, sse, http, ws)                                                                                                                                                                      |
| `sandbox`        | object   | Sandbox security settings (enabled, filesystemMode, allowedMounts)                                                                                                                                                 |
| `enabledPlugins` | object   | Plugin enable/disable overrides (map of `name@marketplace` → boolean)                                                                                                                                              |
| `plugins`        | object   | Plugin management (externalDirectories, installRoot)                                                                                                                                                               |
| `hooks`          | object   | Shell commands for `PreToolUse` / `PostToolUse` hooks                                                                                                                                                              |

Inspect merged config at runtime: `/config`, `/config env`, `/config model`

### Environment variables

Set via shell export **or** the `"env"` section in settings.json (shell exports take precedence):

| Variable                   | Purpose                                                                                                    |
| -------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `ANTHROPIC_API_KEY`        | Claude API key                                                                                             |
| `ANTHROPIC_AUTH_TOKEN`     | Bearer token (alternative)                                                                                 |
| `XAI_API_KEY`              | xAI / Grok API key                                                                                         |
| `OPENAI_API_KEY`           | OpenAI API key                                                                                             |
| `OPENROUTER_API_KEY`       | OpenRouter API key                                                                                         |
| `GROQ_API_KEY`             | Groq Cloud API key                                                                                         |
| `GEMINI_API_KEY`           | Google Gemini API key ([free from AI Studio](https://aistudio.google.com/apikey))                          |
| `DASHSCOPE_API_KEY`        | Alibaba Cloud DashScope (OpenAI-compatible)                                                                |
| `OLLAMA_HOST`              | Ollama endpoint (e.g. `http://192.168.1.100:11434`)                                                        |
| `CODINEER_WORKSPACE_ROOT`  | Override workspace root                                                                                    |
| `CODINEER_CONFIG_HOME`     | Override global config dir (default `~/.codineer`); the global flat config moves to the parent of this dir |
| `CODINEER_PERMISSION_MODE` | Default permission mode                                                                                    |
| `NO_COLOR`                 | Disable ANSI colors                                                                                        |
| `CLICOLOR=0`               | Disable ANSI colors (alternative)                                                                          |

**Credential chain (per-provider, in priority order):**

| Provider           | Chain                                                                                                        |
| ------------------ | ------------------------------------------------------------------------------------------------------------ |
| Anthropic (Claude) | `ANTHROPIC_API_KEY` / `ANTHROPIC_AUTH_TOKEN` → Codineer OAuth (`codineer login`) → Claude Code auto-discover |
| xAI (Grok)         | `XAI_API_KEY`                                                                                                |
| OpenAI             | `OPENAI_API_KEY`                                                                                             |
| Custom providers   | inline `apiKey` → `apiKeyEnv` env var                                                                        |

**Claude Code auto-discovery:** If you have Claude Code installed and logged in (`claude login`), Codineer automatically discovers your Claude Code credentials from `~/.claude/.credentials.json` (or macOS Keychain). This means you can use Codineer with your existing Claude subscription — no separate API key needed.

Configure in `settings.json`:

```json
{ "credentials": { "autoDiscover": true, "claudeCode": { "enabled": true } } }
```

Check auth status: `codineer status` or `codineer status anthropic`

**Configuration management:**

```bash
codineer config set model sonnet               # Set a config value
codineer config get model                      # Read a config value
codineer config list                           # Show all settings
```

---

## Project Context

`CODINEER.md` is the project memory file. It tells the AI about your codebase, conventions, and workflows.

```bash
codineer init        # auto-generate from detected stack
```

Codineer walks up the directory tree and loads all matching instruction files:

| File                        | Purpose                             |
| --------------------------- | ----------------------------------- |
| `CODINEER.md`               | Primary context (commit this)       |
| `CODINEER.local.md`         | Personal overrides (gitignore this) |
| `.codineer/CODINEER.md`     | Alternative location                |
| `.codineer/instructions.md` | Additional instructions             |

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

Plugins extend Codineer with custom **tools** (AI-callable functions), **slash commands**, **hooks** (pre/post tool-call interception), and **lifecycle scripts**. A plugin is a directory with a `plugin.json` manifest:

```
.codineer/plugins/my-plugin/
├── plugin.json              ← manifest (tools, commands, hooks, lifecycle)
├── tools/query-db.sh        ← AI calls this tool automatically
├── hooks/audit.sh           ← runs before/after every tool call
└── commands/deploy.sh       ← user types /deploy
```

Manage plugins from the REPL:

```bash
/plugin list                        # list all plugins with status
/plugin install ./path/to/plugin    # install from local path or Git URL
/plugin enable my-plugin            # enable
/plugin disable my-plugin           # disable
```

> **Full plugin development guide:** [`crates/plugins/README.md`](crates/plugins/README.md)

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

| Tool               | Description                           |
| ------------------ | ------------------------------------- |
| `bash`             | Execute shell commands                |
| `PowerShell`       | Execute PowerShell commands (Windows) |
| `read_file`        | Read file contents                    |
| `write_file`       | Create or overwrite files             |
| `edit_file`        | Targeted string replacement           |
| `glob_search`      | Find files by glob pattern            |
| `grep_search`      | Search file contents with regex       |
| `WebFetch`         | Fetch and summarize web pages         |
| `WebSearch`        | Web search via DuckDuckGo             |
| `NotebookEdit`     | Edit Jupyter notebook cells           |
| `TodoWrite`        | Manage structured task lists          |
| `Agent`            | Launch sub-agents                     |
| `Skill`            | Execute skill prompts                 |
| `REPL`             | Run code in Python, Node, or shell    |
| `ToolSearch`       | Search available tools                |
| `Sleep`            | Pause execution for a duration        |
| `SendUserMessage`  | Send a message to the user            |
| `Config`           | Read/write config values              |
| `StructuredOutput` | Return structured JSON                |

### Crate structure

All crates are published to crates.io. Install `codineer-cli` — the others are internal dependencies.

| Crate               | Role                          |
| ------------------- | ----------------------------- |
| `codineer-cli`      | CLI binary (**install this**) |
| `codineer-runtime`  | Core runtime engine           |
| `codineer-api`      | AI provider API clients       |
| `codineer-tools`    | Tool definitions & execution  |
| `codineer-plugins`  | Plugin system and hooks       |
| `codineer-commands` | Slash commands                |
| `codineer-lsp`      | LSP client integration        |

---

## License

[MIT](LICENSE)

---

<p align="center">
  Made with 🦀 by <a href="https://github.com/andeya">andeya</a>
</p>
