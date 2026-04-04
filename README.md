<p align="center">
  <img src="https://raw.githubusercontent.com/andeya/codineer/main/assets/logo.svg" width="96" alt="">
</p>
<h1 align="center">codineer</h1>
<p align="center">
  <em>Your local AI coding agent — one binary, zero cloud lock-in.</em>
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

Built in safe Rust. Ships as a **single binary**. No daemon, no cloud dependency — bring your own API key and go.

## Table of Contents

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
# Environment variable (any session)
export ANTHROPIC_API_KEY="sk-ant-..."   # Claude (recommended)
export XAI_API_KEY="xai-..."            # Grok
export OPENAI_API_KEY="sk-..."          # GPT / OpenAI-compatible

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

Flags available in prompt mode:

| Flag                         | Description                                                  |
| ---------------------------- | ------------------------------------------------------------ |
| `--model <name>`             | Choose model (see [Model Selection](#model-selection))       |
| `--output-format text\|json` | Output format (default: `text`)                              |
| `--allowedTools <list>`      | Comma-separated tool allowlist                               |
| `--permission-mode <mode>`   | Permission level (see [Permission Modes](#permission-modes)) |

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
| `gpt-4o`    | `gpt-4o`                      | OpenAI    |
| `o3`        | `o3`                          | OpenAI    |

```bash
codineer --model opus "review my changes"
codineer --model grok-mini "quick question"
```

Switch model mid-session with `/model <name>`.

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

Codineer loads settings from (highest to lowest precedence):

1. `.codineer/settings.local.json` — local overrides (gitignored)
2. `.codineer/settings.json` — project settings (commit this)
3. `.codineer.json` — project-level flat config
4. `~/.codineer/settings.json` — global user settings
5. `~/.codineer.json` — global flat config

Inspect the merged configuration at any time:

```bash
/config          # full merged config
/config env      # environment variables section
/config model    # model settings
/config plugins  # plugin configuration
/config hooks    # hook configuration
```

**Useful environment variables:**

| Variable                    | Purpose                              |
| --------------------------- | ------------------------------------ |
| `ANTHROPIC_API_KEY`         | Claude API key                       |
| `XAI_API_KEY`               | xAI / Grok API key                   |
| `OPENAI_API_KEY`            | OpenAI API key                       |
| `CODINEER_WORKSPACE_ROOT`   | Override workspace root path         |
| `CODINEER_CONFIG_HOME`      | Override config directory (`~/.codineer`) |
| `CODINEER_PERMISSION_MODE`  | Default permission mode              |
| `NO_COLOR`                  | Disable ANSI color output            |

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

Supported transport types: `stdio` (default), `sse`, `http`, `ws`.

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
