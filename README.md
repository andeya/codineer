<p align="center">
  <img src="https://raw.githubusercontent.com/andeya/aineer/main/docs/images/logo.svg" width="96" alt="">
</p>
<h1 align="center">aineer</h1>
<p align="center">
  <em>The Agentic Development Environment — where Shell, AI, and Agent merge into one.</em>
</p>

<p align="center">
  <a href="https://github.com/andeya/aineer/actions"><img src="https://github.com/andeya/aineer/workflows/CI/badge.svg" alt="CI"></a>
  <a href="https://github.com/andeya/aineer/releases"><img src="https://img.shields.io/github/v/release/andeya/aineer" alt="Release"></a>
  <a href="https://crates.io/crates/aineer"><img src="https://img.shields.io/crates/v/aineer.svg" alt="crates.io"></a>
  <img src="https://raw.githubusercontent.com/andeya/aineer/main/docs/images/badge-platforms.svg" alt="macOS | Linux | Windows">
  <br>
  <a href="README_CN.md">中文文档</a>
</p>

---

**Aineer** is an **ADE (Agentic Development Environment)** — Shell commands, AI chat, and autonomous agent actions weave together in a single unified stream. It reads your workspace, understands project context, and helps you write, refactor, debug, and ship code.

Built in safe Rust with **Tauri 2 + React 19 + Radix UI + Tailwind CSS + xterm.js**. Desktop GUI by default, `--cli` for classic terminal REPL. No daemon, no runtime dependency — bring any model and go.

> **Current status:** The **CLI REPL** (`aineer --cli` or `aineer`) is the **mature, full-featured interface** with 40+ built-in tools, streaming tool execution, multi-provider AI, plugins, MCP, and more. The **Desktop GUI** provides settings, theme switching, integrated terminal (xterm.js), model selection, and cache management; desktop AI chat is under active development.

<p align="center">
  <img src="https://raw.githubusercontent.com/andeya/aineer/main/docs/images/ScreenShot_01.png" alt="Aineer REPL screenshot" width="780">
</p>

## Why Aineer?

Most AI coding CLIs lock you into a single provider. Claude Code requires Anthropic. Codex CLI requires OpenAI. **Aineer works with all of them — and local models too.**

