# codineer-commands

[Codineer](https://github.com/andeya/codineer) 的斜杠命令与 Agent/Skill 发现。

[English](README.md)

---

本 crate 实现了 REPL 斜杠命令系统，并提供 CLI 界面所需的 Agent 和 Skill 发现功能。

### 命令分类

| 分类       | 示例                                                                    |
| ---------- | ----------------------------------------------------------------------- |
| **核心**   | `/help`、`/status`、`/version`、`/model`、`/cost`、`/config`、`/memory` |
| **会话**   | `/compact`、`/clear`、`/session`、`/resume`、`/export`                  |
| **Git**    | `/diff`、`/branch`、`/commit`、`/commit-push-pr`、`/pr`、`/issue`、`/worktree` |
| **Agent**  | `/agents`、`/skills`、`/plugin`                                         |
| **高级**   | `/ultraplan`、`/bughunter`、`/teleport`、`/debug-tool-call`、`/vim`     |
| **诊断**   | `/doctor`                                                               |
| **更新**   | `/update [check\|apply\|dismiss\|status]`                              |
| **导航**   | `/init`、`/permissions`、`/exit`                                        |

`/compact` 通过运行时触发基于模型的上下文压缩。`/permissions` 与精细权限规则引擎交互。

## 说明

本 crate 是 Codineer 项目的内部组件，作为 `codineer-cli` 的依赖发布到 crates.io，不用于独立使用。在 Codineer 工作区之外不保证 API 稳定性。

## 许可证

MIT — 详见 [LICENSE](https://github.com/andeya/codineer/blob/main/LICENSE)。
