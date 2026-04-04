# codineer-api

AI provider API clients and streaming for [Codineer](https://github.com/andeya/codineer).

This crate handles communication with AI model providers (Anthropic, OpenAI-compatible, xAI/Grok), including request construction, SSE stream parsing, authentication, and retry logic.

## Note

This is an internal crate of the Codineer project. It is published to crates.io as a dependency of `codineer-cli` and is not intended for standalone use. API stability is not guaranteed outside the Codineer workspace.

## License

MIT — see [LICENSE](https://github.com/andeya/codineer/blob/main/LICENSE) for details.
