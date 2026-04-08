# codineer-core

Core types and abstractions shared across all [Codineer](https://github.com/andeya/codineer) crates.

[中文文档](README_CN.md)

This crate provides the foundational building blocks that every other crate in the workspace depends on:

| Module         | Purpose                                                                           |
| -------------- | --------------------------------------------------------------------------------- |
| `events`       | `EventKind` enum (35 variants) and zero-copy `RuntimeEvent` for observer dispatch |
| `observer`     | `RuntimeObserver` trait, `EventDirective`, `Decision` for hook/plugin integration |
| `error`        | `RuntimeError` enum (structured, `thiserror`-based)                               |
| `cancel`       | `CancelToken` / `CancelGuard` for cooperative cancellation                        |
| `config`       | `ConfigSource` enum (global, project, local, CLI)                                 |
| `prompt_types` | `SystemBlock`, `BlockKind`, `CacheControl`, `ThinkingConfig` for API payloads     |
| `loop_state`   | `LoopState`, `Transition`, `StopReason` for conversation loop control             |
| `elicitation`  | `ElicitationRequest`, `ElicitationHandler` trait for structured user input        |
| `telemetry`    | `TelemetryEvent`, `TelemetrySink` trait for usage analytics                       |
| `gemini_cache` | `GeminiCacheConfig` for Gemini `cachedContents` API integration                   |
| `oauth`        | OAuth PKCE flow types, token storage, and provider-agnostic auth                  |
| `credentials_types` | Shared credential chain types used across provider clients                   |

### Design principles

- **Zero `unsafe`** — enforced at workspace level via `#![forbid(unsafe_code)]`.
- **Borrow-first** — `RuntimeEvent` uses `&str` / `Cow<'a, str>` for zero-copy dispatch.
- **Newtype safety** — `ElicitationId`, `OptionId` prevent string mix-ups at type level.
- **`const fn`** where possible — `CacheControl::ephemeral()`, `ThinkingConfig::disabled()`.

## Note

This is an internal crate of the Codineer project. It is published to crates.io as a dependency of `codineer-cli` and is not intended for standalone use. API stability is not guaranteed outside the Codineer workspace.

## License

MIT — see [LICENSE](https://github.com/andeya/codineer/blob/main/LICENSE) for details.
