# aineer-runtime

[Aineer](https://github.com/andeya/aineer) 的核心运行时引擎。

[English](README.md)

---

本 crate 实现了会话生命周期、配置加载、系统提示组装、权限管理、沙箱、错误恢复和对话编排。MCP 传输由独立的 `aineer-mcp` crate 处理。

### 文件操作亮点

- **Grep / glob**：基于 ripgrep 核心库（`grep-regex`、`grep-searcher`、`ignore`），支持高性能、`.gitignore` 感知的多行正则搜索。
- **读取**：支持文本文件、PDF 文本提取（`lopdf`）和图片 base64 编码。
- **写入 / 编辑**：通过临时文件 + 重命名实现原子写入，基于 mtime 的冲突检测，保留行尾符，单次编辑歧义检测，以及可配置的文件大小限制。
- **差异对比**：使用 `similar` 库实现基于 LCS 的统一 diff 生成。

### 对话编排

`run_turn_with_blocks` 通过四个独立方法编排每一轮对话：

1. **`stream_with_recovery`** — 发送 API 请求，在瞬态故障时自动重试/恢复。
2. **`check_permissions`** — 对每个待执行的工具逐一进行权限检查和观察者前置钩子。
3. **`execute_tools`** — 执行已批准的工具（通过 `ToolExecutor` 的 `execute_batch` 安全并发，否则串行执行）。
4. **`apply_post_hooks`** — 运行观察者后置钩子并构建会话结果消息。

### 流式工具执行

`StreamingToolExecutor` 在工具参数通过 SSE 流到达时立即启动执行——无需等待模型生成完毕。并发安全的工具并行运行；bash 失败时自动中止兄弟工具调用。实时进度事件回传给渲染器。

### 基于模型的上下文压缩

当对话上下文接近模型的输入预算时，`compact` 模块触发 LLM 总结调用来压缩历史，同时保留关键决策和文件修改记录。当总结调用本身失败时使用启发式回退。Token 预算通过 `ModelContextWindow` 按模型计算。

### 精细权限规则

在三种权限模式（`read-only`、`workspace-write`、`danger-full-access`）之外，`permissions` 模块支持按工具和输入的 glob 模式规则：

```json
[{ "tool": "bash", "input": "rm *", "decision": "always-deny" }]
```

规则按顺序匹配，首个命中的规则生效。支持 `always-allow`、`always-deny` 和 `always-ask` 三种决策。

## 说明

本 crate 是 Aineer 项目的内部组件，作为 `aineer-cli` 的依赖发布到 crates.io，不用于独立使用。在 Aineer 工作区之外不保证 API 稳定性。

## 许可证

MIT — 详见 [LICENSE](https://github.com/andeya/aineer/blob/main/LICENSE)。
