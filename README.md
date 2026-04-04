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

## Install

Choose the method that works best for you:

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

## Quick Start

**1. Set up your API key** (pick one):

```bash
export ANTHROPIC_API_KEY="sk-ant-..."   # Claude
export XAI_API_KEY="xai-..."            # Grok
export OPENAI_API_KEY="sk-..."          # GPT
codineer login                          # or use OAuth
```

**2. Start coding:**

```bash
codineer                                       # interactive REPL
codineer prompt "explain this project"         # one-shot
codineer -p "list TODOs" --output-format json  # scripting
```

Codineer auto-detects which provider to use. No extra configuration needed.

## What It Does

- **Reads your project** — `CODINEER.md`, configs, git state, LSP diagnostics
- **Runs tools** — shell, file read/write/edit, glob, grep, web fetch, notebooks
- **Manages context** — session save/restore, compaction, conversation history
- **Extends easily** — MCP servers (stdio/SSE/HTTP/WebSocket), plugins, custom agents & skills
- **Stays safe** — sandboxed execution, permission modes, private by design
- **Works everywhere** — macOS, Linux, Windows; Anthropic, OpenAI, xAI, Ollama

## Configuration

Codineer loads settings from (highest to lowest precedence):

1. `.codineer/settings.local.json` — local overrides (gitignored)
2. `.codineer/settings.json` — project settings
3. `~/.codineer/settings.json` — global settings

Run `codineer help` for full documentation.

## License

[MIT](LICENSE)

---

<p align="center">
  Made with 🦀 by <a href="https://github.com/andeya">andeya</a>
</p>
