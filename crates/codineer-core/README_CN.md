# codineer-core

所有 [Codineer](https://github.com/andeya/codineer) crate 共享的核心类型与抽象。

[English](README.md)

---

本 crate 提供工作区中其他所有 crate 依赖的基础构建块：

| 模块           | 用途                                                                            |
| -------------- | ------------------------------------------------------------------------------- |
| `events`       | `EventKind` 枚举（35 个变体）与零拷贝 `RuntimeEvent`，用于观察者分发            |
| `observer`     | `RuntimeObserver` trait、`EventDirective`、`Decision`，用于 Hook/插件集成       |
| `error`        | `RuntimeError` 枚举（结构化，基于 `thiserror`）                                 |
| `cancel`       | `CancelToken` / `CancelGuard`，协作式取消                                       |
| `config`       | `ConfigSource` 枚举（全局、项目、本地、CLI）                                    |
| `prompt_types` | `SystemBlock`、`BlockKind`、`CacheControl`、`ThinkingConfig`，用于 API 负载构建 |
| `loop_state`   | `LoopState`、`Transition`、`StopReason`，用于对话循环控制                       |
| `elicitation`  | `ElicitationRequest`、`ElicitationHandler` trait，用于结构化用户输入            |
| `telemetry`    | `TelemetryEvent`、`TelemetrySink` trait，用于使用分析                           |
| `gemini_cache` | `GeminiCacheConfig`，用于 Gemini `cachedContents` API 集成                       |
| `oauth`        | OAuth PKCE 流程类型、Token 存储和 Provider 无关的认证                            |
| `credentials_types` | 跨 Provider 客户端共享的凭据链类型                                          |

### 设计原则

- **零 `unsafe`** — 在工作区级别通过 `#![forbid(unsafe_code)]` 强制执行。
- **借用优先** — `RuntimeEvent` 使用 `&str` / `Cow<'a, str>` 实现零拷贝分发。
- **Newtype 安全** — `ElicitationId`、`OptionId` 在类型层面防止字符串混淆。
- **尽可能 `const fn`** — `CacheControl::ephemeral()`、`ThinkingConfig::disabled()`。

## 说明

本 crate 是 Codineer 项目的内部组件，作为 `codineer-cli` 的依赖发布到 crates.io，不用于独立使用。在 Codineer 工作区之外不保证 API 稳定性。

## 许可证

MIT — 详见 [LICENSE](https://github.com/andeya/codineer/blob/main/LICENSE)。
