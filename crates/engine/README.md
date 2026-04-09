# aineer-runtime

Core runtime engine for [Aineer](https://github.com/andeya/aineer).

[中文文档](README_CN.md)

This crate implements the session lifecycle, configuration loading, system prompt assembly, permission management, sandboxing, error recovery, and conversation orchestration. MCP transport is handled by the separate `aineer-mcp` crate.

### File operations highlights

- **Grep / glob**: powered by ripgrep core crates (`grep-regex`, `grep-searcher`, `ignore`) for high-performance, `.gitignore`-aware search with multiline regex support.
- **Read**: supports text files, PDF text extraction (`lopdf`), and image base64 encoding.
- **Write / edit**: atomic writes via temp-file + rename, mtime-based conflict detection, line-ending preservation, ambiguity detection for single edits, and configurable file size limits.
- **Diff**: LCS-based unified diff generation using `similar`.

### Conversation orchestration

`run_turn_with_blocks` orchestrates each turn through four distinct methods:

1. **`stream_with_recovery`** — sends the API request with automatic retry/recovery on transient failures.
2. **`check_permissions`** — runs sequential permission checks and observer pre-hooks for each pending tool use.
3. **`execute_tools`** — executes approved tools (concurrently when safe via `execute_batch` on `ToolExecutor`, otherwise sequentially).
4. **`apply_post_hooks`** — runs observer post-hooks and builds result messages for the session.

### Streaming tool execution

The `StreamingToolExecutor` starts tools as soon as their parameters arrive in the SSE stream — before the model finishes generating. Concurrency-safe tools run in parallel; a bash failure automatically aborts sibling tool calls. Real-time progress events flow back to the renderer.

### Model-based context compaction

When conversation context approaches the model's input budget, the `compact` module triggers an LLM summarization call to compress history while preserving key decisions and file modifications. A heuristic fallback is used when the summarization call itself fails. Token budgets are calculated per-model via `ModelContextWindow`.

### Fine-grained permission rules

Beyond the three permission modes (`read-only`, `workspace-write`, `danger-full-access`), the `permissions` module supports glob-pattern rules per tool and input:

```json
[{ "tool": "bash", "input": "rm *", "decision": "always-deny" }]
```

Rules are evaluated in order; the first matching rule wins. `always-allow`, `always-deny`, and `always-ask` decisions are supported.

## Note

This is an internal crate of the Aineer project. It is published to crates.io as a dependency of `aineer-cli` and is not intended for standalone use. API stability is not guaranteed outside the Aineer workspace.

## License

MIT — see [LICENSE](https://github.com/andeya/aineer/blob/main/LICENSE) for details.
