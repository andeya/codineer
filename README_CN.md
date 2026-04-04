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

安全 Rust 构建，**单个约 15 MB 二进制文件**。无守护进程，无运行时依赖——带上任意模型即可开始。

## 为什么选择 Codineer？

大多数 AI 编程 CLI 将你绑定在单一 Provider 上。Claude Code 依赖 Anthropic，Codex CLI 依赖 OpenAI。**Codineer 支持所有 Provider——包括本地模型。**

|                                                    |   Codineer   | Claude Code  |    Codex CLI    |   Aider    |
| -------------------------------------------------- | :----------: | :----------: | :-------------: | :--------: |
| **多 Provider**（Anthropic、OpenAI、xAI、Ollama…） | **全部内置** | 仅 Anthropic | OpenAI + Ollama |    支持    |
| **零配置本地 AI**（自动检测 Ollama）               |   **支持**   |    不支持    |  `--oss` 参数   | 需手动配置 |
| **单一二进制**（无运行时依赖）                     |   **Rust**   |   Node.js    |     Node.js     |   Python   |
| **MCP 协议**（外部工具集成）                       |   **支持**   |     支持     |      支持       |    支持    |
| **插件系统** + Agent + Skill                       |   **支持**   |     支持     |     不支持      |   不支持   |
| **权限模式**（只读 → 完全访问）                    |   **支持**   |     支持     |      支持       |    部分    |
| **工具调用降级**（优雅降级）                       |   **支持**   |    不适用    |     不适用      |   不适用   |
| **Git 工作流**（/commit、/pr、/diff、/branch）     |   **内置**   |   通过工具   |    通过工具     |  自动提交  |
| **Vim 模式**                                       |   **支持**   |    不支持    |     不支持      |   不支持   |
| **CI/CD 就绪**（JSON 输出、工具白名单）            |   **支持**   |     支持     |      支持       |    有限    |

**核心优势：**

- **Provider 自由** — 用 `--model` 在 Claude、GPT、Grok、Ollama、LM Studio、OpenRouter、Groq 或任何 OpenAI 兼容 API 间切换。零厂商锁定。
- **零配置本地 AI** — 启动 Ollama，运行 `codineer`。自动检测本地模型并选择最适合编程的那个。
- **即刻启动** — `brew install` 或 `cargo install`。一个 Rust 二进制文件，无运行时依赖。
- **优雅降级** — 不支持 function calling 的模型自动降级为纯文本模式。
- **项目记忆** — `CODINEER.md` 让 AI 拥有关于代码库的持久上下文。提交到仓库，与团队共享。

## 目录

