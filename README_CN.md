<p align="center">
  <img src="https://raw.githubusercontent.com/andeya/codineer/main/assets/logo.svg" width="96" alt="">
</p>
<h1 align="center">codineer</h1>
<p align="center">
  <em>你的多 Provider AI 编程助手 — 单一二进制，任意模型，零锁定。</em>
</p>

<p align="center">
  <a href="https://github.com/andeya/codineer/actions"><img src="https://github.com/andeya/codineer/workflows/CI/badge.svg" alt="CI"></a>
  <a href="https://github.com/andeya/codineer/releases"><img src="https://img.shields.io/github/v/release/andeya/codineer" alt="Release"></a>
  <a href="https://crates.io/crates/codineer-cli"><img src="https://img.shields.io/crates/v/codineer-cli.svg" alt="crates.io"></a>
  <img src="https://raw.githubusercontent.com/andeya/codineer/main/assets/badge-platforms.svg" alt="macOS | Linux | Windows">
  <br>
  <a href="README.md">English</a>
</p>

---

**Codineer** 将你的终端变成 AI 编程伙伴。它读取工作区、理解项目上下文，帮你编写、重构、调试和交付代码——全程无需离开命令行。

安全 Rust 构建，**单个二进制文件**。无守护进程，无运行时依赖——带上任意模型即可开始。

## 为什么选择 Codineer？

大多数 AI 编程 CLI 将你绑定在单一 Provider 上。Claude Code 依赖 Anthropic，Codex CLI 依赖 OpenAI。**Codineer 支持所有 Provider——包括本地模型。**

| | Codineer | Claude Code | Codex CLI | Aider |
|---|:---:|:---:|:---:|:---:|
| **多 Provider**（Anthropic、OpenAI、xAI、Ollama…） | **支持** | 仅 Anthropic | 仅 OpenAI | 支持 |
| **零配置本地 AI**（自动检测 Ollama） | **支持** | 不支持 | 不支持 | 需手动配置 |
| **单一二进制**（无运行时依赖） | **Rust** | Node.js | Node.js | Python |
| **MCP 协议**（外部工具集成） | **支持** | 支持 | 支持 | 不支持 |
| **插件系统** + Agent + Skill | **支持** | 部分 | 不支持 | 不支持 |
| **权限模式**（只读 → 完全访问） | **支持** | 支持 | 支持 | 不支持 |
| **工具调用降级**（优雅降级） | **支持** | 不适用 | 不适用 | 不适用 |
| **Git 工作流**（/commit、/pr、/diff、/branch） | **内置** | 通过工具 | 通过工具 | 自动提交 |
| **Vim 模式** | **支持** | 不支持 | 不支持 | 不支持 |
| **CI/CD 就绪**（JSON 输出、工具白名单） | **支持** | 有限 | 支持 | 不支持 |

**核心优势：**

- **Provider 自由** — 用一个参数在 Claude、GPT、Grok、Ollama 或任何 OpenAI 兼容 API 间切换。零厂商锁定。
- **免费本地 AI** — 启动 Ollama，运行 `codineer`。零 API Key，零成本。Codineer 自动检测本地模型并选择最适合编程的那个。
- **即刻启动** — 一条 `cargo install` 或 `brew install`。无需 Node.js、Python、Docker。一个约 15 MB 的二进制文件即可运行。
- **优雅降级** — 不支持 function calling 的模型自动降级为纯文本模式。任何模型都能工作。
- **项目记忆** — `CODINEER.md` 让 AI 拥有关于代码库、规范和工作流的持久上下文。提交到仓库，与团队共享。

## 目录

