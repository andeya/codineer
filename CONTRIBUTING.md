# Contributing to Aineer

Thanks for your interest in Aineer! This guide helps you get up and running quickly.

## Development setup

1. Install the [stable Rust toolchain](https://rustup.rs/) (edition 2021, MSRV see `Cargo.toml`).
2. Clone the repo and work from the repository root — this is a Cargo workspace.

```bash
git clone https://github.com/andeya/aineer.git
cd aineer
cargo build
```

## Architecture overview

```
aineer (Cargo workspace)
├── crates/
│   ├── app/            # Unified binary — GUI (default) or CLI (--cli)
│   ├── aineer-cli/     # CLI mode library (REPL, rendering, auto-update)
│   ├── aineer-core/    # Shared types, events, observer traits
│   ├── aineer-runtime/ # Conversation engine, config, sessions, permissions
│   ├── aineer-api/     # AI provider clients (Anthropic, OpenAI-compat, Gemini cache)
│   ├── gateway/        # Embedded OpenAI-compatible model gateway
│   ├── terminal/       # Terminal emulator backend (alacritty_terminal + PTY)
│   ├── ui/             # GUI components (egui widgets, theme, timeline)
│   ├── aineer-mcp/     # Model Context Protocol client & transports
│   ├── aineer-lsp/     # Language Server Protocol client
│   ├── aineer-tools/   # Built-in tool implementations
│   ├── aineer-commands/ # Slash command parsing & dispatch
│   └── aineer-plugins/ # Plugin system, hooks, lifecycle
├── assets/             # Logo and badge SVGs
├── settings.example.json # Full configuration reference
└── .github/workflows/    # CI and release automation
```

**Dependency direction** (top → bottom):

```
aineer (app)
    ↓
aineer-cli · aineer-ui · aineer-gateway · aineer-terminal
    ↓
commands · tools · plugins
    ↓
runtime
    ↓
api · mcp · lsp · core
```

`core` has no internal dependencies. Every other crate depends on `core`. The `app` crate (`aineer`) is the only binary.

## Build & verify

Run these before opening a pull request:

```bash
cargo fmt --all --check            # Formatting
cargo clippy --workspace -- -D warnings   # Lints
cargo check --workspace            # Type-check
cargo test --workspace             # All tests
```

## Where to add things

| What you're adding         | Where it goes                                    |
| -------------------------- | ------------------------------------------------ |
| New CLI flag or subcommand | `crates/aineer-cli/src/cli.rs`                   |
| New slash command          | `crates/commands/src/slash_spec.rs`              |
| New built-in tool          | `crates/tools/src/` (+ register in `builtin.rs`) |
| New API provider           | `crates/api/src/providers/`                      |
| Runtime behavior change    | `crates/runtime/src/`                            |
| Plugin system change       | `crates/plugins/src/`                            |
| Shared types / traits      | `crates/aineer-core/src/`                        |

## Code style

- **Follow existing patterns** in the touched crate. Don't introduce a new style.
- **Format with `rustfmt`**. The CI checks formatting.
- **Keep `clippy` clean** for the workspace targets you changed.
- **Use `thiserror`** for error types. Avoid `Box<dyn Error>` in public APIs.
- **Prefer `Result<T, E>`** over ad-hoc error strings.
- **Minimize `.clone()`** — pass references or use `Arc` where shared ownership is needed.
- **Write tests** for new behavior in the same PR. Tests live alongside the module (e.g. `mod tests` at the bottom of the file, or `src/tests/` for larger suites).

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/) style:

```
feat(tools): add WebSearch tool with DuckDuckGo backend
fix(runtime): prevent context overflow on long sessions
docs: update README with self-update instructions
refactor(api): extract ProviderCacheStrategy trait
test(commands): add slash command parsing edge cases
```

Scope is the crate name without the `aineer-` prefix (e.g. `tools`, `runtime`, `api`, `cli`).

## Pull requests

- Branch from `main`.
- Keep each PR scoped to **one clear change**.
- Include: motivation, implementation summary, and what verification you ran.
- If review feedback changes behavior, re-run the verification commands.
- Prefer focused diffs over drive-by refactors.

## Testing tips

```bash
cargo test -p aineer-cli               # Test one crate
cargo test -p aineer-cli -- auto_update  # Filter tests by name
cargo test --workspace -- --nocapture      # See println output
```

Integration tests that require network (API calls) are `#[ignore]`d by default. Run them with:

```bash
cargo test --workspace -- --ignored
```

## Need help?

Open a [GitHub issue](https://github.com/andeya/aineer/issues) or start a discussion. We're happy to help you get started.
