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

<p align="center">
  <img src="https://raw.githubusercontent.com/andeya/codineer/main/assets/ScreenShot_01.png" alt="Codineer REPL 截图" width="780">
</p>

## 为什么选择 Codineer？

大多数 AI 编程 CLI 将你绑定在单一 Provider 上。Claude Code 依赖 Anthropic，Codex CLI 依赖 OpenAI。**Codineer 支持所有 Provider——包括本地模型。**

|                                                                                   |   Codineer   | Claude Code  |    Codex CLI    |   Aider    |
| --------------------------------------------------------------------------------- | :----------: | :----------: | :-------------: | :--------: |
| **多 Provider**（Anthropic、OpenAI、xAI、Ollama…）                                | **全部内置** | 仅 Anthropic | OpenAI + Ollama |    支持    |
| **零 Token 成本**（[免费使用主流模型](#openclaw-zero-token免费使用主流-ai-模型)） |   **支持**   |    不支持    |     不支持      |   不支持   |
| **零配置本地 AI**（自动检测 Ollama）                                              |   **支持**   |    不支持    |  `--oss` 参数   | 需手动配置 |
| **单一二进制**（无运行时依赖）                                                    |   **Rust**   |   Node.js    |     Node.js     |   Python   |
| **多模态输入**（`@image.png`、剪贴板粘贴、拖拽上传）                              |   **支持**   |     支持     |      有限       |    有限    |
| **MCP 协议**（外部工具集成）                                                      |   **支持**   |     支持     |      支持       |    支持    |
| **插件系统** + Agent + Skill                                                      |   **支持**   |     支持     |     不支持      |   不支持   |
| **权限模式**（只读 → 完全访问）                                                   |   **支持**   |     支持     |      支持       |    部分    |
| **工具调用降级**（优雅降级）                                                      |   **支持**   |    不适用    |     不适用      |   不适用   |
| **Git 工作流**（/commit、/pr、/diff、/branch）                                    |   **内置**   |   通过工具   |    通过工具     |  自动提交  |
| **Vim 模式**                                                                      |   **支持**   |    不支持    |     不支持      |   不支持   |
| **CI/CD 就绪**（JSON 输出、工具白名单）                                           |   **支持**   |     支持     |      支持       |    有限    |

**核心优势：**

- **Provider 自由** — 用 `--model` 在 Claude、GPT、Grok、Ollama、LM Studio、OpenRouter、Groq 或任何 OpenAI 兼容 API 间切换。零厂商锁定。
- **零 Token 成本** — 搭配 [OpenClaw Zero Token](https://github.com/andeya/openclaw-zero-token) 网关，免费使用 Claude、ChatGPT、Gemini、DeepSeek 等 10+ 主流模型，无需购买任何 API Key。
- **零配置本地 AI** — 启动 Ollama，运行 `codineer`。自动检测本地模型并选择最适合编程的那个。
- **即刻启动** — `brew install` 或 `cargo install`。一个 Rust 二进制文件，无运行时依赖。
- **多模态输入** — 通过 `@photo.png` 附加图片、从剪贴板粘贴（`Ctrl+V` / `/image`）或将文件拖拽到终端。支持 Anthropic、OpenAI 及所有兼容多模态的 Provider。
- **优雅降级** — 不支持 function calling 的模型自动降级为纯文本模式。
- **项目记忆** — `.codineer/CODINEER.md` 让 AI 拥有关于代码库的持久上下文。提交到仓库，与团队共享。
- **自适应终端 UI** — 欢迎面板和分割线随窗口宽度实时调整。超窄终端自动切换为单列布局；拖动窗口时输入行原位重绘，无闪烁。兼容 macOS、Linux 和 Windows（Windows Terminal / ConPTY）。

## 目录

- [安装](#安装)
- [快速开始](#快速开始)
- [模型与 Provider](#模型与-provider)
- [使用方法](#使用方法)
  - [文件与图片附件](#文件与图片附件)
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
export GEMINI_API_KEY="AIzaSy..."                  # Google Gemini（aistudio.google.com 免费申请）
export DASHSCOPE_API_KEY="sk-..."                 # 阿里云通义 DashScope（兼容 OpenAI）
ollama serve                                      # 本地 AI（无需 Key）
# 或通过 OpenClaw Zero Token 网关免费使用所有主流模型（见下文）
codineer login                                    # 或 OAuth 登录（默认 Provider）
codineer login anthropic --source claude-code     # 使用 Claude Code 凭据
codineer status                                   # 查看认证状态
codineer config set model sonnet                   # 设置默认模型（别名或全名）

# 2. 初始化项目上下文（可选）
codineer init

# 3. 开始编码
codineer                                          # 交互式 REPL
codineer "解释一下这个项目"                       # 一次性提问
```

Codineer 自动检测可用 Provider，无需额外参数。所有凭据也可写入 [settings.json](#配置) 代替 shell export。

---

## 模型与 Provider

### 模型别名

在 `settings.json` 中定义自己的模型短名：

```json
{
  "modelAliases": {
    "sonnet": "claude-sonnet-4-6",
    "opus": "claude-opus-4-6",
    "haiku": "claude-haiku-4-5-20251213",
    "grok": "grok-3",
    "gpt": "gpt-4o",
    "flash": "gemini/gemini-2.5-flash"
  }
}
```

```bash
codineer --model sonnet "帮我 review 这次改动"
codineer --model flash "快速问一个问题"
```

不内置任何别名——由你决定哪些短名适合自己的工作流。查看已配置的别名：`codineer models`。

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

### Google Gemini（OpenAI 兼容，免费 API Key）

在 [Google AI Studio](https://aistudio.google.com/apikey) 免费申请 API Key，无需绑定信用卡。在 `settings.json` 中配置：

```json
{
  "model": "gemini/gemini-2.5-flash",
  "env": {
    "GEMINI_API_KEY": "AIzaSy..."
  },
  "providers": {
    "gemini": {
      "baseUrl": "https://generativelanguage.googleapis.com/v1beta/openai",
      "apiKeyEnv": "GEMINI_API_KEY",
      "defaultModel": "gemini-2.5-flash",
      "models": ["gemini-2.5-flash", "gemini-2.5-pro"]
    }
  }
}
```

```bash
codineer --model gemini/gemini-2.5-flash "解释这段代码"
codineer --model gemini/gemini-2.5-pro "审查架构设计"
```

> **注意：** 必须使用 OpenAI 兼容端点（`/v1beta/openai`），而非 Gemini 原生 REST API（`/v1beta/models/...:generateContent`）。Codineer 会在 baseUrl 后追加 `/chat/completions`。

### 阿里云通义（DashScope，OpenAI 兼容）

使用 `provider/model` 形式，并在 `settings.json` 的 `providers` 中配置 `baseUrl`（国内与国际域名以[官方文档](https://help.aliyun.com/zh/model-studio/)为准）：

```bash
export DASHSCOPE_API_KEY="sk-..."
codineer --model dashscope/qwen-plus-2025-07-28 "用一句话解释 Rust 所有权"
```

流式响应若只携带 `reasoning_content` / `thought` 等扩展字段，当前版本会一并解析；若仍出现 **assistant stream produced no content**，CLI 会自动再发**一次非流式**请求作为补偿。请尽量使用**从源码或最新 Release 构建的二进制**，旧版本可能缺少上述逻辑。

### Azure OpenAI

在对应 `providers.<name>` 下设置 `apiVersion`（例如 `2024-02-15-preview`），Codineer 会将其拼为 `api-version=...` 附加到 `.../chat/completions` 请求 URL。完整示例见仓库根目录 [`settings.example.json`](https://github.com/andeya/codineer/blob/main/settings.example.json) 中的 `azure-openai` 条目。

### 列出可用模型

```bash
codineer models               # 所有 Provider
codineer models anthropic     # 按 Provider 筛选
codineer models ollama        # 显示本地 Ollama 模型
```

### 模型解析顺序

未指定 `--model` 时：

1. settings.json 中的 `model` 字段
2. 根据可用 API 凭据自动检测
3. 检测运行中的 Ollama 实例

若解析出的模型缺少凭据，Codineer 会依次尝试 `fallbackModels` 中的每个模型。

会话中切换模型：`/model <名称>`

### 模型回退

在 `settings.json` 中设置有序的回退模型列表。当主模型不可用（如缺少 API key）时，Codineer 依序尝试每个回退模型：

```json
{
  "model": "sonnet",
  "modelAliases": { "sonnet": "claude-sonnet-4-6" },
  "fallbackModels": ["ollama/qwen3-coder", "groq/llama-3.3-70b-versatile"]
}
```

`model` 和 `fallbackModels` 都支持 `modelAliases` 中定义的自定义别名。零成本设置特别有用：将云端模型设为主模型，本地模型设为回退。

### OpenClaw Zero Token（免费使用主流 AI 模型）

> **零 API Token 成本** — 通过浏览器登录，一键聚合 Claude、ChatGPT、Gemini、DeepSeek、Qwen、Kimi、Grok 等 10+ 主流大模型，完全免费调用。

[OpenClaw Zero Token](https://github.com/andeya/openclaw-zero-token) 是一个 AI 网关，它通过驱动各大模型的官方 Web 端（浏览器登录）来替代付费 API Key。只要你能在浏览器中正常使用这些模型，就可以通过 Codineer 统一调用——无需购买任何 API Token。

**亮点：**

| 传统方式             | OpenClaw Zero Token 方式 |
| -------------------- | ------------------------ |
| 购买 API Token       | **完全免费**             |
| 按请求付费           | 无强制配额               |
| 需要绑定信用卡       | 仅需浏览器登录           |
| API Token 有泄露风险 | 凭据仅本地存储           |

**支持的模型：** Claude Web、ChatGPT Web、Gemini Web、DeepSeek Web、Qwen Web（国际/国内）、Kimi、Doubao、Grok Web、GLM Web、Xiaomi MiMo 等，其中 11/13 个 Web 模型支持工具调用。

**配置方法：**

1. 部署并启动 [OpenClaw Zero Token](https://github.com/andeya/openclaw-zero-token) 网关（默认端口 3001）
2. 在 `settings.json` 中添加 provider：

```json
{
  "model": "openclaw-zero/openclaw",
  "env": {
    "OPENCLAW_ZERO_API_KEY": "your-gateway-token"
  },
  "providers": {
    "openclaw-zero": {
      "baseUrl": "http://127.0.0.1:3001/v1",
      "apiKeyEnv": "OPENCLAW_ZERO_API_KEY",
      "defaultModel": "openclaw",
      "models": ["openclaw"],
      "headers": {
        "x-openclaw-scopes": "operator.write"
      }
    }
  }
}
```

3. 开始使用：

```bash
codineer --model openclaw-zero/openclaw "帮我重构这个模块"
codineer --model openclaw-zero/claude-web/claude-sonnet-4-6 "代码 review"
codineer --model openclaw-zero/deepseek-web/deepseek-chat "解释这段代码"
```

> `headers` 字段为可选配置，用于向 Provider 发送的每个请求附加自定义 HTTP 头。在 OpenClaw Zero Token 场景中，`x-openclaw-scopes` 用于控制权限范围。

---

## 使用方法

### 交互式 REPL

```bash
codineer
```

启动后会显示**带边框的欢迎摘要**（工作区、目录、模型、会话与 `codineer --resume …` 提示），主提示符为 **`❯`**。用自然语言交流。支持**斜杠命令**（Tab 自动补全）：

| 分类      | 命令                                                                     |
| --------- | ------------------------------------------------------------------------ |
| **信息**  | `/help` `/status` `/version` `/model` `/cost` `/config` `/memory`        |
| **会话**  | `/compact` `/clear` `/session` `/resume` `/export`                       |
| **Git**   | `/diff` `/branch` `/commit` `/commit-push-pr` `/pr` `/issue` `/worktree` |
| **Agent** | `/agents` `/skills` `/plugin`                                            |
| **高级**  | `/ultraplan` `/bughunter` `/teleport` `/debug-tool-call` `/vim`          |
| **导航**  | `/init` `/permissions` `/exit`                                           |

**快捷键：**

| 快捷键                               | 功能                                                                                        |
| ------------------------------------ | ------------------------------------------------------------------------------------------- |
| `?`                                  | 内联快捷键参考面板                                                                          |
| `!<命令>`                            | Bash 模式 — 向 AI 发送 shell 命令执行请求                                                   |
| `@`                                  | 文件 / 图片附件（Tab 补全路径；`@img.png` → 图片块）                                        |
| `Ctrl+V` / `/image`                  | 粘贴剪贴板图片 → 插入 `[Image #N]` 占位符（`Ctrl+V` 适用 macOS/Linux；`/image` 全平台通用） |
| `/`                                  | 斜杠命令补全（配合 Tab）                                                                    |
| `↑` / `↓`                            | 历史记录回溯                                                                                |
| `Shift+Enter`、`Ctrl+J`、`\ + Enter` | 插入换行                                                                                    |
| `Ctrl+C`                             | 取消输入；空提示符下连按两次退出                                                            |
| `Ctrl+D`                             | 退出（空提示符下）                                                                          |
| `双击 Esc`                           | 清空输入                                                                                    |
| `/vim`                               | 切换 Vim 模态编辑                                                                           |

### 文件与图片附件

使用 `@` 前缀将上下文直接附加到消息中：

| 语法                 | 效果                                           |
| -------------------- | ---------------------------------------------- |
| `@src/main.rs`       | 注入文件内容（最多 2000 行）                   |
| `@src/main.rs:10-50` | 注入指定行范围                                 |
| `@src/`              | 列出目录内容                                   |
| `@photo.png`         | 以多模态图片块（base64）附加                   |
| `@archive.bin`       | 注入二进制文件元数据（大小、类型），不读取内容 |

**剪贴板与拖拽图片：**

| 输入                        | 效果                                                          |
| --------------------------- | ------------------------------------------------------------- |
| 剪贴板中有图片时按 `Ctrl+V` | 插入 `[Image #N]` 占位符，提交时随消息一起发送（macOS/Linux） |
| `/image`                    | 读取剪贴板图片并插入 `[Image #N]` 占位符——**所有平台**通用    |
| 将图片文件路径拖拽到终端    | 自动识别并作为图片块附加                                      |

> **平台说明：**
>
> - **macOS** — 请用 `Ctrl+V`（而非 `Cmd+V`）。Terminal.app 和 iTerm2 会拦截 `Cmd+V`，无法将图片数据转发给应用。
> - **Linux** — `Ctrl+V` 在大多数终端（gnome-terminal、konsole、kitty 等）中正常工作。
> - **Windows** — Windows Terminal 会拦截 `Ctrl+V` 用于文本粘贴，请改用 `/image`。

图片以正确的多模态内容格式发送——Anthropic 使用 `source: base64`，OpenAI 兼容 Provider 使用 `image_url` data-URL。支持格式：PNG、JPEG、GIF、WebP、BMP。每张图片最大 20 MB。

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

| 文件                            | 路径示意（相对项目根 / 家目录）          | 作用域          | 是否提交         |
| ------------------------------- | ---------------------------------------- | --------------- | ---------------- |
| `.codineer/settings.local.json` | `<项目根>/.codineer/settings.local.json` | 项目 — 本地覆盖 | 否（gitignored） |
| `.codineer/settings.json`       | `<项目根>/.codineer/settings.json`       | 项目配置        | 是               |
| `~/.codineer/settings.json`     | `$HOME/.codineer/settings.json`          | 用户 — 全局配置 | —                |

所有文件使用相同 schema。`env`、`providers`、`mcpServers` 等对象跨层级深度合并；`mcpServers` 中同名服务器以后加载的文件为准（完整替换，不深度合并）。`codineer config set` 始终写入全局文件（`~/.codineer/settings.json`）。

### 配置参考

> **完整字段示例：** [`settings.example.json`](https://github.com/andeya/codineer/blob/main/settings.example.json)

```json
{
  "model": "sonnet",
  "modelAliases": {
    "sonnet": "claude-sonnet-4-6",
    "flash": "gemini/gemini-2.5-flash"
  },
  "permissionMode": "workspace-write",
  "env": {
    "ANTHROPIC_API_KEY": "sk-ant-...",
    "OLLAMA_HOST": "http://192.168.1.100:11434"
  },
  "providers": {
    "ollama": { "baseUrl": "http://my-server:11434/v1" },
    "my-api": {
      "baseUrl": "https://api.example.com/v1",
      "apiKeyEnv": "MY_KEY",
      "headers": { "X-Custom": "value" }
    }
  },
  "mcpServers": { "my-server": { "command": "node", "args": ["server.js"] } },
  "enabledPlugins": { "my-plugin@external": true },
  "hooks": { "PreToolUse": ["lint-check"], "PostToolUse": ["notify"] }
}
```

| 字段             | 类型     | 说明                                                                                                                                                                                                                                |
| ---------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `model`          | string   | 默认模型——别名或全名（如 `"sonnet"`、`"claude-sonnet-4-6"`、`"ollama/qwen3-coder"`）                                                                                                                                                |
| `modelAliases`   | object   | 自定义模型短名映射到完整模型 ID（如 `{"sonnet": "claude-sonnet-4-6"}`）                                                                                                                                                             |
| `fallbackModels` | string[] | 主模型不可用时依序尝试的回退模型列表                                                                                                                                                                                                |
| `permissionMode` | string   | `"read-only"`、`"workspace-write"` 或 `"danger-full-access"`                                                                                                                                                                        |
| `env`            | object   | 启动时注入的环境变量。Shell export 优先。                                                                                                                                                                                           |
| `providers`      | object   | 自定义 OpenAI 兼容 Provider：`baseUrl`、`apiKey` / `apiKeyEnv`、可选 **`apiVersion`**（Azure 等）、**`headers`**（自定义请求头）、`defaultModel` 等（见[示例](https://github.com/andeya/codineer/blob/main/settings.example.json)） |
| `oauth`          | object   | 自定义 OAuth 配置（clientId、authorizeUrl、tokenUrl、scopes 等）                                                                                                                                                                    |
| `credentials`    | object   | 凭据链配置（defaultSource、autoDiscover、claudeCode）                                                                                                                                                                               |
| `mcpServers`     | object   | MCP 服务器定义（stdio、sse、http、ws）                                                                                                                                                                                              |
| `sandbox`        | object   | 沙箱安全设置（enabled、filesystemMode、allowedMounts）                                                                                                                                                                              |
| `enabledPlugins` | object   | 插件启用/禁用覆盖（`name@marketplace` → 布尔值的映射）                                                                                                                                                                              |
| `plugins`        | object   | 插件管理（externalDirectories、installRoot）                                                                                                                                                                                        |
| `hooks`          | object   | `PreToolUse` / `PostToolUse` Hook 的 Shell 命令                                                                                                                                                                                     |

运行时查看合并配置：`/config`、`/config env`、`/config model`

### 环境变量

通过 Shell export **或** settings.json 的 `"env"` 字段设置（Shell export 优先）：

| 变量                       | 用途                                                                              |
| -------------------------- | --------------------------------------------------------------------------------- |
| `ANTHROPIC_API_KEY`        | Claude API Key                                                                    |
| `ANTHROPIC_AUTH_TOKEN`     | Bearer Token（替代方式）                                                          |
| `XAI_API_KEY`              | xAI / Grok API Key                                                                |
| `OPENAI_API_KEY`           | OpenAI API Key                                                                    |
| `OPENROUTER_API_KEY`       | OpenRouter API Key                                                                |
| `GROQ_API_KEY`             | Groq Cloud API Key                                                                |
| `GEMINI_API_KEY`           | Google Gemini API Key（[AI Studio 免费申请](https://aistudio.google.com/apikey)） |
| `DASHSCOPE_API_KEY`        | 阿里云通义 DashScope（OpenAI 兼容模式）                                           |
| `OLLAMA_HOST`              | Ollama 端点（如 `http://192.168.1.100:11434`）                                    |
| `CODINEER_WORKSPACE_ROOT`  | 覆盖工作区根路径                                                                  |
| `CODINEER_CONFIG_HOME`     | 覆盖全局配置目录（默认 `~/.codineer`）；`settings.json` 从该目录读取               |
| `CODINEER_PERMISSION_MODE` | 默认权限模式                                                                      |
| `NO_COLOR`                 | 禁用 ANSI 颜色                                                                    |
| `CLICOLOR=0`               | 禁用 ANSI 颜色（替代方式）                                                        |

**凭据链（按 Provider 分别管理，优先级从高到低）：**

| Provider           | 凭据链                                                                                                  |
| ------------------ | ------------------------------------------------------------------------------------------------------- |
| Anthropic (Claude) | `ANTHROPIC_API_KEY` / `ANTHROPIC_AUTH_TOKEN` → Codineer OAuth (`codineer login`) → Claude Code 自动发现 |
| xAI (Grok)         | `XAI_API_KEY`                                                                                           |
| OpenAI             | `OPENAI_API_KEY`                                                                                        |
| 自定义 Provider    | 内联 `apiKey` → `apiKeyEnv` 环境变量                                                                    |

**Claude Code 自动发现：** 如果你已安装 Claude Code 并登录（`claude login`），Codineer 会自动从 `~/.claude/.credentials.json`（或 macOS 钥匙串）发现凭据。这意味着你可以直接使用已有的 Claude 订阅，无需单独获取 API Key。

在 `settings.json` 中配置：

```json
{ "credentials": { "autoDiscover": true, "claudeCode": { "enabled": true } } }
```

查看认证状态：`codineer status` 或 `codineer status anthropic`

**配置管理：**

```bash
codineer config set model sonnet               # 设置配置项
codineer config get model                      # 读取配置项
codineer config list                           # 列出全部配置
```

---

## 项目上下文

`.codineer/CODINEER.md` 是**项目记忆文件**——它会被注入到每次对话的 system prompt 中，让 AI 无需反复询问即可了解你的代码库。典型内容包括：

- 使用的技术栈和编程语言
- 构建、lint、测试命令
- 编码规范（命名、错误处理、提交风格）
- 仓库目录结构说明

```bash
codineer init        # 根据检测到的技术栈自动生成
```

`codineer init` 会创建 `.codineer/` 目录并自动检测技术栈（Rust/Python/Node 等）生成初始 `CODINEER.md`。可自由编辑并提交到仓库，让整个团队受益。

Codineer 沿目录树向上查找并加载所有匹配的指令文件：

| 文件                        | 用途                       |
| --------------------------- | -------------------------- |
| `.codineer/CODINEER.md`     | 主要上下文（建议提交）     |
| `CODINEER.md`               | 兼容旧版（同样支持）       |
| `CODINEER.local.md`         | 个人覆盖（加入 gitignore） |
| `.codineer/instructions.md` | 附加指令                   |

所有匹配文件从每个祖先目录收集、去重后拼接到 system prompt 中。这意味着 monorepo 中的子项目可以拥有自己的 `CODINEER.md`，与根目录的版本互补。

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

插件用于扩展 Codineer，可以提供自定义**工具**（AI 可调用的函数）、**斜杠命令**、**钩子**（工具调用前/后拦截）和**生命周期脚本**。插件是一个包含 `plugin.json` 清单的目录：

```
.codineer/plugins/my-plugin/
├── plugin.json              ← 清单（工具、命令、钩子、生命周期）
├── tools/query-db.sh        ← AI 自动调用此工具
├── hooks/audit.sh           ← 每次工具调用前/后运行
└── commands/deploy.sh       ← 用户输入 /deploy 时执行
```

在 REPL 中管理插件：

```bash
/plugin list                        # 列出所有插件及状态
/plugin install ./path/to/plugin    # 从本地路径或 Git URL 安装
/plugin enable my-plugin            # 启用
/plugin disable my-plugin           # 禁用
```

> **完整插件开发指南：** [`crates/plugins/README_CN.md`](crates/plugins/README_CN.md)

### Agent 与 Skill

**Agent** 是针对专项任务的命名子 Agent 配置。**Skill** 是可复用的提示模板。

```bash
codineer agents          # 列出 Agent
codineer skills          # 列出 Skill
/agents                  # REPL 内
/skills                  # REPL 内
```

Skill 搜索路径：项目 `.codineer/skills/`（未初始化时退到 `~/.codineer/skills/`），以及 `$CODINEER_CONFIG_HOME/skills/` 和 `~/.codineer/skills/`。

---

## 参考

### 内置工具

| 分类 | 工具 | 说明 |
| ---- | ---- | ---- |
| **文件 I/O** | `read_file` | 读取文件内容（文本、PDF 文本提取、图片 base64） |
| | `write_file` | 创建或覆盖文件（原子写入，mtime 追踪） |
| | `edit_file` | 精准字符串替换，支持冲突检测 |
| | `glob_search` | 按 Glob 模式查找文件（.gitignore 感知） |
| | `grep_search` | 正则搜索文件内容（基于 ripgrep 核心） |
| **Shell** | `bash` | 执行 Shell 命令（超时、后台执行） |
| | `PowerShell` | 执行 PowerShell 命令（Windows / 跨平台） |
| | `REPL` | 运行 Python、Node 或 Shell 代码 |
| **Web** | `WebFetch` | 抓取并摘要网页 |
| | `WebSearch` | DuckDuckGo 搜索 |
| **Notebook** | `NotebookEdit` | 编辑 Jupyter Notebook |
| **Agent** | `Agent` | 启动子 Agent 进行并行任务 |
| | `SendUserMessage` | 向用户发送消息 |
| **LSP** | `Lsp` | 语言服务器操作（悬浮、补全、跳转定义、引用、符号、重命名、格式化、诊断） |
| **任务管理** | `TaskCreate` | 创建后台任务（可选命令） |
| | `TaskGet` | 获取任务状态和输出 |
| | `TaskList` | 列出所有任务 |
| | `TaskUpdate` | 更新任务标题、描述或状态 |
| | `TaskStop` | 停止运行中的任务 |
| **规划模式** | `EnterPlanMode` | 进入只读规划模式 |
| | `ExitPlanMode` | 退出规划模式 |
| **Git Worktree** | `EnterWorktree` | 创建并进入隔离的 git worktree |
| | `ExitWorktree` | 退出并可选清理 worktree |
| **定时任务** | `CronCreate` | 创建托管的 cron 任务 |
| | `CronDelete` | 删除托管的 cron 任务 |
| | `CronList` | 列出托管的 cron 任务 |
| **MCP 资源** | `ListMcpResources` | 列出可用的 MCP 资源 |
| | `ReadMcpResource` | 按 URI 读取 MCP 资源 |
| | `MCPSearch` | 全文搜索 MCP 资源 |
| **协作** | `TeamCreate` | 创建命名 Agent 团队 |
| | `TeamDelete` | 删除 Agent 团队 |
| | `SendMessage` | 向 Agent 或团队发送消息 |
| | `SlashCommand` | 调用已注册的 slash 命令 |
| **其他** | `TodoWrite` | 管理结构化任务列表 |
| | `Skill` | 执行 Skill 模板 |
| | `ToolSearch` | 搜索可用工具 |
| | `Config` | 读写配置值 |
| | `StructuredOutput` | 返回结构化 JSON |
| | `Sleep` | 暂停执行指定时长 |

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
