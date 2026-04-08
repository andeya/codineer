# codineer-api

[Codineer](https://github.com/andeya/codineer) 的 AI Provider API 客户端与流式传输。

[English](README.md)

---

本 crate 负责与 AI 模型 Provider（Anthropic、OpenAI 兼容、xAI/Grok）的通信，包括请求构建、SSE 流解析、认证和重试逻辑。

### Provider 客户端

| 客户端               | 覆盖范围                                                   |
| -------------------- | ---------------------------------------------------------- |
| `AnthropicClient`    | Anthropic 原生 API（Claude 模型）                           |
| `OpenAiCompatClient` | 任何 OpenAI 兼容端点（OpenAI、Gemini、Ollama 等）          |
| `CodineerProvider`   | Codineer 自有的 OAuth Provider                              |

### 缓存策略

`ProviderCacheStrategy` trait 将 Provider 特定的缓存逻辑与客户端解耦：

- **`GeminiCacheStrategy`** — 管理 Google 的 `cachedContents` API，缓存系统提示和工具定义，降低长会话的延迟和 Token 成本。
- **`NoCacheStrategy`** — 无缓存支持的 Provider 使用的默认空操作。

实现该 trait 即可添加新的 Provider 缓存策略。

### 核心能力

- SSE 流解析：支持增量内容、工具调用 delta 和思考块
- 瞬态故障自动指数退避重试
- OAuth 和 API Key 凭据链
- 输入内容块上的 `cache_control` 支持 Anthropic prompt 缓存

## 说明

本 crate 是 Codineer 项目的内部组件，作为 `codineer-cli` 的依赖发布到 crates.io，不用于独立使用。在 Codineer 工作区之外不保证 API 稳定性。

## 许可证

MIT — 详见 [LICENSE](https://github.com/andeya/codineer/blob/main/LICENSE)。
