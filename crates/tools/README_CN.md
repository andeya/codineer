# codineer-tools

[Codineer](https://github.com/andeya/codineer) 的 AI 可调用工具定义与执行。

[English](README.md)

---

本 crate 实现了 AI Agent 可用的所有内置工具：

| 分类             | 工具                                                                      |
| ---------------- | ------------------------------------------------------------------------- |
| **文件 I/O**     | `read_file`、`write_file`、`edit_file`、`glob_search`、`grep_search`      |
| **Shell**        | `bash`、`PowerShell`、`REPL`                                              |
| **Web**          | `WebFetch`、`WebSearch`                                                   |
| **Notebook**     | `NotebookEdit`                                                            |
| **Agent**        | `Agent`（子 Agent 编排）、`SendUserMessage`                               |
| **LSP**          | `Lsp`（悬浮、补全、跳转定义、引用、符号、重命名、格式化、诊断）           |
| **任务管理**     | `TaskCreate`、`TaskGet`、`TaskList`、`TaskUpdate`、`TaskStop`             |
| **规划模式**     | `EnterPlanMode`、`ExitPlanMode`                                           |
| **Git Worktree** | `EnterWorktree`、`ExitWorktree`                                           |
| **定时任务**     | `CronCreate`、`CronDelete`、`CronList`                                    |
| **MCP 资源**     | `ListMcpResources`、`ReadMcpResource`、`MCPSearch`                        |
| **协作**         | `TeamCreate`、`TeamDelete`、`SendMessage`、`SlashCommand`                 |
| **其他**         | `TodoWrite`、`Skill`、`ToolSearch`、`Config`、`StructuredOutput`、`Sleep` |

### 工具懒加载

并非所有工具都在初始提示中发送给模型。核心工具立即加载，MCP 工具和扩展工具通过 `ToolSearch` 工具按需发现。这减少了 prompt Token 消耗，让模型专注于最相关的能力。Agent 可调用 `ToolSearch` 按需查找并激活额外工具。

## 说明

本 crate 是 Codineer 项目的内部组件，作为 `codineer-cli` 的依赖发布到 crates.io，不用于独立使用。在 Codineer 工作区之外不保证 API 稳定性。

## 许可证

MIT — 详见 [LICENSE](https://github.com/andeya/codineer/blob/main/LICENSE)。
