# codineer-api

AI provider API clients and streaming for [Codineer](https://github.com/andeya/codineer).

[中文文档](README_CN.md)

This crate handles communication with AI model providers (Anthropic, OpenAI-compatible, xAI/Grok), including request construction, SSE stream parsing, authentication, and retry logic.

### Provider clients

| Client               | Covers                                                       |
| -------------------- | ------------------------------------------------------------ |
| `AnthropicClient`    | Anthropic native API (Claude models)                         |
| `OpenAiCompatClient` | Any OpenAI-compatible endpoint (OpenAI, Gemini, Ollama, …)  |
| `CodineerProvider`   | Codineer's own OAuth-based provider                          |

### Cache strategy

The `ProviderCacheStrategy` trait decouples provider-specific caching from the client:

- **`GeminiCacheStrategy`** — manages Google's `cachedContents` API to cache system prompts and tool definitions, reducing latency and token costs for long sessions.
- **`NoCacheStrategy`** — no-op default for providers without caching support.

New provider-specific cache strategies can be added by implementing the trait.

### Key capabilities

- SSE stream parsing with incremental content, tool-call deltas, and thinking blocks
- Automatic retry with exponential backoff on transient failures
- OAuth and API-key credential chains
- `cache_control` on input content blocks for Anthropic prompt caching

## Note

This is an internal crate of the Codineer project. It is published to crates.io as a dependency of `codineer-cli` and is not intended for standalone use. API stability is not guaranteed outside the Codineer workspace.

## License

MIT — see [LICENSE](https://github.com/andeya/codineer/blob/main/LICENSE) for details.
