<p align="center">
  <img src="assets/logo-light.svg" alt="Codineer" width="360">
  <br>
  <em>Your local AI coding agent — one binary, zero cloud lock-in.</em>
</p>

<p align="center">
  <a href="https://github.com/andeya/codineer/actions"><img src="https://github.com/andeya/codineer/workflows/CI/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
  <a href="https://github.com/andeya/codineer/releases"><img src="https://img.shields.io/github/v/release/andeya/codineer" alt="Release"></a>
  <br>
  <a href="README_CN.md">中文文档</a>
</p>

---

Codineer is a **local-first coding agent** that runs entirely in your terminal. It reads your workspace, understands your project, and helps you write, refactor, debug, and ship code — interactively or in one-shot mode.

Built in safe Rust. Ships as a single, self-contained binary. No daemon, no cloud dependency (bring your own API key).

## Why Codineer?

- **Private by design** — your code stays on your machine; only the prompts you send leave the terminal
- **Workspace-aware** — reads `CODINEER.md`, project configs, git state, and LSP diagnostics before every turn
- **Tool-rich** — shell execution, file read/write/edit, glob/grep search, web fetch, todo tracking, notebook editing, and more
- **Extensible** — MCP servers, local plugins, custom agents and skills via `.codineer/` directories
- **Sandboxed** — optional process isolation via Linux namespaces or macOS Seatbelt profiles
- **Multi-provider** — Anthropic (Claude), xAI (Grok), and any OpenAI-compatible API

## Quick Start

### Install

```bash
# From source
cargo install --path crates/codineer-cli --locked

# Or via Homebrew (macOS/Linux)
brew install andeya/tap/codineer

# Or download a prebuilt binary from GitHub Releases
```

### Authenticate

```bash
# Anthropic (Claude)
export ANTHROPIC_API_KEY="sk-ant-..."

# xAI (Grok)
export XAI_API_KEY="xai-..."

# OpenAI
export OPENAI_API_KEY="sk-..."

# Or use Anthropic OAuth:
codineer login
```

### Run

```bash
# Interactive REPL
codineer

# One-shot prompt
codineer prompt "explain the architecture of this project"

# JSON output for scripting
codineer -p "list all TODO items" --output-format json
```

## Core Features

| Feature | Description |
|---------|-------------|
| **Interactive REPL** | Conversational coding sessions with Vim keybindings, tab completion, and history |
| **Workspace Tools** | `bash`, `read_file`, `write_file`, `edit_file`, `glob`, `grep`, `web_fetch`, `web_search`, `todo_write`, `notebook_edit` |
| **Slash Commands** | `/status`, `/compact`, `/config`, `/cost`, `/model`, `/permissions`, `/resume`, `/clear`, `/init`, `/diff`, `/export` |
| **Agent & Skill System** | Discover and run agents/skills from `.codineer/agents/` and `.codineer/skills/` |
| **Plugin System** | Install, manage, and extend with custom plugins and hooks |
| **MCP Support** | Connect to external tool servers via Model Context Protocol (stdio, SSE, HTTP, WebSocket) |
| **Git Integration** | Branch detection, worktree management, commit/PR workflows |
| **Session Management** | Save, restore, and resume coding sessions |
| **Sandbox** | Process isolation with Linux `unshare` or macOS `sandbox-exec` |

## Configuration

Codineer loads configuration from multiple sources (in precedence order):

1. `.codineer/settings.local.json` — local overrides (gitignored)
2. `.codineer/settings.json` — project settings
3. `~/.codineer/settings.json` — user-global settings

Key settings: `model`, `permissionMode`, `mcpServers`, `sandbox`, `hooks`, `enabledPlugins`.

See `codineer help` for full documentation of environment variables and configuration files.

## Project Structure

```text
crates/
├── api/              # AI provider clients + streaming
├── codineer-cli/     # Interactive CLI binary
├── commands/         # Slash commands & agent/skill discovery
├── lsp/              # Language Server Protocol client
├── plugins/          # Plugin system & hooks
├── runtime/          # Session, config, MCP, prompt, sandbox
└── tools/            # AI-callable tool definitions
```

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## License

[MIT](LICENSE)

---

<p align="center">
  Made with 🦀 by <a href="https://github.com/andeya">andeya</a>
</p>