- [为什么选择 Codineer？](#为什么选择-codineer)
- [安装](#安装)
- [快速开始](#快速开始)
- [使用指南](#使用指南)
  - [交互式 REPL](#交互式-repl)
  - [一次性提问](#一次性提问)
  - [会话管理](#会话管理)
  - [模型选择](#模型选择)
  - [权限模式](#权限模式)
  - [脚本与自动化](#脚本与自动化)
- [项目初始化](#项目初始化)
- [配置](#配置)
- [扩展 Codineer](#扩展-codineer)
  - [MCP 服务器](#mcp-服务器)
  - [插件](#插件)
  - [Agent 与 Skill](#agent-与-skill)
- [内置工具](#内置工具)
- [发布到 crates.io](#发布到-cratesio)
- [许可证](#许可证)

---

## 安装

### Homebrew（macOS / Linux）

```bash
brew install andeya/codineer/codineer
```

### Cargo（从 crates.io）

```bash
cargo install codineer-cli
```

### 下载二进制

前往 **[Releases](https://github.com/andeya/codineer/releases)** 页面下载对应平台的预编译包：

| 平台                  | 文件                                          |
| --------------------- | --------------------------------------------- |
| macOS (Apple Silicon) | `codineer-*-aarch64-apple-darwin.tar.gz`      |
| macOS (Intel)         | `codineer-*-x86_64-apple-darwin.tar.gz`       |
| Linux (x86_64)        | `codineer-*-x86_64-unknown-linux-gnu.tar.gz`  |
| Linux (ARM64)         | `codineer-*-aarch64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64)      | `codineer-*-x86_64-pc-windows-msvc.zip`       |

### 从源码构建

```bash
git clone https://github.com/andeya/codineer.git
cd codineer
cargo install --path crates/codineer-cli --locked
```

---

## 快速开始

**第一步：认证**——选择一种方式：

```bash
# 云端 Provider（需要 API Key）
export ANTHROPIC_API_KEY="sk-ant-..."   # Claude（推荐）
export XAI_API_KEY="xai-..."            # Grok
export OPENAI_API_KEY="sk-..."          # GPT / OpenAI 兼容接口

# 免费云端 Provider
export OPENROUTER_API_KEY="..."         # OpenRouter（有免费模型）
export GROQ_API_KEY="..."               # Groq Cloud（慷慨的免费额度）

# 本地模型（无需 API Key）
ollama serve                            # 启动 Ollama 后运行 codineer
codineer --model ollama/qwen3-coder     # 明确指定模型

# 或通过配置文件（持久化，无需每次 export）
# ~/.codineer/settings.json:
#   { "env": { "ANTHROPIC_API_KEY": "sk-ant-..." } }

# OAuth 登录（凭证存储在系统密钥链）
codineer login
```

**第二步：初始化项目**（可选，但强烈推荐）：

```bash
cd your-project
codineer init        # 自动生成包含项目上下文的 CODINEER.md
```

**第三步：开始编码：**

```bash
codineer             # 打开交互式 REPL
```

Codineer 会自动检测可用的 API 供应商，无需额外配置。

---

## 使用指南

### 交互式 REPL

默认模式，无参数启动后直接用自然语言交流：

```bash
codineer
```

在 REPL 内可使用**斜杠命令**（支持 Tab 自动补全）：

**会话与信息**

| 命令                                   | 说明                                         |
| -------------------------------------- | -------------------------------------------- |
| `/help`                                | 显示所有可用命令                             |
| `/status`                              | 查看会话信息：模型、token 数、Git 分支、配置 |
| `/version`                             | 显示 Codineer 版本                           |
| `/model [名称]`                        | 查看或切换当前模型                           |
| `/permissions [模式]`                  | 查看或切换权限模式                           |
| `/cost`                                | 显示 token 用量和费用估算                    |
| `/compact`                             | 压缩对话历史以节省 token                     |
| `/clear [--confirm]`                   | 重置对话（需加 `--confirm` 才真正执行）      |
| `/session [list\|switch <id>]`         | 列出或切换已命名会话                         |
| `/resume <文件>`                       | 恢复已保存的会话文件                         |
| `/export [文件]`                       | 将对话导出为 Markdown                        |
| `/memory`                              | 查看已加载的 CODINEER.md 记忆文件            |
| `/config [env\|hooks\|model\|plugins]` | 查看合并后的配置                             |
| `/init`                                | 为当前项目重新生成 CODINEER.md               |

**Git 与工作流**

| 命令              | 说明                          |
| ----------------- | ----------------------------- |
| `/diff`           | 显示工作区 git diff           |
| `/branch`         | 查看或管理 git 分支           |
| `/commit`         | 创建 git 提交                 |
| `/commit-push-pr` | 提交、推送并创建 Pull Request |
| `/pr`             | 创建或管理 Pull Request       |
| `/issue`          | 创建或浏览 GitHub Issue       |
| `/worktree`       | 管理 git worktree             |

**Agent 与插件**

| 命令                                 | 说明                 |
| ------------------------------------ | -------------------- |
| `/plugin list\|install\|enable\|...` | 管理插件             |
| `/agents`                            | 列出已配置的子 Agent |
| `/skills`                            | 列出可用的 Skill     |

**高级**

| 命令               | 说明                  |
| ------------------ | --------------------- |
| `/ultraplan`       | 生成详细的实现计划    |
| `/bughunter`       | 系统化 Bug 搜寻模式   |
| `/teleport`        | 跳转到指定文件或符号  |
| `/debug-tool-call` | 调试上一次工具调用    |
| `/vim`             | 切换 Vim 风格模态编辑 |
| `/exit` 或 `/quit` | 退出 REPL             |

**键盘快捷键：**

| 按键                      | 功能                             |
| ------------------------- | -------------------------------- |
| `↑` / `↓`                 | 浏览输入历史                     |
| `Tab`                     | 循环补全斜杠命令                 |
| `Shift+Enter` 或 `Ctrl+J` | 换行（多行输入）                 |
| `Ctrl+C`                  | 取消当前输入或中断正在执行的工具 |

### 一次性提问

只执行一次提示后退出，适用于脚本和 CI：

```bash
codineer "解释这个项目的整体架构"
codineer prompt "列出所有 TODO 注释" --output-format json
codineer -p "概括 Cargo.toml 的内容" --model sonnet
```

可用参数：

| 参数                             | 说明                                |
| -------------------------------- | ----------------------------------- |
| `-p <文本>`                      | 一次性提问（后续内容为提示文本）    |
| `--model <名称>`                 | 指定模型（见[模型选择](#模型选择)） |
| `--output-format text\|json`     | 输出格式（默认 `text`）             |
| `--allowedTools <列表>`          | 逗号分隔的工具白名单（可重复指定）  |
| `--permission-mode <模式>`       | 权限级别（见[权限模式](#权限模式)） |
| `--dangerously-skip-permissions` | 跳过所有权限检查                    |
| `--version`、`-V`                | 显示版本和构建信息                  |

### 会话管理

跨终端会话保存与恢复对话：

```bash
# 在 REPL 内导出会话
/export session.json

# 稍后恢复，可同时执行斜杠命令
codineer --resume session.json
codineer --resume session.json /status /compact /cost
```

### 模型选择

Codineer 为常用模型提供短别名：

| 别名        | 实际模型                    | 供应商    |
| ----------- | --------------------------- | --------- |
| `opus`      | `claude-opus-4-6`           | Anthropic |
| `sonnet`    | `claude-sonnet-4-6`         | Anthropic |
| `haiku`     | `claude-haiku-4-5-20251213` | Anthropic |
| `grok`      | `grok-3`                    | xAI       |
| `grok-mini` | `grok-3-mini`               | xAI       |
| `gpt`       | `gpt-4o`                    | OpenAI    |
| `mini`      | `gpt-4o-mini`               | OpenAI    |
| `o3`        | `o3`                        | OpenAI    |

```bash
codineer --model opus "帮我 review 这次改动"
codineer --model grok-mini "快速问一个问题"
```

#### 自定义 Provider（OpenAI 兼容）

通过 `provider/model` 语法使用任意 OpenAI 兼容 API：

| 前缀                         | Provider    | 是否需要 API key？ |
| ---------------------------- | ----------- | ------------------- |
| `ollama/<model>`             | Ollama      | 否                  |
| `lmstudio/<model>`           | LM Studio   | 否                  |
| `groq/<model>`               | Groq Cloud  | `GROQ_API_KEY`      |
| `openrouter/<model>`         | OpenRouter  | `OPENROUTER_API_KEY` |

```bash
codineer --model ollama/qwen3-coder "重构这个模块"
codineer --model groq/llama-3.3-70b-versatile "解释这个函数"
codineer --model ollama   # 自动从 Ollama 选择最佳编码模型
```

**Ollama 零配置**：当没有任何 API key 且 Ollama 正在运行时，Codineer 会自动检测并选择最佳编码模型。

在配置文件中添加自定义 Provider：

```json
{
  "providers": {
    "ollama": { "baseUrl": "http://localhost:11434/v1" },
    "my-api": { "baseUrl": "https://my-endpoint.com/v1", "apiKeyEnv": "MY_API_KEY" }
  }
}
```

> **注意**：不支持 function calling 的模型会自动降级为纯文本模式。

会话过程中可通过 `/model <名称>` 切换模型。

在配置文件中设置持久化的默认模型：

```json
{ "model": "sonnet" }
```

未指定 `--model` 参数时，Codineer 优先使用配置中的 `model` 字段，再根据可用的 API 凭据自动检测，最后尝试检测本地 Ollama 实例。

### 权限模式

精确控制 Agent 可以使用哪些工具：

| 模式                 | 允许的操作                       |
| -------------------- | -------------------------------- |
| `read-only`          | 只读和搜索工具，不允许任何写操作 |
| `workspace-write`    | 可编辑工作区内的文件（默认）     |
| `danger-full-access` | 完全无限制，包含系统级命令       |

```bash
codineer --permission-mode read-only "对代码库做安全审计"
codineer --permission-mode danger-full-access "运行完整测试套件并修复失败"
```

会话中可通过 `/permissions <模式>` 切换权限。

### 脚本与自动化

结合 `--output-format json` 与管道命令，方便与其他工具集成：

```bash
# 提取结构化数据
codineer -p "列出 src/ 下所有公开函数" --output-format json | jq '.content[0].text'

# CI 流水线示例
codineer -p "检查安全问题" \
  --permission-mode read-only \
  --allowedTools read_file,grep_search \
  --output-format json
```

---

## 项目初始化

**`CODINEER.md`** 是项目记忆文件，用于告知 Codineer 关于代码库的约定和工作流程。自动生成方式：

```bash
codineer init
```

这将在项目根目录生成一个 `CODINEER.md`，包含检测到的技术栈、验证命令和仓库结构。建议将其提交到版本控制，与团队共享项目上下文。

示例 `CODINEER.md`：

```markdown
# CODINEER.md

## Detected stack

- Languages: Rust, TypeScript

## Verification

- `cargo test --workspace`
- `npm test`

## Working agreement

- 所有 PR 需通过 CI
- 使用 conventional commits 格式
```

Codineer 会从工作区根目录向上逐级查找匹配的指令文件：

| 文件                        | 用途                           |
| --------------------------- | ------------------------------ |
| `CODINEER.md`               | 主要项目上下文（建议提交）     |
| `CODINEER.local.md`         | 个人本地覆盖（加入 gitignore） |
| `.codineer/CODINEER.md`     | `.codineer/` 目录内的替代位置  |
| `.codineer/instructions.md` | 附加指令                       |

---

## 配置

按优先级从高到低加载：

1. `.codineer/settings.local.json` — 本地覆盖（已 gitignore，不提交）
2. `.codineer/settings.json` — 项目级配置（建议提交）
3. `.codineer.json` — 项目级扁平配置
4. `~/.codineer/settings.json` — 用户全局配置
5. `~/.codineer.json` — 用户全局扁平配置

随时查看合并后的配置：

```bash
/config          # 完整合并配置
/config env      # 环境变量部分
/config model    # 模型配置
/config plugins  # 插件配置
/config hooks    # Hook 配置
```

**常用环境变量：**

| 变量                       | 用途                                 |
| -------------------------- | ------------------------------------ |
| `ANTHROPIC_API_KEY`        | Claude API Key                       |
| `ANTHROPIC_AUTH_TOKEN`     | Bearer Token（API Key 的替代方式）   |
| `XAI_API_KEY`              | xAI / Grok API Key                   |
| `OPENAI_API_KEY`           | OpenAI API Key                       |
| `OPENROUTER_API_KEY`       | OpenRouter API Key（有免费模型）     |
| `GROQ_API_KEY`             | Groq Cloud API Key（免费额度可用）   |
| `CODINEER_WORKSPACE_ROOT`  | 覆盖工作区根路径                     |
| `CODINEER_CONFIG_HOME`     | 覆盖配置目录（默认 `~/.codineer`）   |
| `CODINEER_PERMISSION_MODE` | 默认权限模式                         |
| `NO_COLOR`                 | 禁用 ANSI 颜色输出                   |

> **注意：** API 密钥可通过环境变量、`settings.json` 的 `"env"` 字段或 OAuth（`codineer login`）配置。显式环境变量始终优先于配置文件中的值。

---

## 扩展 Codineer

### MCP 服务器

Codineer 支持 [Model Context Protocol](https://modelcontextprotocol.io) 来接入外部工具。在配置文件中添加 MCP 服务器：

```json
{
  "mcpServers": {
    "my-server": {
      "command": "node",
      "args": ["path/to/mcp-server.js"],
      "type": "stdio"
    }
  }
}
```

支持的传输类型：`stdio`（默认）、`sse`、`http`、`ws`（或 `websocket`）。

### 插件

插件可添加自定义工具和 Hook。通过 REPL 管理：

```bash
/plugin list                          # 列出已安装的插件
/plugin install ./path/to/plugin      # 安装本地插件
/plugin enable my-plugin              # 启用插件
/plugin disable my-plugin             # 禁用插件
/plugin update my-plugin              # 更新到最新版
/plugin uninstall my-plugin-id        # 卸载插件
```

### Agent 与 Skill

**Agent** 是针对专项任务的命名子 Agent 配置：

```bash
codineer agents          # 列出已配置的 Agent
codineer agents --help   # 查看 Agent 帮助
/agents                  # 在 REPL 内同效
```

**Skill** 是可复用提示模板。Codineer 会在 `.codineer/skills/`、`$CODINEER_CONFIG_HOME/skills/` 和 `~/.codineer/skills/` 中搜索：

```bash
codineer skills          # 列出可用 Skill
codineer /skills help    # 查看 Skill 详情
/skills                  # 在 REPL 内同效
```

---

## 内置工具

Codineer 自带一套丰富的工具供 AI 调用：

| 工具               | 说明                                   |
| ------------------ | -------------------------------------- |
| `bash`             | 执行 Shell 命令                        |
| `PowerShell`       | 执行 PowerShell 命令（Windows）        |
| `read_file`        | 读取文件内容，支持偏移和行数限制       |
| `write_file`       | 创建或覆盖文件                         |
| `edit_file`        | 对文件做精准字符串替换                 |
| `glob_search`      | 按 Glob 模式查找文件                   |
| `grep_search`      | 使用正则表达式搜索文件内容             |
| `WebFetch`         | 抓取并摘要网页内容                     |
| `WebSearch`        | 通过 DuckDuckGo 搜索网络               |
| `NotebookEdit`     | 编辑 Jupyter Notebook 单元格           |
| `TodoWrite`        | 管理结构化任务列表                     |
| `Agent`            | 启动子 Agent 处理复杂任务              |
| `Skill`            | 加载并执行 Skill 提示模板              |
| `ToolSearch`       | 按关键词搜索可用工具                   |
| `REPL`             | 运行持久化语言 REPL（Python、Node 等） |
| `Sleep`            | 暂停执行指定时长                       |
| `SendUserMessage`  | 向用户发送消息                         |
| `Config`           | 读取或写入配置值                       |
| `StructuredOutput` | 返回结构化 JSON 输出                   |

---

## 发布到 crates.io

所有 crate 使用 `codineer-` 前缀发布到 crates.io。发布流程通过 GitHub Actions 自动化——打标签即可触发：

```bash
git tag v0.6.0
git push origin v0.6.0
```

| Crate               | 说明                      |
| ------------------- | ------------------------- |
| `codineer-cli`      | CLI 二进制 — **安装这个** |
| `codineer-runtime`  | 核心运行时引擎            |
| `codineer-api`      | AI 供应商 API 客户端      |
| `codineer-tools`    | 内置工具定义与执行        |
| `codineer-plugins`  | 插件系统和 Hook           |
| `codineer-commands` | 斜杠命令和发现            |
| `codineer-lsp`      | LSP 客户端集成            |

> **注意：** 库 crate 是 `codineer-cli` 的内部实现细节。它们发布到 crates.io 是为了满足依赖要求，其 API 不保证对外稳定。

---

## 许可证

[MIT](LICENSE)

---

<p align="center">
  由 <a href="https://github.com/andeya">andeya</a> 使用 🦀 构建
</p>