- [安装](#安装)
- [快速开始](#快速开始)
- [模型与 Provider](#模型与-provider)
- [使用方法](#使用方法)
- [配置](#配置)
- [项目上下文](#项目上下文)
- [扩展 Codineer](#扩展-codineer)
- [参考](#参考)
- [许可证](#许可证)

---

## 安装

```bash
brew install andeya/codineer/codineer            # Homebrew（macOS / Linux）
cargo install codineer-cli                        # Cargo（从 crates.io）
```

或从 [Releases](https://github.com/andeya/codineer/releases) 下载预编译包：

| 平台                  | 文件                                          |
| --------------------- | --------------------------------------------- |
| macOS (Apple Silicon) | `codineer-*-aarch64-apple-darwin.tar.gz`      |
| macOS (Intel)         | `codineer-*-x86_64-apple-darwin.tar.gz`       |
| Linux (x86_64)        | `codineer-*-x86_64-unknown-linux-gnu.tar.gz`  |
| Linux (ARM64)         | `codineer-*-aarch64-unknown-linux-gnu.tar.gz` |
| Windows (x86_64)      | `codineer-*-x86_64-pc-windows-msvc.zip`       |

<details><summary>从源码构建</summary>

```bash
git clone https://github.com/andeya/codineer.git
cd codineer
cargo install --path crates/codineer-cli --locked
```

</details>

---

## 快速开始

```bash
# 1. 选择一种 Provider：
export ANTHROPIC_API_KEY="sk-ant-..."             # Claude
export OPENAI_API_KEY="sk-..."                    # GPT
export XAI_API_KEY="xai-..."                      # Grok
export OPENROUTER_API_KEY="..."                   # OpenRouter（免费模型）
export GROQ_API_KEY="..."                         # Groq Cloud（免费额度）
ollama serve                                      # 本地 AI（无需 Key）
codineer login                                    # 或 OAuth

# 2. 初始化项目上下文（可选）
codineer init

# 3. 开始编码
codineer                                          # 交互式 REPL
codineer "解释一下这个项目"                       # 一次性提问
```

Codineer 自动检测可用 Provider，无需额外参数。所有凭据也可写入 [settings.json](#配置) 代替 shell export。

---

## 模型与 Provider

### 内置别名

| 别名        | 模型                        | Provider  |
| ----------- | --------------------------- | --------- |
| `opus`      | `claude-opus-4-6`           | Anthropic |
| `sonnet`    | `claude-sonnet-4-6`         | Anthropic |
| `haiku`     | `claude-haiku-4-5-20251213` | Anthropic |
| `grok`      | `grok-3`                    | xAI       |
| `grok-mini` | `grok-3-mini`               | xAI       |
| `grok-2`    | `grok-2`                    | xAI       |
| `gpt`       | `gpt-4o`                    | OpenAI    |
| `mini`      | `gpt-4o-mini`               | OpenAI    |
| `o3`        | `o3`                        | OpenAI    |
| `o3-mini`   | `o3-mini`                   | OpenAI    |

```bash
codineer --model opus "帮我 review 这次改动"
codineer --model grok-mini "快速问一个问题"
```

### 自定义 Provider（OpenAI 兼容）

通过 `provider/model` 语法使用任意 OpenAI 兼容 API：

| 前缀                 | Provider   | API key              |
| -------------------- | ---------- | -------------------- |
| `ollama/<model>`     | Ollama     | —                    |
| `lmstudio/<model>`   | LM Studio  | —                    |
| `groq/<model>`       | Groq Cloud | `GROQ_API_KEY`       |
| `openrouter/<model>` | OpenRouter | `OPENROUTER_API_KEY` |

```bash
codineer --model ollama/qwen3-coder "重构这个模块"
codineer --model groq/llama-3.3-70b-versatile "解释这个"
codineer --model ollama              # 自动选择最佳编程模型
```

**Ollama 零配置**：当没有 API key 且 Ollama 正在运行时，Codineer 自动检测并选择最佳编程模型。支持 `OLLAMA_HOST` 环境变量和远程实例（见[配置](#环境变量)）。

> 不支持 function calling 的模型自动降级为纯文本模式——任何模型都能工作。

### 模型解析顺序

未指定 `--model` 时：

1. settings.json 中的 `model` 字段
2. 根据可用 API 凭据自动检测
3. 检测运行中的 Ollama 实例

会话中切换模型：`/model <名称>`

---

## 使用方法

### 交互式 REPL

```bash
codineer
```

用自然语言交流。支持**斜杠命令**（Tab 自动补全）：

| 分类      | 命令                                                                     |
| --------- | ------------------------------------------------------------------------ |
| **信息**  | `/help` `/status` `/version` `/model` `/cost` `/config` `/memory`        |
| **会话**  | `/compact` `/clear` `/session` `/resume` `/export`                       |
| **Git**   | `/diff` `/branch` `/commit` `/commit-push-pr` `/pr` `/issue` `/worktree` |
| **Agent** | `/agents` `/skills` `/plugin`                                            |
| **高级**  | `/ultraplan` `/bughunter` `/teleport` `/debug-tool-call` `/vim`          |
| **导航**  | `/init` `/permissions` `/exit`                                           |

**快捷键：** `↑`/`↓` 历史记录、`Tab` 补全、`Shift+Enter` 换行、`Ctrl+C` 取消。

### 一次性提问

```bash
codineer "解释这个项目的架构"
codineer -p "列出所有 TODO" --output-format json
codineer --model sonnet --permission-mode read-only "审计代码"
```

| 参数                         | 说明                                                 |
| ---------------------------- | ---------------------------------------------------- |
| `-p <文本>`                  | 一次性提问                                           |
| `--model <名称>`             | 选择模型                                             |
| `--output-format text\|json` | 输出格式                                             |
| `--allowedTools <列表>`      | 限制工具访问（逗号分隔）                             |
| `--permission-mode <模式>`   | `read-only`、`workspace-write`、`danger-full-access` |
| `--resume <文件>`            | 恢复已保存的会话                                     |
| `-V`、`--version`            | 显示版本                                             |

### 权限模式

| 模式                 | 允许                     |
| -------------------- | ------------------------ |
| `read-only`          | 只读和搜索，不允许写操作 |
| `workspace-write`    | 编辑工作区内文件（默认） |
| `danger-full-access` | 完全无限制，包含系统命令 |

### 脚本与 CI

```bash
codineer -p "检查安全问题" \
  --permission-mode read-only \
  --allowedTools read_file,grep_search \
  --output-format json | jq '.content[0].text'
```

---

## 配置

### 配置文件

Codineer 从多个 JSON 文件合并设置（优先级从高到低）：

| 文件                            | 作用域              | 是否提交         |
| ------------------------------- | ------------------- | ---------------- |
| `.codineer/settings.local.json` | 项目 — 本地覆盖     | 否（gitignored） |
| `.codineer/settings.json`       | 项目 — 团队配置     | 是               |
| `.codineer.json`                | 项目 — 扁平配置     | 是               |
| `~/.codineer/settings.json`     | 用户 — 全局         | —                |
| `~/.codineer.json`              | 用户 — 全局扁平配置 | —                |

所有文件使用相同 schema。`env`、`providers`、`mcpServers` 等对象跨层级深度合并。

### 配置参考

```json
{
  "model": "sonnet",
  "permissionMode": "workspace-write",
  "env": {
    "ANTHROPIC_API_KEY": "sk-ant-...",
    "OLLAMA_HOST": "http://192.168.1.100:11434"
  },
  "providers": {
    "ollama": { "baseUrl": "http://my-server:11434/v1" },
    "my-api": { "baseUrl": "https://api.example.com/v1", "apiKeyEnv": "MY_KEY" }
  },
  "mcpServers": { "my-server": { "command": "node", "args": ["server.js"] } },
  "plugins": ["my-plugin"],
  "hooks": { "PreToolUse": ["lint-check"], "PostToolUse": ["notify"] }
}
```

| 字段             | 类型   | 说明                                                         |
| ---------------- | ------ | ------------------------------------------------------------ |
| `model`          | string | 默认模型（如 `"sonnet"`、`"ollama/qwen3-coder"`）            |
| `permissionMode` | string | `"read-only"`、`"workspace-write"` 或 `"danger-full-access"` |
| `env`            | object | 启动时注入的环境变量。Shell export 优先。                    |
| `providers`      | object | 自定义 OpenAI 兼容 Provider 端点                             |
| `mcpServers`     | object | MCP 服务器定义（stdio、sse、http、ws）                       |
| `plugins`        | array  | 要加载的插件名称                                             |
| `hooks`          | object | `PreToolUse` / `PostToolUse` Hook 的 Shell 命令              |

运行时查看合并配置：`/config`、`/config env`、`/config model`

### 环境变量

通过 Shell export **或** settings.json 的 `"env"` 字段设置（Shell export 优先）：

| 变量                       | 用途                                           |
| -------------------------- | ---------------------------------------------- |
| `ANTHROPIC_API_KEY`        | Claude API Key                                 |
| `ANTHROPIC_AUTH_TOKEN`     | Bearer Token（替代方式）                       |
| `XAI_API_KEY`              | xAI / Grok API Key                             |
| `OPENAI_API_KEY`           | OpenAI API Key                                 |
| `OPENROUTER_API_KEY`       | OpenRouter API Key                             |
| `GROQ_API_KEY`             | Groq Cloud API Key                             |
| `OLLAMA_HOST`              | Ollama 端点（如 `http://192.168.1.100:11434`） |
| `CODINEER_WORKSPACE_ROOT`  | 覆盖工作区根路径                               |
| `CODINEER_CONFIG_HOME`     | 覆盖配置目录（`~/.codineer`）                  |
| `CODINEER_PERMISSION_MODE` | 默认权限模式                                   |
| `NO_COLOR`                 | 禁用 ANSI 颜色                                 |

**凭据优先级：** Shell 环境变量 → settings.json `"env"` → OAuth（`codineer login`）

---

## 项目上下文

`CODINEER.md` 是项目记忆文件，告诉 AI 你的代码库约定和工作流。

```bash
codineer init        # 根据检测到的技术栈自动生成
```

Codineer 沿目录树向上查找并加载所有匹配的指令文件：

| 文件                        | 用途                       |
| --------------------------- | -------------------------- |
| `CODINEER.md`               | 主要上下文（建议提交）     |
| `CODINEER.local.md`         | 个人覆盖（加入 gitignore） |
| `.codineer/CODINEER.md`     | 替代位置                   |
| `.codineer/instructions.md` | 附加指令                   |

---

## 扩展 Codineer

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

```bash
/plugin list                        # 列出已安装
/plugin install ./path/to/plugin    # 安装本地插件
/plugin enable my-plugin            # 启用
```

### Agent 与 Skill

**Agent** 是针对专项任务的命名子 Agent 配置。**Skill** 是可复用的提示模板。

```bash
codineer agents          # 列出 Agent
codineer skills          # 列出 Skill
/agents                  # REPL 内
/skills                  # REPL 内
```

Skill 搜索路径：`.codineer/skills/`、`~/.codineer/skills/`、`$CODINEER_CONFIG_HOME/skills/`。

---

## 参考

### 内置工具

| 工具               | 说明                            |
| ------------------ | ------------------------------- |
| `bash`             | 执行 Shell 命令                 |
| `PowerShell`       | 执行 PowerShell 命令（Windows） |
| `read_file`        | 读取文件内容                    |
| `write_file`       | 创建或覆盖文件                  |
| `edit_file`        | 精准字符串替换                  |
| `glob_search`      | 按 Glob 模式查找文件            |
| `grep_search`      | 正则搜索文件内容                |
| `WebFetch`         | 抓取并摘要网页                  |
| `WebSearch`        | DuckDuckGo 搜索                 |
| `NotebookEdit`     | 编辑 Jupyter Notebook           |
| `TodoWrite`        | 管理任务列表                    |
| `Agent`            | 启动子 Agent                    |
| `Skill`            | 执行 Skill 模板                 |
| `REPL`             | 运行 Python、Node 或 Shell      |
| `ToolSearch`       | 搜索可用工具                    |
| `Sleep`            | 暂停执行指定时长                |
| `SendUserMessage`  | 向用户发送消息                  |
| `Config`           | 读写配置值                      |
| `StructuredOutput` | 返回结构化 JSON                 |

### Crate 结构

所有 crate 发布到 crates.io。安装 `codineer-cli`——其余为内部依赖。

| Crate               | 角色                       |
| ------------------- | -------------------------- |
| `codineer-cli`      | CLI 二进制（**安装这个**） |
| `codineer-runtime`  | 核心运行时引擎             |
| `codineer-api`      | AI Provider API 客户端     |
| `codineer-tools`    | 工具定义与执行             |
| `codineer-plugins`  | 插件系统和 Hook            |
| `codineer-commands` | 斜杠命令                   |
| `codineer-lsp`      | LSP 客户端集成             |

---

## 许可证

[MIT](LICENSE)

---

<p align="center">
  由 <a href="https://github.com/andeya">andeya</a> 使用 🦀 构建
</p>
