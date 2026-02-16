<p align="center">
  <img src="assets/logo-light.svg" alt="Codineer" width="360">
  <br>
  <em>你的本地 AI 编程助手 — 单一二进制，零云端锁定。</em>
</p>

<p align="center">
  <a href="https://github.com/andeya/codineer/actions"><img src="https://github.com/andeya/codineer/workflows/CI/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
  <a href="https://github.com/andeya/codineer/releases"><img src="https://img.shields.io/github/v/release/andeya/codineer" alt="Release"></a>
  <a href="README.md">English</a>
</p>

---

Codineer 是一个**本地优先的编程智能体**，完全在你的终端中运行。它能读取你的工作区、理解项目结构，帮你编写、重构、调试和交付代码 — 支持交互式对话和一次性命令两种模式。

使用安全 Rust 构建，编译为单个独立二进制文件。无守护进程，无云端依赖（自带 API Key 即可）。

## 为什么选择 Codineer？

- **隐私优先** — 代码始终留在本地，只有你主动发送的提示词才会离开终端
- **工作区感知** — 每轮对话前自动读取 `CODINEER.md`、项目配置、Git 状态和 LSP 诊断信息
- **工具丰富** — Shell 执行、文件读写编辑、全局搜索、网页抓取、待办管理、Notebook 编辑等
- **高度可扩展** — 支持 MCP 服务器、本地插件、自定义 Agent 和 Skill（通过 `.codineer/` 目录）
- **安全沙箱** — 可选的进程隔离：Linux 命名空间 或 macOS Seatbelt 沙箱
- **多供应商** — 支持 Anthropic (Claude)、xAI (Grok) 以及任何 OpenAI 兼容 API

## 快速开始

### 安装

```bash
# 从源码安装
cargo install --path crates/codineer-cli --locked

# 或通过 Homebrew（macOS/Linux）
brew install andeya/tap/codineer

# 或从 GitHub Releases 下载预编译二进制
```

### 认证

```bash
# Anthropic (Claude)
export ANTHROPIC_API_KEY="sk-ant-..."

# xAI (Grok)
export XAI_API_KEY="xai-..."

# OpenAI
export OPENAI_API_KEY="sk-..."

# 或使用 Anthropic OAuth：
codineer login
```

### 运行

```bash
# 交互式 REPL
codineer

# 一次性提示
codineer prompt "解释这个项目的架构"

# JSON 输出（适合脚本集成）
codineer -p "列出所有 TODO 项" --output-format json
```

## 核心功能

| 功能 | 说明 |
|------|------|
| **交互式 REPL** | 对话式编程会话，支持 Vim 键绑定、Tab 补全和历史记录 |
| **工作区工具** | `bash`、`read_file`、`write_file`、`edit_file`、`glob`、`grep`、`web_fetch`、`web_search`、`todo_write`、`notebook_edit` |
| **斜杠命令** | `/status`、`/compact`、`/config`、`/cost`、`/model`、`/permissions`、`/resume`、`/clear`、`/init`、`/diff`、`/export` |
| **Agent 与 Skill 系统** | 从 `.codineer/agents/` 和 `.codineer/skills/` 发现并运行自定义智能体和技能 |
| **插件系统** | 安装、管理和扩展自定义插件与钩子 |
| **MCP 支持** | 通过 Model Context Protocol 连接外部工具服务器（stdio、SSE、HTTP、WebSocket） |
| **Git 集成** | 分支检测、工作树管理、提交/PR 工作流 |
| **会话管理** | 保存、恢复和续接编程会话 |
| **安全沙箱** | Linux `unshare` 或 macOS `sandbox-exec` 进程隔离 |

## 配置

Codineer 按以下优先级加载配置：

1. `.codineer/settings.local.json` — 本地覆盖（已 gitignore）
2. `.codineer/settings.json` — 项目级配置
3. `~/.codineer/settings.json` — 用户全局配置

关键配置项：`model`、`permissionMode`、`mcpServers`、`sandbox`、`hooks`、`enabledPlugins`。

运行 `codineer help` 查看完整的环境变量和配置文件文档。

## 项目结构

```text
crates/
├── api/              # AI 供应商客户端 + 流式传输
├── codineer-cli/     # 交互式 CLI 二进制
├── commands/         # 斜杠命令与 Agent/Skill 发现
├── lsp/              # Language Server Protocol 客户端
├── plugins/          # 插件系统与钩子