|                                                                                                         |      Aineer      |  Claude Code   |    Codex CLI    |    Aider     |
| ------------------------------------------------------------------------------------------------------- | :--------------: | :------------: | :-------------: | :----------: |
| **Multi-provider** (Anthropic, OpenAI, xAI, Ollama, …)                                                  | **All built-in** | Anthropic only | OpenAI + Ollama |     Yes      |
| **Zero-token-cost** ([free access to major models](#token-free-gateway-free-access-to-major-ai-models)) |     **Yes**      |       No       |       No        |      No      |
| **Zero-config local AI** (auto-detect Ollama)                                                           |     **Yes**      |       No       |  `--oss` flag   | Manual setup |
| **Single app** (no runtime deps)                                                                        |  **Rust+Tauri**  |    Node.js     |     Node.js     |    Python    |
| **Multimodal input** (`@image.png`, clipboard paste)                                                    |     **Yes**      |      Yes       |     Limited     |   Limited    |
| **MCP protocol** (external tool integration)                                                            |     **Yes**      |      Yes       |       Yes       |     Yes      |
| **Plugin system** + agents + skills                                                                     |     **Yes**      |      Yes       |       No        |      No      |
| **Permission modes** (read-only → full access)                                                          |     **Yes**      |      Yes       |       Yes       |   Partial    |
| **Tool-use fallback** (graceful degradation)                                                            |     **Yes**      |      N/A       |       N/A       |     N/A      |
| **Git workflow** (/commit, /pr, /diff, /branch)                                                         |   **Built-in**   |   Via tools    |    Via tools    | Auto-commit  |
| **Vim mode** in REPL                                                                                    |     **Yes**      |       No       |       No        |      No      |
| **CI/CD ready** (JSON output, tool allowlists)                                                          |     **Yes**      |      Yes       |       Yes       |   Limited    |
| **Context caching** (Gemini cachedContents, Anthropic prompt cache)                                     |     **Yes**      | Anthropic only |       No        |      No      |
| **Streaming tool executor** (parallel tools, sibling abort, progress events)                            |     **Yes**      |      Yes       |       No        |      No      |
| **Permission rules** (glob-based allow/deny matrix per tool)                                            |     **Yes**      |      Yes       |   Allowlists    |      No      |

**Key advantages:**

- **Provider freedom** — switch between Claude, GPT, Grok, Ollama, LM Studio, OpenRouter, Groq, or any OpenAI-compatible API with `--model`. No vendor lock-in.
- **Zero token cost** — pair with [Token Free Gateway](https://github.com/andeya/token-free-gateway) to use Claude, ChatGPT, Gemini, DeepSeek, and 10+ more models for free — no API key purchase needed.
- **Zero-config local AI** — start Ollama, run `aineer`. Auto-detects local models and picks the best one.
- **Instant setup** — `brew install` or `cargo install`. One Rust binary, no runtime dependencies.
- **Multimodal input** — attach images via `@photo.png`, paste from clipboard (`Ctrl+V` / `/image`). Works with Anthropic, OpenAI, and any multimodal-capable provider.
- **Graceful degradation** — models without function calling automatically fall back to text-only mode.
- **Project memory** — `.aineer/AINEER.md` gives the AI persistent context about your codebase. Commit it to share with your team.
- **Adaptive terminal UI** — welcome panel and separator line reflow in real time as the window resizes. Ultra-narrow terminals collapse to a single-column layout; the input line redraws in place without flicker.
- **Smart context caching** — Gemini's `cachedContents` API and Anthropic's prompt cache are managed automatically, reducing latency and token costs for long sessions.
- **Streaming tool execution** — tools start as soon as their parameters arrive, with parallel execution for safe tools, automatic sibling abort on bash failure, and real-time progress events.
- **Fine-grained permission rules** — glob-pattern `always-allow`/`always-deny`/`always-ask` rules per tool and input, beyond the three permission modes.

## Table of Contents

- [Install](#install)
- [Quick Start](#quick-start)
- [Models & Providers](#models--providers)
- [Usage](#usage)
  - [File & Image Attachments](#file--image-attachments)
- [Configuration](#configuration)
- [Project Context](#project-context)
- [Extending Aineer](#extending-aineer)
- [Sessions & Resume](#sessions--resume)
- [Self-Update](#self-update)
- [Troubleshooting](#troubleshooting)
- [Reference](#reference)
- [Roadmap](#roadmap)
- [License](#license)

---

## Install

**Desktop app (recommended):**

Download the latest installer from [Releases](https://github.com/andeya/aineer/releases):

| Platform              | File                               |
| --------------------- | ---------------------------------- |
| macOS (Apple Silicon) | `Aineer_*_aarch64.dmg`             |
| macOS (Intel)         | `Aineer_*_x64.dmg`                 |
| Linux (x86_64)        | `aineer_*_amd64.deb` / `.AppImage` |
| Linux (ARM64)         | `aineer_*_arm64.deb`               |
| Windows (x86_64)      | `Aineer_*_x64-setup.exe`           |

**CLI-only mode:**

```bash
brew install andeya/aineer/aineer            # Homebrew (macOS / Linux)
cargo install aineer                           # Cargo (from crates.io)
```

<details><summary>Build from source</summary>

```bash
git clone https://github.com/andeya/aineer.git
cd aineer
bun install                                    # Install frontend dependencies
cargo tauri build                              # Build Tauri desktop app (GUI + CLI)
# Or for CLI-only:
cargo install --path app --locked
```

**Prerequisites:** Rust toolchain, [Bun](https://bun.sh), and platform-specific [Tauri dependencies](https://v2.tauri.app/start/prerequisites/).

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
# Or use Token Free Gateway for free access to all major models (see below)
aineer login                                    # Or OAuth login (default provider)
aineer login anthropic --source claude-code     # Use Claude Code credentials
aineer status                                   # Check authentication status
aineer config set model sonnet                   # Set default model (alias or full name)

# 2. Initialize project context (optional)
aineer init

# 3. Start coding
aineer                                          # Interactive REPL
aineer "explain this project"                   # One-shot prompt
```

Aineer auto-detects your provider. No extra flags needed. All credentials can also go in [settings.json](#configuration) instead of shell exports.

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
aineer --model sonnet "review my changes"
aineer --model flash "quick question"
```

No aliases are built in — you decide what short names make sense for your workflow. View your configured aliases: `aineer models`.

### Custom providers (OpenAI-compatible)

Use any OpenAI-compatible API with the `provider/model` syntax:

| Prefix               | Provider   | API key              |
| -------------------- | ---------- | -------------------- |
| `ollama/<model>`     | Ollama     | —                    |
| `lmstudio/<model>`   | LM Studio  | —                    |
| `groq/<model>`       | Groq Cloud | `GROQ_API_KEY`       |
| `openrouter/<model>` | OpenRouter | `OPENROUTER_API_KEY` |

```bash
aineer --model ollama/qwen3-coder "refactor this module"
aineer --model groq/llama-3.3-70b-versatile "explain this"
aineer --model ollama              # auto-pick best coding model
```

**Zero-config Ollama**: when no API keys are found and Ollama is running, Aineer auto-detects it and picks the best coding model. Supports `OLLAMA_HOST` env var and remote instances (see [Configuration](#environment-variables)).

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
aineer --model gemini/gemini-2.5-flash "explain this code"
aineer --model gemini/gemini-2.5-pro "review my architecture"
```

> **Important:** Use the OpenAI-compatible endpoint (`/v1beta/openai`), not the native Gemini REST API (`/v1beta/models/...:generateContent`). Aineer appends `/chat/completions` to the base URL.

### Token Free Gateway (Free Access to Major AI Models)

> **Zero API token cost** — log in via browser once, then call Claude, ChatGPT, Gemini, DeepSeek, Qwen, Kimi, Grok, and more through a single unified gateway — completely free.

[Token Free Gateway](https://github.com/andeya/token-free-gateway) is an AI gateway that drives official model web UIs (browser login) instead of paid API keys. If you can use a model in the browser, you can call it through Aineer — no API token purchase required.

**Highlights:**

| Traditional approach | Token Free Gateway way   |
| -------------------- | ------------------------ |
| Buy API tokens       | **Completely free**      |
| Pay per request      | No enforced quota        |
| Credit card required | Browser login only       |
| API tokens may leak  | Credentials stored local |

**Supported models:** Claude Web, ChatGPT Web, Gemini Web, DeepSeek Web, Qwen Web (intl/cn), Kimi, Doubao, Grok Web, GLM Web, Xiaomi MiMo, and more. 11 out of 13 web models support tool calling.

**Setup:**

1. Deploy and start the [Token Free Gateway](https://github.com/andeya/token-free-gateway) (default port 3456)
2. Add the provider to `settings.json`:

```json
{
  "model": "token-free-gateway/claude-sonnet-4-6",
  "env": {
    "TFG_API_KEY": "your-gateway-token"
  },
  "providers": {
    "token-free-gateway": {
      "baseUrl": "http://127.0.0.1:3456/v1",
      "apiKeyEnv": "TFG_API_KEY",
      "defaultModel": "claude-opus-4-6"
    }
  }
}
```

3. Start coding:

```bash
aineer --model token-free-gateway/claude-sonnet-4-6 "refactor this module"
aineer --model token-free-gateway/claude-opus-4-6 "code review"
aineer --model token-free-gateway/deepseek-chat "explain this code"
```

You can also combine it with model aliases and fallback models for a seamless experience:

```json
{
  "model": "sonnet",
  "modelAliases": {
    "opus": "token-free-gateway/claude-opus-4-6",
    "sonnet": "token-free-gateway/claude-sonnet-4-6"
  },
  "fallbackModels": ["sonnet", "flash", "qwen-plus"]
}
```

### Alibaba Cloud DashScope (OpenAI-compatible)

Use `provider/model` and configure `providers` in `settings.json` with the correct `baseUrl` (see [Model Studio docs](https://www.alibabacloud.com/help/en/model-studio/)):

```bash
export DASHSCOPE_API_KEY="sk-..."
aineer --model dashscope/qwen-plus-2025-07-28 "one-line Rust ownership"
```

Streaming deltas may use `reasoning_content`, `thought`, or array-shaped `content`; Aineer normalizes these. If you still see **assistant stream produced no content**, the CLI automatically retries **once** with a non-streaming request. Use a current build from source or the latest release.

### Azure OpenAI

Set optional `apiVersion` (e.g. `2024-02-15-preview`) on a `providers.<name>` entry; Aineer appends `api-version=…` to the `chat/completions` URL. See [`settings.example.json`](https://github.com/andeya/aineer/blob/main/settings.example.json) (`azure-openai`).

### List available models

```bash
aineer models               # All providers
aineer models anthropic     # Filter by provider
aineer models ollama        # Show local Ollama models
```

### Model resolution order

When no `--model` flag is given:

1. `model` field in settings.json
2. Auto-detect from available API credentials
3. Auto-detect running Ollama instance

If the resolved model lacks credentials, Aineer tries each entry in `fallbackModels` before giving up.

Switch model mid-session: `/model <name>`

### Fallback models

Set an ordered list of fallback models in `settings.json`. If the primary model is unavailable (e.g. missing API key), Aineer tries each fallback in order:

```json
{
  "model": "sonnet",
  "modelAliases": { "sonnet": "claude-sonnet-4-6" },
  "fallbackModels": ["ollama/qwen3-coder", "groq/llama-3.3-70b-versatile"]
}
```

`model` and `fallbackModels` both support your custom aliases from `modelAliases`. This is especially useful for zero-cost setups: set a cloud model as primary and local models as fallback.

---

## Usage

### Interactive REPL

```bash
aineer
```

A **framed welcome banner** shows workspace, directory, model, session, and a copy-paste `aineer --resume …` line. The banner and separator adapt to the current terminal width in real time — narrow terminals switch to a single-column layout automatically. The prompt is **`❯`**. Type naturally. Use **slash commands** (Tab-autocomplete supported):

| Category        | Commands                                                                 |
| --------------- | ------------------------------------------------------------------------ |
| **Info**        | `/help` `/status` `/version` `/model` `/cost` `/config` `/memory`        |
| **Session**     | `/compact` `/clear` `/session` `/resume` `/export`                       |
| **Git**         | `/diff` `/branch` `/commit` `/commit-push-pr` `/pr` `/issue` `/worktree` |
| **Agents**      | `/agents` `/skills` `/plugin`                                            |
| **Advanced**    | `/ultraplan` `/bughunter` `/teleport` `/debug-tool-call` `/vim`          |
| **Diagnostics** | `/doctor`                                                                |
| **Update**      | `/update [check\|apply\|dismiss\|status]`                                |
| **Navigation**  | `/init` `/permissions` `/exit`                                           |

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
aineer "explain this project's architecture"
aineer -p "list all TODO comments" --output-format json
aineer --model sonnet --permission-mode read-only "audit the codebase"
```

| Flag                                      | Description                                                  |
| ----------------------------------------- | ------------------------------------------------------------ |
| `-p <text>`                               | One-shot prompt                                              |
| `--model <name>`                          | Choose model                                                 |
| `--output-format text\|json\|stream-json` | Output format (`stream-json` emits newline-delimited events) |
| `--allowedTools <list>`                   | Restrict tool access (comma-separated)                       |
| `--permission-mode <mode>`                | `read-only`, `workspace-write`, `danger-full-access`         |
| `--resume <file>`                         | Resume a saved session                                       |
| `-V`, `--version`                         | Print version                                                |

### Permission modes

| Mode                 | Allows                                        |
| -------------------- | --------------------------------------------- |
| `read-only`          | Read and search only — no writes              |
| `workspace-write`    | Edit files inside the workspace (default)     |
| `danger-full-access` | Unrestricted access including system commands |

### Scripting & CI

```bash
aineer -p "check for security issues" \
  --permission-mode read-only \
  --allowedTools read_file,grep_search \
  --output-format json | jq '.content[0].text'
```

---

## Configuration

### Settings files

Aineer merges JSON settings from multiple files (highest to lowest precedence):

| File                          | Actual path                                  | Scope                     | Committed?      |
| ----------------------------- | -------------------------------------------- | ------------------------- | --------------- |
| `.aineer/settings.local.json` | `<project root>/.aineer/settings.local.json` | Project — local overrides | No (gitignored) |
| `.aineer/settings.json`       | `<project root>/.aineer/settings.json`       | Project settings          | Yes             |
| `~/.aineer/settings.json`     | `$HOME/.aineer/settings.json`                | User — global settings    | —               |

All files use the same schema. `env`, `providers`, and `mcpServers` objects are deep-merged across layers. For `mcpServers`, a server name defined in a later file replaces (not deep-merges) the earlier definition. `aineer config set` writes to the global file (`~/.aineer/settings.json`).

### Settings reference

> **Full example with all fields:** [`settings.example.json`](https://github.com/andeya/aineer/blob/main/settings.example.json)

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

| Key               | Type     | Description                                                                                                                                                                                                      |
| ----------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `model`           | string   | Default model — alias or full name (e.g. `"sonnet"`, `"claude-sonnet-4-6"`, `"ollama/qwen3-coder"`)                                                                                                              |
| `modelAliases`    | object   | Custom short names mapping to full model IDs (e.g. `{"sonnet": "claude-sonnet-4-6"}`)                                                                                                                            |
| `fallbackModels`  | string[] | Ordered list of fallback models when the primary is unavailable                                                                                                                                                  |
| `permissionMode`  | string   | `"read-only"`, `"workspace-write"`, or `"danger-full-access"`                                                                                                                                                    |
| `env`             | object   | Environment variables injected at startup. Shell exports take precedence.                                                                                                                                        |
| `providers`       | object   | Custom OpenAI-compatible endpoints: `baseUrl`, `apiKey` / `apiKeyEnv`, optional **`apiVersion`** (Azure), `defaultModel`, etc. (see [example](https://github.com/andeya/aineer/blob/main/settings.example.json)) |
| `oauth`           | object   | Custom OAuth config (clientId, authorizeUrl, tokenUrl, scopes, etc.)                                                                                                                                             |
| `credentials`     | object   | Credential chain config (defaultSource, autoDiscover, claudeCode)                                                                                                                                                |
| `mcpServers`      | object   | MCP server definitions (stdio, sse, http, ws)                                                                                                                                                                    |
| `sandbox`         | object   | Sandbox security settings (enabled, filesystemMode, allowedMounts)                                                                                                                                               |
| `enabledPlugins`  | object   | Plugin enable/disable overrides (map of `name@marketplace` → boolean)                                                                                                                                            |
| `plugins`         | object   | Plugin management (externalDirectories, installRoot)                                                                                                                                                             |
| `hooks`           | object   | Shell commands for `PreToolUse` / `PostToolUse` hooks                                                                                                                                                            |
| `geminiCache`     | object   | Gemini context caching: `{ "enabled": true, "ttlSeconds": 3600 }` — caches system prompt + tools via Google's cachedContents API                                                                                 |
| `permissionRules` | array    | Fine-grained tool permission rules: `[{ "tool": "bash", "input": "rm *", "decision": "always-deny" }]`                                                                                                           |

Inspect merged config at runtime: `/config`, `/config env`, `/config model`

### Environment variables

Set via shell export **or** the `"env"` section in settings.json (shell exports take precedence):

| Variable                 | Purpose                                                                                       |
| ------------------------ | --------------------------------------------------------------------------------------------- |
| `ANTHROPIC_API_KEY`      | Claude API key                                                                                |
| `ANTHROPIC_AUTH_TOKEN`   | Bearer token (alternative)                                                                    |
| `XAI_API_KEY`            | xAI / Grok API key                                                                            |
| `OPENAI_API_KEY`         | OpenAI API key                                                                                |
| `OPENROUTER_API_KEY`     | OpenRouter API key                                                                            |
| `GROQ_API_KEY`           | Groq Cloud API key                                                                            |
| `GEMINI_API_KEY`         | Google Gemini API key ([free from AI Studio](https://aistudio.google.com/apikey))             |
| `DASHSCOPE_API_KEY`      | Alibaba Cloud DashScope (OpenAI-compatible)                                                   |
| `OLLAMA_HOST`            | Ollama endpoint (e.g. `http://192.168.1.100:11434`)                                           |
| `AINEER_WORKSPACE_ROOT`  | Override workspace root                                                                       |
| `AINEER_CONFIG_HOME`     | Override global config dir (default `~/.aineer`); `settings.json` is read from this directory |
| `AINEER_PERMISSION_MODE` | Default permission mode                                                                       |
| `NO_COLOR`               | Disable ANSI colors                                                                           |
| `CLICOLOR=0`             | Disable ANSI colors (alternative)                                                             |

**Credential chain (per-provider, in priority order):**

| Provider           | Chain                                                                                                    |
| ------------------ | -------------------------------------------------------------------------------------------------------- |
| Anthropic (Claude) | `ANTHROPIC_API_KEY` / `ANTHROPIC_AUTH_TOKEN` → Aineer OAuth (`aineer login`) → Claude Code auto-discover |
| xAI (Grok)         | `XAI_API_KEY`                                                                                            |
| OpenAI             | `OPENAI_API_KEY`                                                                                         |
| Custom providers   | inline `apiKey` → `apiKeyEnv` env var                                                                    |

**Claude Code auto-discovery:** If you have Claude Code installed and logged in (`claude login`), Aineer automatically discovers your Claude Code credentials from `~/.claude/.credentials.json` (or macOS Keychain). This means you can use Aineer with your existing Claude subscription — no separate API key needed.

Configure in `settings.json`:

```json
{ "credentials": { "autoDiscover": true, "claudeCode": { "enabled": true } } }
```

Check auth status: `aineer status` or `aineer status anthropic`

**Configuration management:**

```bash
aineer config set model sonnet               # Set a config value
aineer config get model                      # Read a config value
aineer config list                           # Show all settings
```

---

## Project Context

`.aineer/AINEER.md` is the **project memory file** — it is injected into every conversation's system prompt so the AI understands your codebase without re-asking each time. Typical content includes:

- Tech stack and languages used
- Build, lint, and test commands
- Coding conventions (naming, error handling, commit style)
- Repository layout notes

```bash
aineer init        # auto-generate from detected stack
```

`aineer init` scaffolds the `.aineer/` directory and auto-detects your stack (Rust/Python/Node/etc.) to generate a starter `AINEER.md`. Edit it freely and commit it to the repo so the whole team benefits.

Aineer walks up the directory tree and loads all matching instruction files:

| File                      | Purpose                             |
| ------------------------- | ----------------------------------- |
| `.aineer/AINEER.md`       | Primary context (commit this)       |
| `AINEER.md`               | Legacy location (also supported)    |
| `AINEER.local.md`         | Personal overrides (gitignore this) |
| `.aineer/instructions.md` | Additional instructions             |

Files are loaded from every ancestor directory, deduplicated, and concatenated into the system prompt. This means monorepo sub-projects can have their own `AINEER.md` that augments the root one.

---

## Extending Aineer

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

Plugins extend Aineer with custom **tools** (AI-callable functions), **slash commands**, **hooks** (pre/post tool-call interception), and **lifecycle scripts**. A plugin is a directory with a `plugin.json` manifest:

```
.aineer/plugins/my-plugin/
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
aineer agents          # list agents
aineer skills          # list skills
/agents                  # inside REPL
/skills                  # inside REPL
```

Skills are discovered from the project's `.aineer/skills/` (or `~/.aineer/skills/` when no project is initialized), then `$AINEER_CONFIG_HOME/skills/` and `~/.aineer/skills/`.

---

## Sessions & Resume

Every conversation is saved automatically. You can resume any session later — even across reboots.

```bash
aineer --resume /path/to/session.jsonl     # Resume from CLI
```

Inside the REPL:

| Command          | Action                                        |
| ---------------- | --------------------------------------------- |
| `/session`       | Show current session path                     |
| `/resume <path>` | Load and resume a previous session            |
| `/export [path]` | Export the session transcript                 |
| `/compact`       | Summarize and compress context to free tokens |
| `/clear`         | Clear conversation history                    |

The welcome banner includes a copy-paste `aineer --resume …` line so you can always return to a session.

---

## Self-Update

Aineer can update itself. A background check runs periodically (every 24h by default) and shows a notification when a new version is available.

```bash
aineer update                   # Check for updates and auto-install
```

Inside the REPL:

| Command           | Action                                                        |
| ----------------- | ------------------------------------------------------------- |
| `/update`         | Check for new versions                                        |
| `/update apply`   | Download and install the latest version                       |
| `/update dismiss` | Suppress notification for the current version                 |
| `/update status`  | Show current version, last check time, and dismissed versions |

The updater downloads the correct prebuilt binary for your platform (macOS, Linux, Windows) and replaces the current executable atomically with a backup. If no prebuilt binary is available for your platform, it shows manual install instructions.

---

## Troubleshooting

<details><summary><strong>No API key / authentication errors</strong></summary>

```bash
aineer status                         # Check which credentials are detected
aineer status anthropic               # Check a specific provider
aineer login                          # OAuth login
aineer login anthropic --source claude-code   # Reuse Claude Code credentials
```

Set API keys via shell exports or `settings.json` → `"env"`. See [Environment variables](#environment-variables).

</details>

<details><summary><strong>Model not found / unsupported model</strong></summary>

```bash
aineer models                         # List all available models
aineer models ollama                  # Check Ollama models
aineer --model ollama/qwen3-coder "test"   # Use explicit provider/model
```

For custom providers, make sure `baseUrl` uses the OpenAI-compatible endpoint (e.g. `/v1` or `/v1beta/openai` for Gemini).

</details>

<details><summary><strong>"assistant stream produced no content"</strong></summary>

Some providers (DashScope, certain OpenRouter models) send responses in non-standard formats. Aineer normalizes `reasoning_content`, `thought`, and array-shaped `content`. If you still see this error, the CLI automatically retries once with a non-streaming request. Ensure you are on the latest version: `aineer update`.

</details>

<details><summary><strong>Permission denied when editing files</strong></summary>

By default, Aineer runs in `workspace-write` mode and asks before writing outside the workspace. Change mode with:

```bash
aineer --permission-mode danger-full-access    # Unrestricted
aineer --permission-mode read-only             # No writes at all
```

Or set permanently in `settings.json`: `"permissionMode": "danger-full-access"`

</details>

<details><summary><strong>Ollama not detected</strong></summary>

- Ensure Ollama is running: `ollama serve`
- Check the endpoint: `curl http://localhost:11434/v1/models`
- For remote Ollama: `export OLLAMA_HOST=http://your-server:11434`

</details>

<details><summary><strong>Images not working (Ctrl+V)</strong></summary>

- **macOS**: Use `Ctrl+V` (not `Cmd+V`). Terminal.app/iTerm2 intercept `Cmd+V`.
- **Windows**: Use `/image` instead (Windows Terminal intercepts `Ctrl+V` for text paste).
- **Linux**: Works in most terminals. Try `/image` if `Ctrl+V` fails.
- Fallback: `@photo.png` to attach an image file directly.

</details>

<details><summary><strong>Context overflow / conversation too long</strong></summary>

Aineer automatically compacts context when it approaches the model's limit. You can also manually trigger compaction:

```
/compact                # Summarize and compress context
/clear                  # Start fresh
```

Configure `geminiCache` in settings for intelligent context caching with Gemini models.

</details>

---

## Reference

### Built-in tools

| Category                           | Tool               | Description                                                                                                            |
| ---------------------------------- | ------------------ | ---------------------------------------------------------------------------------------------------------------------- |
| **File I/O**                       | `read_file`        | Read file contents (text, PDF text extraction, image base64)                                                           |
|                                    | `write_file`       | Create or overwrite files (atomic writes, mtime tracking)                                                              |
|                                    | `edit_file`        | Targeted string replacement with conflict detection                                                                    |
|                                    | `glob_search`      | Find files by glob pattern (.gitignore-aware)                                                                          |
|                                    | `grep_search`      | Search file contents with regex (ripgrep-powered)                                                                      |
| **Shell**                          | `bash`             | Execute shell commands with timeout and background support                                                             |
|                                    | `PowerShell`       | Execute PowerShell commands (Windows / cross-platform)                                                                 |
|                                    | `REPL`             | Run code in Python, Node, or shell                                                                                     |
| **Web**                            | `WebFetch`         | Fetch and summarize web pages                                                                                          |
|                                    | `WebSearch`        | Web search via DuckDuckGo                                                                                              |
| **Notebook**                       | `NotebookEdit`     | Edit Jupyter notebook cells                                                                                            |
| **Agent**                          | `Agent`            | Launch sub-agents for parallel tasks                                                                                   |
|                                    | `SendUserMessage`  | Send a message to the user                                                                                             |
| **LSP**                            | `Lsp`              | Language server operations (hover, completion, go-to-definition, references, symbols, rename, formatting, diagnostics) |
| **Task management**                | `TaskCreate`       | Create a background task with optional command                                                                         |
|                                    | `TaskGet`          | Get task status and output                                                                                             |
|                                    | `TaskList`         | List all tasks                                                                                                         |
|                                    | `TaskUpdate`       | Update task title, description, or status                                                                              |
|                                    | `TaskStop`         | Stop a running task                                                                                                    |
| **Plan mode**                      | `EnterPlanMode`    | Enter read-only planning mode                                                                                          |
|                                    | `ExitPlanMode`     | Exit planning mode                                                                                                     |
| **Git worktree**                   | `EnterWorktree`    | Create and enter an isolated git worktree                                                                              |
|                                    | `ExitWorktree`     | Exit and optionally clean up worktree                                                                                  |
| **Cron**                           | `CronCreate`       | Create a managed cron job                                                                                              |
|                                    | `CronDelete`       | Delete a managed cron job                                                                                              |
|                                    | `CronList`         | List managed cron jobs                                                                                                 |
| **MCP resources**                  | `ListMcpResources` | List available MCP resources                                                                                           |
|                                    | `ReadMcpResource`  | Read an MCP resource by URI                                                                                            |
|                                    | `MCPSearch`        | Full-text search across MCP resources                                                                                  |
| **Collaboration** _(experimental)_ | `TeamCreate`       | Create a named agent team                                                                                              |
|                                    | `TeamDelete`       | Delete an agent team                                                                                                   |
|                                    | `SendMessage`      | Send a message to an agent or team                                                                                     |
|                                    | `SlashCommand`     | Invoke a registered slash command                                                                                      |
| **Misc**                           | `TodoWrite`        | Manage structured task lists                                                                                           |
|                                    | `Skill`            | Execute skill prompts                                                                                                  |
|                                    | `ToolSearch`       | Search available tools                                                                                                 |
|                                    | `Config`           | Read/write config values                                                                                               |
|                                    | `StructuredOutput` | Return structured JSON                                                                                                 |
|                                    | `Sleep`            | Pause execution for a duration                                                                                         |

### Crate structure

The `aineer` crate (in `app/`) is the Tauri 2 desktop application that bundles a GUI and CLI mode. All other crates are internal dependencies.

| Crate                    | Role                                                                                           |
| ------------------------ | ---------------------------------------------------------------------------------------------- |
| `aineer`                 | Tauri 2 desktop app — GUI (default) or CLI (`--cli`) mode, includes PTY manager (portable-pty) |
| `aineer-cli`             | CLI mode library (embedded in `aineer`, provides REPL)                                         |
| `aineer-protocol`        | Shared types, events, credentials                                                              |
| `aineer-api`             | AI provider API clients                                                                        |
| `aineer-provider`        | Provider registry (multi-provider management)                                                  |
| `aineer-engine`          | Agent engine (conversation, planning, execution)                                               |
| `aineer-gateway`         | Embedded OpenAI-compatible model gateway                                                       |
| `aineer-settings`        | Unified settings system (3-layer merge: user/project/local)                                    |
| `aineer-memory`          | Memory system (project knowledge persistence)                                                  |
| `aineer-mcp`             | MCP protocol client & transport                                                                |
| `aineer-tools`           | Tool definitions & execution                                                                   |
| `aineer-plugins`         | Plugin system and hooks                                                                        |
| `aineer-lsp`             | LSP client integration                                                                         |
| `aineer-channels`        | Multi-channel adapter trait _(planned)_                                                        |
| `aineer-auto-update`     | Self-update mechanism                                                                          |
| `aineer-release-channel` | Release channel management (dev/nightly/preview/stable)                                        |

---

## Roadmap

Aineer is under active development. The table below distinguishes what works today from what is planned.

| Area                                                       | Status           | Notes                                                               |
| ---------------------------------------------------------- | ---------------- | ------------------------------------------------------------------- |
| CLI REPL + all slash commands                              | **Stable**       | Full-featured interactive experience                                |
| 40+ built-in tools                                         | **Stable**       | File I/O, shell, web, LSP, agents, tasks, cron, worktree, MCP, etc. |
| Multi-provider AI (Anthropic, OpenAI, xAI, Ollama, custom) | **Stable**       | Credential chains, auto-detect, fallback                            |
| Streaming tool executor                                    | **Stable**       | Parallel execution, sibling abort, progress events                  |
| Permission system                                          | **Stable**       | 3 modes + fine-grained glob rules                                   |
| MCP protocol (stdio, sse, http, ws)                        | **Stable**       |                                                                     |
| Plugin system + hooks                                      | **Stable**       |                                                                     |
| Sessions & auto-resume                                     | **Stable**       |                                                                     |
| Self-update (CLI + desktop)                                | **Stable**       |                                                                     |
| Context caching (Gemini + Anthropic)                       | **Stable**       |                                                                     |
| Desktop GUI — settings, themes, terminal, cache            | **Stable**       | Full settings UI with CodeMirror JSON editor                        |
| Desktop GUI — model selection                              | **Stable**       | Status bar + settings page                                          |
| Desktop GUI — system tray                                  | **Stable**       | Minimize to tray on close                                           |
| Desktop GUI — AI chat                                      | **In progress**  | Shell + terminal work; AI chat provider integration WIP             |
| Collaboration tools (TeamCreate, SendMessage)              | **Experimental** | In-process registry                                                 |
| Multi-channel delivery (Lark, WeChat, WhatsApp bots)       | **Planned**      | Adapter trait defined, implementations not yet built                |
| LLM-summarized context compaction                          | **Available**    | Engine support exists; CLI `/compact` uses heuristic by default     |

---

## License

[MIT](LICENSE)

---

<p align="center">
  Made with 🦀 by <a href="https://github.com/andeya">andeya</a>
</p>
