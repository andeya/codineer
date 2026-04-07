# codineer-runtime

Core runtime engine for [Codineer](https://github.com/andeya/codineer).

This crate implements the session lifecycle, configuration loading, MCP (Model Context Protocol) client, system prompt assembly, permission management, sandboxing, and conversation orchestration.

### File operations highlights

- **Grep / glob**: powered by ripgrep core crates (`grep-regex`, `grep-searcher`, `ignore`) for high-performance, `.gitignore`-aware search with multiline regex support.
- **Read**: supports text files, PDF text extraction (`lopdf`), and image base64 encoding.
- **Write / edit**: atomic writes via temp-file + rename, mtime-based conflict detection, line-ending preservation, ambiguity detection for single edits, and configurable file size limits.
- **Diff**: LCS-based unified diff generation using `similar`.

### Conversation orchestration

`run_turn_with_blocks` executes tools in three phases: sequential prefix → concurrent batch (via `execute_batch` on `ToolExecutor`) → sequential suffix, controlled by each tool's `is_concurrency_safe` flag.

## Note

This is an internal crate of the Codineer project. It is published to crates.io as a dependency of `codineer-cli` and is not intended for standalone use. API stability is not guaranteed outside the Codineer workspace.

## License

MIT — see [LICENSE](https://github.com/andeya/codineer/blob/main/LICENSE) for details.
