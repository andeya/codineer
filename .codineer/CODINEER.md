# CODINEER.md

This file provides persistent context to Codineer when working with this repository.

## Project

**Codineer** is a local AI coding-agent CLI written in Rust.  It is the project that _builds_ the
`codineer` binary — meaning this repo is both the tool and its own dogfood environment.

- Binary crate: `codineer-cli` → produces the `codineer` executable
- Workspace root: repo root (`Cargo.toml` with `members = ["crates/*"]`)
- Current version: `0.6.7` (shared across all crates via `workspace.package.version`)

## Repository layout

```
crates/
  api/          # HTTP client, provider abstractions (Anthropic, OpenAI-compat, codineer-provider)
  runtime/      # Core engine: config, session, permissions, sandbox, MCP, prompts, hooks
  tools/        # Built-in tool implementations (read/write/edit/bash/agent/todo/skill…)
  plugins/      # Plugin system: manifest, discovery, install, bundled embedding
  commands/     # Slash-command specs, discovery (skills, agents), git helpers
  lsp/          # LSP integration for workspace diagnostics
  codineer-cli/ # CLI entry point, REPL, banner, init, session store, bootstrap
.codineer/      # Project config committed to repo (settings.json, CODINEER.md, .gitignore)
```

## Languages & toolchain

- **Language**: Rust (edition 2021, MSRV declared in workspace `Cargo.toml`)
- **Build**: `cargo build` / `cargo build --release`
- **No runtime dependencies** — single static binary

## Verification commands

Run all three from the **repo root** before every commit:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

> `cargo fmt` without `--check` auto-fixes formatting; `--check` is used in CI.

## Coding conventions

- **No external `lazy_static`** — use `std::sync::OnceLock` for one-time initialization.
- **Error types**: prefer `Box<dyn std::error::Error>` at boundaries; typed errors inside crates.
- **Paths**: always use `runtime::codineer_runtime_dir(cwd)` (not raw `.codineer` joins) for
  runtime artifacts (sessions, agents, todos, sandbox dirs).  Use `runtime::find_project_codineer_dir(cwd)`
  to locate the nearest initialized `.codineer/` without falling back to home.
- **Config loading**: `ConfigLoader::default_for(cwd)` walks ancestor dirs to find the project
  `.codineer/settings.json`; the global config is always `~/.codineer/settings.json`.
- **Plugin manifests**: `plugin.json` lives at the plugin directory root (not in `.codineer-plugin/`).
- **No `.codineer.json` flat config** — only directory-based `settings.json` is supported.
- Comments should explain *why*, not *what*.  Avoid narrating obvious code.
- Commit messages in English; code comments in English.

## Key design decisions

- `.codineer/` in the project is only created by `codineer init`; the binary never auto-creates it
  on startup (only `~/.codineer/` is auto-scaffolded).
- Runtime artifacts (sessions, todos, agents, sandbox) fall back to `~/.codineer/` when no project
  `.codineer/settings.json` exists in the ancestor chain.
- `is_initialized` (banner hint) checks only `cwd/.codineer/settings.json` — no ancestor walk —
  to avoid false-positives from `~/.codineer/settings.json`.
- Bundled plugins are embedded via `include_str!` and extracted to `~/.codineer/plugins/` on
  startup; they are NOT auto-discovered from `<project>/.codineer/plugins/`.

## Working agreement

- Keep shared project defaults in `.codineer/settings.json`; machine-local overrides in
  `.codineer/settings.local.json` (gitignored).
- Prefer small, focused commits.  Run the three verification commands above before pushing.
- Update this file when architecture or conventions change.
