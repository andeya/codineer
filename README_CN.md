<p align="center">
  <img src="https://raw.githubusercontent.com/andeya/aineer/main/docs/images/logo-horizontal-light.svg" height="64" alt="Aineer">
</p>
<p align="center">
  <em>Agent 不是功能，而是你所处的环境。</em>
</p>

<p align="center">
  <a href="https://github.com/andeya/aineer/actions"><img src="https://github.com/andeya/aineer/workflows/CI/badge.svg" alt="CI"></a>
  <a href="https://github.com/andeya/aineer/releases"><img src="https://img.shields.io/github/v/release/andeya/aineer" alt="Release"></a>
  <a href="https://crates.io/crates/aineer"><img src="https://img.shields.io/crates/v/aineer.svg" alt="crates.io"></a>
  <img src="https://raw.githubusercontent.com/andeya/aineer/main/docs/images/badge-platforms.svg" alt="macOS | Linux | Windows">
  <br>
  <a href="README.md">English</a>
</p>

---

**Aineer** 是 **ADE（代理式开发环境）**。Shell、对话式 AI 与 Agent 自主执行在同一信息流中衔接，并与工作区持续对齐——减少从查阅代码到交付变更间的上下文切换。**CLI REPL** 是成熟的功能完整界面（40+ 工具、流式执行、多 Provider AI、插件、MCP）。**桌面 GUI** 提供设置、主题、集成终端和模型选择；桌面端 AI 对话正在积极开发中。

基于 **Tauri 2 + React 19 + Tailwind CSS + xterm.js** 构建。无守护进程，无运行时依赖——带上任意模型即可开始。

<p align="center">
  <img src="https://raw.githubusercontent.com/andeya/aineer/main/docs/images/ScreenShot_01.png" alt="Aineer REPL 截图" width="780">
</p>

## 为什么选择 Aineer？

大多数 AI 编程 CLI 将你绑定在单一 Provider 上。Claude Code 依赖 Anthropic，Codex CLI 依赖 OpenAI。**Aineer 支持所有 Provider——包括本地模型。**

