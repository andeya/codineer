# codineer-plugins

[Codineer](https://github.com/andeya/codineer) 的插件系统与 Hook 框架。

[English](README.md)

---

## 概述

**插件**是一个自包含的扩展包，可以提供以下任意组合的能力：

| 能力         | 说明                                               | 触发方式           |
| ------------ | -------------------------------------------------- | ------------------ |
| **Tools**    | 暴露给 AI 的可调用函数                             | AI 自动调用        |
| **Commands** | 用户可用的自定义 `/斜杠命令`                       | 用户输入 `/命令名` |
| **Hooks**    | 在每次工具调用前/后自动运行的脚本                  | 自动拦截           |
| **生命周期** | 会话开始（`Init`）和结束（`Shutdown`）时运行的脚本 | 自动触发           |

插件安装并启用后，其工具会与内置工具（如 `bash`、`read_file`）一起展示给 AI，钩子透明运行，命令可在 REPL 中使用。

---

## 插件结构

插件是一个目录，根目录下有 `plugin.json` 清单文件：

```
my-plugin/
├── plugin.json          ← 清单文件（必需）
├── tools/
│   └── query-db.sh      ← 工具可执行文件
├── hooks/
│   ├── pre.sh           ← PreToolUse 钩子
│   └── post.sh          ← PostToolUse 钩子
├── commands/
│   └── deploy.sh        ← 斜杠命令可执行文件
└── lifecycle/
    ├── init.sh           ← 会话启动时运行
    └── shutdown.sh       ← 会话结束时运行
```

只有 `plugin.json` 是必需的，其他文件取决于插件提供哪些能力。

---

## plugin.json 参考

```json
{
  "name": "my-plugin",
  "version": "1.0.0",
  "description": "简要描述这个插件的功能",
  "permissions": ["read", "write", "execute"],
  "defaultEnabled": true,
  "tools": [ ... ],
  "commands": [ ... ],
  "hooks": {
    "PreToolUse": ["./hooks/pre.sh"],
    "PostToolUse": ["./hooks/post.sh"]
  },
  "lifecycle": {
    "Init": ["./lifecycle/init.sh"],
    "Shutdown": ["./lifecycle/shutdown.sh"]
  }
}
```

### 顶层字段

| 字段             | 类型     | 必填 | 说明                                                  |
| ---------------- | -------- | ---- | ----------------------------------------------------- |
| `name`           | string   | 是   | 唯一的插件名称                                        |
| `version`        | string   | 是   | 语义化版本号                                          |
| `description`    | string   | 是   | 人类可读的描述                                        |
| `permissions`    | string[] | 否   | 插件级权限：`"read"`、`"write"`、`"execute"`          |
| `defaultEnabled` | boolean  | 否   | 是否默认启用（默认值：`false`）                       |
| `tools`          | array    | 否   | 工具定义（见下方）                                    |
| `commands`       | array    | 否   | 斜杠命令定义（见下方）                                |
| `hooks`          | object   | 否   | 钩子脚本：`PreToolUse` 和/或 `PostToolUse` 字符串数组 |
| `lifecycle`      | object   | 否   | 生命周期脚本：`Init` 和/或 `Shutdown` 字符串数组      |

---

## 定义工具（Tools）

工具是 AI 在对话过程中可以调用的函数，每个工具映射到一个可执行脚本。

```json
{
  "tools": [
    {
      "name": "query_database",
      "description": "对项目数据库执行只读 SQL 查询",
      "inputSchema": {
        "type": "object",
        "properties": {
          "sql": { "type": "string", "description": "要执行的 SQL 查询" },
          "limit": {
            "type": "integer",
            "minimum": 1,
            "description": "最大返回行数"
          }
        },
        "required": ["sql"],
        "additionalProperties": false
      },
      "command": "./tools/query-db.sh",
      "args": ["--readonly"],
      "requiredPermission": "read-only"
    }
  ]
}
```

### 工具字段

| 字段                 | 类型     | 必填 | 说明                                                               |
| -------------------- | -------- | ---- | ------------------------------------------------------------------ |
| `name`               | string   | 是   | 工具名称（在所有插件和内置工具中必须唯一）                         |
| `description`        | string   | 是   | 告诉 AI 何时以及如何使用此工具                                     |
| `inputSchema`        | object   | 是   | 描述工具输入参数的 JSON Schema                                     |
| `command`            | string   | 是   | 可执行文件路径（相对于插件根目录，或绝对路径）                     |
| `args`               | string[] | 否   | 传给命令的额外参数                                                 |
| `requiredPermission` | string   | 是   | 三选一：`"read-only"`、`"workspace-write"`、`"danger-full-access"` |

### 工具 I/O 协议

- **输入**：工具通过 **stdin** 接收 JSON 对象。
- **输出**：工具通过 **stdout** 输出结果（纯文本或 JSON）。
- **错误**：非零退出码视为错误；stderr 内容会被捕获并上报。

### 环境变量

工具脚本运行时会注入以下环境变量：

| 变量                 | 说明                                   |
| -------------------- | -------------------------------------- |
| `CODINEER_PLUGIN_ID` | 完整插件 ID（如 `my-plugin@external`） |
| `CODINEER_TOOL_NAME` | 当前被调用的工具名称                   |

### 工具脚本示例

```bash
#!/bin/sh
# tools/query-db.sh — 从 stdin 读取 JSON，结果输出到 stdout
INPUT=$(cat)
SQL=$(echo "$INPUT" | jq -r '.sql')
sqlite3 project.db "$SQL" --json
```

---

## 定义命令（Commands）

命令是用户可以在 REPL 中通过 `/名称` 调用的自定义斜杠命令。

```json
{
  "commands": [
    {
      "name": "deploy",
      "description": "将当前项目部署到 staging 环境",
      "command": "./commands/deploy.sh"
    }
  ]
}
```

### 命令字段

| 字段          | 类型   | 必填 | 说明                                           |
| ------------- | ------ | ---- | ---------------------------------------------- |
| `name`        | string | 是   | 斜杠命令名称（用户输入 `/name`）               |
| `description` | string | 是   | 在 `/help` 中显示的说明                        |
| `command`     | string | 是   | 可执行文件路径（相对于插件根目录，或绝对路径） |

---

## 钩子（Hooks）

钩子在每次工具调用（包括内置工具和插件工具）的前/后自动运行。

```json
{
  "hooks": {
    "PreToolUse": ["./hooks/audit-log.sh"],
    "PostToolUse": ["./hooks/notify.sh"]
  }
}
```

- **PreToolUse**：每次工具执行前运行。退出码 `2` 表示拒绝此次工具调用。
- **PostToolUse**：每次工具执行后运行。

多个插件可以各自注册钩子，它们按顺序依次执行。

---

## 生命周期（Lifecycle）

生命周期脚本在每个会话中运行一次：

```json
{
  "lifecycle": {
    "Init": ["./lifecycle/init.sh"],
    "Shutdown": ["./lifecycle/shutdown.sh"]
  }
}
```

- **Init**：Codineer 会话启动时运行（插件加载完成后）。
- **Shutdown**：会话结束时运行。

---

## 发现与安装

### 插件位置

插件从以下位置发现：

| 位置                                           | 来源类型 | 说明                                |
| ---------------------------------------------- | -------- | ----------------------------------- |
| `~/.codineer/plugins/*/`                       | 已安装   | 默认安装位置                        |
| `settings.json` 中的 `plugins.installRoot`     | 已安装   | 自定义安装根目录（覆盖默认值）      |
| `plugins.externalDirectories` 中的条目         | 外部     | 额外目录（如 `<项目>/.codineer/plugins`）|
| 嵌入 Codineer 二进制                           | 内置     | 随 Codineer 分发                    |

每个插件子目录的根目录下必须包含 `plugin.json`。

> **注意**：项目内的 `.codineer/plugins/` **不会被自动发现**。如需使用项目本地插件，请在 `.codineer/settings.json` 的 `plugins.externalDirectories` 中添加该路径。

### 管理插件

使用 `/plugin` 斜杠命令（别名：`/plugins`、`/marketplace`）：

```bash
/plugin list                        # 列出所有插件及状态
/plugin install ./path/to/plugin    # 从本地目录安装
/plugin install https://github.com/user/repo.git  # 从 Git 安装
/plugin enable my-plugin            # 启用插件
/plugin disable my-plugin           # 禁用插件
/plugin update my-plugin@external   # 更新已安装的插件
/plugin uninstall my-plugin@external  # 卸载插件
```

也可直接通过 CLI 管理：

```bash
codineer plugins list
codineer plugins install ./my-plugin
```

### 配置

通过 `settings.json` 启用/禁用插件：

```json
{
  "enabledPlugins": {
    "my-plugin@external": true,
    "another-plugin@external": false
  }
}
```

---

## 完整示例

以下是一个提供工具、命令和钩子的完整插件：

### 目录结构

```
devops-toolkit/
├── plugin.json
├── tools/
│   └── health-check.sh
├── commands/
│   └── deploy.sh
└── hooks/
    └── audit-log.sh
```

### plugin.json

```json
{
  "name": "devops-toolkit",
  "version": "0.1.0",
  "description": "DevOps 自动化：健康检查、部署和审计日志",
  "permissions": ["read", "execute"],
  "defaultEnabled": true,
  "tools": [
    {
      "name": "check_service_health",
      "description": "检查指定服务的健康状态",
      "inputSchema": {
        "type": "object",
        "properties": {
          "service": { "type": "string", "description": "要检查的服务名" }
        },
        "required": ["service"],
        "additionalProperties": false
      },
      "command": "./tools/health-check.sh",
      "requiredPermission": "read-only"
    }
  ],
  "commands": [
    {
      "name": "deploy",
      "description": "将当前项目部署到 staging 环境",
      "command": "./commands/deploy.sh"
    }
  ],
  "hooks": {
    "PreToolUse": ["./hooks/audit-log.sh"]
  }
}
```

### tools/health-check.sh

```bash
#!/bin/sh
INPUT=$(cat)
SERVICE=$(echo "$INPUT" | jq -r '.service')
curl -sf "http://${SERVICE}.internal/health" && echo "healthy" || echo "unhealthy"
```

### commands/deploy.sh

```bash
#!/bin/sh
echo "正在部署到 staging..."
git push origin main:staging
echo "完成。"
```

### hooks/audit-log.sh

```bash
#!/bin/sh
echo "[$(date -u +%FT%TZ)] tool_call plugin=$CODINEER_PLUGIN_ID tool=$CODINEER_TOOL_NAME" >> .codineer/audit.log
```

### 安装和使用

```bash
# 将插件复制到项目插件目录
cp -r devops-toolkit .codineer/plugins/

# 或通过斜杠命令安装
/plugin install ./devops-toolkit

# 启用插件
/plugin enable devops-toolkit

# 现在 AI 可以调用 check_service_health，/deploy 命令也已可用
```

---

## 说明

本 crate 是 Codineer 项目的内部组件，作为 `codineer-cli` 的依赖发布到 crates.io，不用于独立使用。在 Codineer 工作区之外不保证 API 稳定性。

## 许可证

MIT — 详见 [LICENSE](https://github.com/andeya/codineer/blob/main/LICENSE)。