|                                                                                  |     Aineer     | Claude Code  |    Codex CLI    |   Aider    |
| -------------------------------------------------------------------------------- | :------------: | :----------: | :-------------: | :--------: |
| **多 Provider**（Anthropic、OpenAI、xAI、Ollama…）                               |  **全部内置**  | 仅 Anthropic | OpenAI + Ollama |    支持    |
| **零 Token 成本**（[免费使用主流模型](#token-free-gateway免费使用主流-ai-模型)） |    **支持**    |    不支持    |     不支持      |   不支持   |
| **零配置本地 AI**（自动检测 Ollama）                                             |    **支持**    |    不支持    |  `--oss` 参数   | 需手动配置 |
| **单一二进制**（无运行时依赖）                                                   | **Rust+Tauri** |   Node.js    |     Node.js     |   Python   |
| **多模态输入**（`@image.png`、剪贴板粘贴）                                       |    **支持**    |     支持     |      有限       |    有限    |
| **MCP 协议**（外部工具集成）                                                     |    **支持**    |     支持     |      支持       |    支持    |
| **插件系统** + Agent + Skill                                                     |    **支持**    |     支持     |     不支持      |   不支持   |
| **权限模式**（只读 → 完全访问）                                                  |    **支持**    |     支持     |      支持       |    部分    |
| **流式工具执行**（并行工具、兄弟中止）                                           |    **支持**    |     支持     |     不支持      |   不支持   |
| **上下文缓存**（Gemini + Anthropic）                                             |    **支持**    | 仅 Anthropic |     不支持      |   不支持   |
| **Git 工作流**（/commit、/pr、/diff、/branch）                                   |    **内置**    |   通过工具   |    通过工具     |  自动提交  |
| **Vim 模式**                                                                     |    **支持**    |    不支持    |     不支持      |   不支持   |

---

## 安装

**桌面应用（推荐）：**

从 [Releases](https://github.com/andeya/aineer/releases) 下载最新安装包：

| 平台                  | 文件                               |
| --------------------- | ---------------------------------- |
| macOS (Apple Silicon) | `Aineer_*_aarch64.dmg`             |
| macOS (Intel)         | `Aineer_*_x64.dmg`                 |
| Linux (x86_64)        | `aineer_*_amd64.deb` / `.AppImage` |
| Linux (ARM64)         | `aineer_*_arm64.deb`               |
| Windows (x86_64)      | `Aineer_*_x64-setup.exe`           |

**仅 CLI 模式：**

```bash
brew install andeya/aineer/aineer            # Homebrew（macOS / Linux）
cargo install aineer                           # Cargo（从 crates.io）
```

<details><summary>从源码构建</summary>

```bash
git clone https://github.com/andeya/aineer.git
cd aineer
bun install                                    # 安装前端依赖
cargo tauri build                              # 构建 Tauri 桌面应用（GUI + CLI）
# 或仅构建 CLI：
cargo install --path app --locked
```

**前置条件：** Rust 工具链、[Bun](https://bun.sh)、以及平台相关的 [Tauri 依赖](https://v2.tauri.app/start/prerequisites/)。

</details>

---

## 快速开始

```bash
# 1. 选择一种 Provider
export ANTHROPIC_API_KEY="sk-ant-..."        # 或 OPENAI_API_KEY、XAI_API_KEY、GEMINI_API_KEY 等
ollama serve                                  # 或直接启动 Ollama——无需 Key
aineer login                                  # 或 OAuth 登录

# 2. 开始编码
aineer init                                   # 初始化项目上下文（可选）
aineer                                        # 交互式 REPL
aineer "解释一下这个项目"                     # 一次性提问
```

Aineer 根据可用凭据自动检测 Provider，无需额外参数。详见[模型与 Provider](#模型与-provider)。

---

## 模型与 Provider

### 模型别名

在 `settings.json` 中定义短名，随处使用：

```json
{
  "modelAliases": {
    "sonnet": "claude-sonnet-4-6",
    "gpt": "gpt-4o",
    "flash": "gemini/gemini-2.5-flash"
  }
}
```

```bash
aineer --model sonnet "帮我 review 这次改动"
aineer models                                  # 列出所有可用模型
```

### 自定义 Provider（OpenAI 兼容）

任意 OpenAI 兼容 API 均可通过 `provider/model` 语法使用：

```bash
aineer --model ollama/qwen3-coder "重构这个模块"
aineer --model groq/llama-3.3-70b-versatile "解释这个"
aineer --model ollama                          # 自动选择最佳本地模型
```

**Ollama 零配置**：当没有 API key 且 Ollama 正在运行时，自动检测并选择最佳编程模型。不支持 function calling 的模型自动降级为纯文本模式。

### 模型解析与回退

未指定 `--model` 时：`settings.json` → 可用凭据 → 运行中的 Ollama。主模型不可用时，依序尝试 `fallbackModels`：

```json
{
  "model": "sonnet",
  "fallbackModels": ["ollama/qwen3-coder", "groq/llama-3.3-70b-versatile"]
}
```

会话中切换模型：`/model <名称>`

### Token Free Gateway（免费使用主流 AI 模型）

> **零 API Token 成本** — 通过浏览器登录，一键聚合 Claude、ChatGPT、Gemini、DeepSeek 等 10+ 主流大模型，完全免费调用。

[Token Free Gateway](https://github.com/andeya/token-free-gateway) 通过驱动各大模型官方 Web 端来替代付费 API Key。只要你能在浏览器中使用这些模型，就可以通过 Aineer 调用。

| 传统方式             | Token Free Gateway 方式 |
| -------------------- | ----------------------- |
| 购买 API Token       | **完全免费**            |
| 按请求付费           | 无强制配额              |
| 需要绑定信用卡       | 仅需浏览器登录          |
| API Token 有泄露风险 | 凭据仅本地存储          |

<details><summary>配置方法</summary>

1. 部署并启动 [Token Free Gateway](https://github.com/andeya/token-free-gateway)（默认端口 3456）
2. 在 `settings.json` 中添加 provider：

```json
{
  "model": "token-free-gateway/claude-sonnet-4-6",
  "env": { "TFG_API_KEY": "your-gateway-token" },
  "providers": {
    "token-free-gateway": {
      "baseUrl": "http://127.0.0.1:3456/v1",
      "apiKeyEnv": "TFG_API_KEY",
      "defaultModel": "claude-opus-4-6"
    }
  }
}
```

</details>

<details><summary>Google Gemini 配置（免费 API Key）</summary>

在 [Google AI Studio](https://aistudio.google.com/apikey) 免费申请 Key。使用 OpenAI 兼容端点：

```json
{
  "model": "gemini/gemini-2.5-flash",
  "env": { "GEMINI_API_KEY": "AIzaSy..." },
  "providers": {
    "gemini": {
      "baseUrl": "https://generativelanguage.googleapis.com/v1beta/openai",
      "apiKeyEnv": "GEMINI_API_KEY"
    }
  }
}
```

</details>

<details><summary>阿里云通义 DashScope / Azure OpenAI 配置</summary>

**DashScope：** `aineer --model dashscope/qwen-plus-2025-07-28 "..."` 并设置 `DASHSCOPE_API_KEY`。按[官方文档](https://help.aliyun.com/zh/model-studio/)配置 `baseUrl`。

**Azure OpenAI：** 在 `providers.<name>` 下设置 `apiVersion`（如 `2024-02-15-preview`）。完整示例见 [`settings.example.json`](https://github.com/andeya/aineer/blob/main/settings.example.json)。

</details>

---

## 使用方法

### 交互式 REPL

```bash
aineer
```

主提示符为 **`❯`**，用自然语言交流。支持**斜杠命令**（Tab 自动补全）：

| 分类      | 命令                                                                     |
| --------- | ------------------------------------------------------------------------ |
| **信息**  | `/help` `/status` `/version` `/model` `/cost` `/config` `/memory`        |
| **会话**  | `/compact` `/clear` `/session` `/resume` `/export`                       |
| **Git**   | `/diff` `/branch` `/commit` `/commit-push-pr` `/pr` `/issue` `/worktree` |
| **Agent** | `/agents` `/skills` `/plugin`                                            |
| **高级**  | `/ultraplan` `/bughunter` `/teleport` `/debug-tool-call` `/vim`          |
| **诊断**  | `/doctor`                                                                |
| **更新**  | `/update [check \| apply \| dismiss \| status]`                          |

<details><summary>快捷键</summary>

| 快捷键                               | 功能                                      |
| ------------------------------------ | ----------------------------------------- |
| `?`                                  | 内联快捷键参考面板                        |
| `!<命令>`                            | Bash 模式 — 向 AI 发送 shell 命令执行请求 |
| `@`                                  | 文件 / 图片附件（Tab 补全路径）           |
| `Ctrl+V` / `/image`                  | 粘贴剪贴板图片                            |
| `↑` / `↓`                            | 历史记录回溯                              |
| `Shift+Enter`、`Ctrl+J`、`\ + Enter` | 插入换行                                  |
| `Ctrl+C`                             | 取消输入；空提示符下连按两次退出          |
| `Ctrl+D`                             | 退出（空提示符下）                        |
| `双击 Esc`                           | 清空输入                                  |
| `/vim`                               | 切换 Vim 模态编辑                         |

</details>

### 文件与图片附件

使用 `@` 前缀将上下文附加到消息中：

| 语法                 | 效果                         |
| -------------------- | ---------------------------- |
| `@src/main.rs`       | 注入文件内容（最多 2000 行） |
| `@src/main.rs:10-50` | 注入指定行范围               |
| `@src/`              | 列出目录内容                 |
| `@photo.png`         | 以多模态图片块（base64）附加 |

剪贴板图片：`Ctrl+V`（macOS/Linux）或 `/image`（全平台）。拖拽图片路径到终端可自动识别。

### 一次性提问与脚本

```bash
aineer "解释这个项目的架构"
aineer -p "列出所有 TODO" --output-format json
aineer --permission-mode read-only "审计代码"
```

| 参数                                          | 说明                                                 |
| --------------------------------------------- | ---------------------------------------------------- |
| `--model <名称>`                              | 选择模型                                             |
| `--output-format text \| json \| stream-json` | 输出格式                                             |
| `--allowedTools <列表>`                       | 限制工具访问（逗号分隔）                             |
| `--permission-mode <模式>`                    | `read-only`、`workspace-write`、`danger-full-access` |
| `--resume <文件>`                             | 恢复已保存的会话                                     |

### 权限模式

| 模式                 | 允许                     |
| -------------------- | ------------------------ |
| `read-only`          | 只读和搜索，不允许写操作 |
| `workspace-write`    | 编辑工作区内文件（默认） |
| `danger-full-access` | 完全无限制，包含系统命令 |

---

## 配置

Aineer 从多个 JSON 文件合并设置（优先级从高到低）：

| 文件                          | 作用域          | 是否提交         |
| ----------------------------- | --------------- | ---------------- |
| `.aineer/settings.local.json` | 项目 — 本地覆盖 | 否（gitignored） |
| `.aineer/settings.json`       | 项目配置        | 是               |
| `~/.aineer/settings.json`     | 用户 — 全局配置 | —                |

> **完整字段示例：** [`settings.example.json`](https://github.com/andeya/aineer/blob/main/settings.example.json)

```json
{
  "model": "sonnet",
  "modelAliases": { "sonnet": "claude-sonnet-4-6" },
  "permissionMode": "workspace-write",
  "env": { "ANTHROPIC_API_KEY": "sk-ant-..." },
  "providers": { "ollama": { "baseUrl": "http://my-server:11434/v1" } },
  "mcpServers": { "my-server": { "command": "node", "args": ["server.js"] } },
  "hooks": { "PreToolUse": ["lint-check"] }
}
```

```bash
aineer config set model sonnet         # 设置配置项
aineer config get model                # 读取配置项
aineer config list                     # 列出全部配置
```

<details><summary>环境变量</summary>

通过 Shell export **或** settings.json 的 `"env"` 字段设置（Shell export 优先）：

| 变量                      | 用途                                                                    |
| ------------------------- | ----------------------------------------------------------------------- |
| `ANTHROPIC_API_KEY`       | Claude API Key                                                          |
| `OPENAI_API_KEY`          | OpenAI API Key                                                          |
| `XAI_API_KEY`             | xAI / Grok API Key                                                      |
| `GEMINI_API_KEY`          | Google Gemini API Key（[免费申请](https://aistudio.google.com/apikey)） |
| `OPENROUTER_API_KEY`      | OpenRouter API Key                                                      |
| `GROQ_API_KEY`            | Groq Cloud API Key                                                      |
| `DASHSCOPE_API_KEY`       | 阿里云通义 DashScope                                                    |
| `OLLAMA_HOST`             | Ollama 端点（如 `http://192.168.1.100:11434`）                          |
| `AINEER_WORKSPACE_ROOT`   | 覆盖工作区根路径                                                        |
| `AINEER_CONFIG_HOME`      | 覆盖全局配置目录（默认 `~/.aineer`）                                    |
| `AINEER_PERMISSION_MODE`  | 默认权限模式                                                            |
| `NO_COLOR` / `CLICOLOR=0` | 禁用 ANSI 颜色                                                          |

</details>

<details><summary>凭据链与 Claude Code 自动发现</summary>

| Provider           | 凭据链                                                                                              |
| ------------------ | --------------------------------------------------------------------------------------------------- |
| Anthropic (Claude) | `ANTHROPIC_API_KEY` / `ANTHROPIC_AUTH_TOKEN` → Aineer OAuth (`aineer login`) → Claude Code 自动发现 |
| xAI (Grok)         | `XAI_API_KEY`                                                                                       |
| OpenAI             | `OPENAI_API_KEY`                                                                                    |
| 自定义 Provider    | 内联 `apiKey` → `apiKeyEnv` 环境变量                                                                |

如果已安装 Claude Code 并登录，Aineer 自动发现凭据——无需单独获取 API Key：

```json
{ "credentials": { "autoDiscover": true, "claudeCode": { "enabled": true } } }
```

查看认证状态：`aineer status` 或 `aineer status anthropic`

</details>

---

## 项目上下文

`.aineer/AINEER.md` 是**项目记忆文件**——注入到每次对话的 system prompt 中，让 AI 无需反复询问即可了解代码库。典型内容：技术栈、构建/测试命令、编码规范。

```bash
aineer init        # 根据检测到的技术栈自动生成
```

Aineer 沿目录树向上查找并加载所有匹配的指令文件（`.aineer/AINEER.md`、`AINEER.md`、`AINEER.local.md`、`.aineer/instructions.md`），去重拼接。Monorepo 子项目可与根目录文件互补。

---

## 扩展 Aineer

### MCP 服务器

通过 [Model Context Protocol](https://modelcontextprotocol.io) 接入外部工具：

```json
{
  "mcpServers": {
    "my-server": { "command": "node", "args": ["mcp-server.js"] }
  }
}
```

传输类型：`stdio`（默认）、`sse`、`http`、`ws`。

### 插件

插件用于扩展 Aineer，可提供自定义**工具**、**斜杠命令**、**钩子**和**生命周期脚本**：

```
.aineer/plugins/my-plugin/
├── plugin.json              ← 清单
├── tools/query-db.sh        ← AI 自动调用此工具
├── hooks/audit.sh           ← 每次工具调用前/后运行
└── commands/deploy.sh       ← 用户输入 /deploy 时执行
```

```bash
/plugin list                        # 列出所有插件及状态
/plugin install ./path/to/plugin    # 从本地路径或 Git URL 安装
/plugin enable my-plugin            # 启用 / 禁用
```

> **完整插件开发指南：** [`crates/plugins/README.md`](crates/plugins/README.md)

### Agent 与 Skill

**Agent** 是针对专项任务的命名子 Agent 配置。**Skill** 是可复用的提示模板。通过 `aineer agents`、`aineer skills` 或 REPL 中的 `/agents`、`/skills` 管理。

---

## 会话与恢复

每次对话自动保存，随时可恢复——即使跨重启：

```bash
aineer --resume /path/to/session.jsonl
```

REPL 内操作：`/session`（显示路径）、`/resume <路径>`、`/export`、`/compact`（压缩上下文）、`/clear`。

---

## 自动更新

Aineer 每 24 小时检查一次更新，发现新版后显示通知。

```bash
aineer update                   # 检查更新并自动安装
```

REPL 内操作：`/update`、`/update apply`、`/update dismiss`、`/update status`。

---

## 常见问题

<details><summary><strong>无 API Key / 认证错误</strong></summary>

```bash
aineer status                         # 查看检测到的凭据
aineer login                          # OAuth 登录
aineer login anthropic --source claude-code   # 复用 Claude Code 凭据
```

通过 Shell export 或 `settings.json` → `"env"` 设置 API Key。详见[环境变量](#环境变量)。

</details>

<details><summary><strong>模型未找到 / 不支持的模型</strong></summary>

```bash
aineer models                         # 列出所有可用模型
aineer --model ollama/qwen3-coder "测试"   # 显式指定 provider/model
```

自定义 Provider 请确保 `baseUrl` 使用 OpenAI 兼容端点。

</details>

<details><summary><strong>"assistant stream produced no content"</strong></summary>

部分 Provider 使用非标准响应格式。Aineer 会自动解析并以非流式请求重试一次。请确保使用最新版本：`aineer update`。

</details>

<details><summary><strong>编辑文件时权限被拒绝</strong></summary>

修改模式：`aineer --permission-mode danger-full-access` 或在 `settings.json` 中永久设置：`"permissionMode": "danger-full-access"`

</details>

<details><summary><strong>Ollama 未被检测到</strong></summary>

- 确保 Ollama 正在运行：`ollama serve`
- 检查端点：`curl http://localhost:11434/v1/models`
- 远程 Ollama：`export OLLAMA_HOST=http://your-server:11434`

</details>

---

## 路线图

| 功能领域                                                 | 状态       |
| -------------------------------------------------------- | ---------- |
| CLI REPL + 40+ 内置工具                                  | **稳定**   |
| 多 Provider AI（Anthropic、OpenAI、xAI、Ollama、自定义） | **稳定**   |
| 流式工具执行 + 权限系统                                  | **稳定**   |
| MCP 协议 + 插件系统 + 会话管理                           | **稳定**   |
| 上下文缓存（Gemini + Anthropic）+ 自动更新               | **稳定**   |
| 桌面 GUI — 设置、主题、终端、模型选择                    | **稳定**   |
| 桌面 GUI — AI 对话                                       | **开发中** |
| 协作工具（TeamCreate、SendMessage）                      | **实验性** |
| 多渠道触达（飞书、微信、WhatsApp bot）                   | **规划中** |

---

## 许可证

[MIT](LICENSE)

---

<p align="center">
  由 <a href="https://github.com/andeya">andeya</a> 使用 🦀 构建
</p>
