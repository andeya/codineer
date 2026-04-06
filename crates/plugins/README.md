# codineer-plugins

Plugin system and hooks for [Codineer](https://github.com/andeya/codineer).

[中文文档](README_CN.md)

---

## Overview

A **plugin** is a self-contained extension package that can provide any combination of:

| Capability    | Description                                                     | Trigger                     |
| ------------- | --------------------------------------------------------------- | --------------------------- |
| **Tools**     | New callable functions exposed to the AI                        | AI calls them automatically |
| **Commands**  | Custom `/slash` commands for the user                           | User types `/command-name`  |
| **Hooks**     | Scripts that run before/after every tool call                   | Automatic interception      |
| **Lifecycle** | Scripts that run at session start (`Init`) and end (`Shutdown`) | Automatic                   |

After a plugin is installed and enabled, its tools appear alongside built-in tools (e.g. `bash`, `read_file`), its hooks run transparently, and its commands become available in the REPL.

---

## Plugin Structure

A plugin is a directory with a `plugin.json` manifest at the root:

```
my-plugin/
├── plugin.json          ← manifest (required)
├── tools/
│   └── query-db.sh      ← tool executable
├── hooks/
│   ├── pre.sh           ← PreToolUse hook
│   └── post.sh          ← PostToolUse hook
├── commands/
│   └── deploy.sh        ← slash command executable
└── lifecycle/
    ├── init.sh           ← runs on session start
    └── shutdown.sh       ← runs on session end
```

Only `plugin.json` is required. All other files depend on what the plugin provides.

---

## plugin.json Reference

```json
{
  "name": "my-plugin",
  "version": "1.0.0",
  "description": "A brief description of what this plugin does",
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

### Top-Level Fields

| Field            | Type     | Required | Description                                                   |
| ---------------- | -------- | -------- | ------------------------------------------------------------- |
| `name`           | string   | Yes      | Unique plugin name                                            |
| `version`        | string   | Yes      | Semver version string                                         |
| `description`    | string   | Yes      | Human-readable description                                    |
| `permissions`    | string[] | No       | Plugin-level permissions: `"read"`, `"write"`, `"execute"`    |
| `defaultEnabled` | boolean  | No       | Whether the plugin is enabled by default (default: `false`)   |
| `tools`          | array    | No       | Tool definitions (see below)                                  |
| `commands`       | array    | No       | Slash command definitions (see below)                         |
| `hooks`          | object   | No       | Hook scripts: `PreToolUse` and/or `PostToolUse` string arrays |
| `lifecycle`      | object   | No       | Lifecycle scripts: `Init` and/or `Shutdown` string arrays     |

---

## Defining Tools

Tools are functions that the AI can call during a conversation. Each tool maps to an executable script.

```json
{
  "tools": [
    {
      "name": "query_database",
      "description": "Run a read-only SQL query against the project database",
      "inputSchema": {
        "type": "object",
        "properties": {
          "sql": {
            "type": "string",
            "description": "The SQL query to execute"
          },
          "limit": {
            "type": "integer",
            "minimum": 1,
            "description": "Max rows to return"
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

### Tool Fields

| Field                | Type     | Required | Description                                                        |
| -------------------- | -------- | -------- | ------------------------------------------------------------------ |
| `name`               | string   | Yes      | Tool name (must be unique across all plugins and built-in tools)   |
| `description`        | string   | Yes      | Tells the AI when and how to use this tool                         |
| `inputSchema`        | object   | Yes      | JSON Schema describing the tool's input parameters                 |
| `command`            | string   | Yes      | Path to the executable (relative to plugin root, or absolute)      |
| `args`               | string[] | No       | Additional arguments passed to the command                         |
| `requiredPermission` | string   | Yes      | One of: `"read-only"`, `"workspace-write"`, `"danger-full-access"` |

### Tool I/O Protocol

- **Input**: The tool receives its input as a JSON object on **stdin**.
- **Output**: The tool writes its result to **stdout** (plain text or JSON).
- **Errors**: Non-zero exit codes are treated as errors; stderr is captured and reported.

### Environment Variables

The following environment variables are injected when a tool script runs:

| Variable             | Description                                |
| -------------------- | ------------------------------------------ |
| `CODINEER_PLUGIN_ID` | Full plugin ID (e.g. `my-plugin@external`) |
| `CODINEER_TOOL_NAME` | The name of the tool being called          |

### Example Tool Script

```bash
#!/bin/sh
# tools/query-db.sh — receives JSON on stdin, outputs result on stdout
INPUT=$(cat)
SQL=$(echo "$INPUT" | jq -r '.sql')
sqlite3 project.db "$SQL" --json
```

---

## Defining Commands

Commands are custom slash commands that users can invoke from the REPL.

```json
{
  "commands": [
    {
      "name": "deploy",
      "description": "Deploy the current project to staging",
      "command": "./commands/deploy.sh"
    }
  ]
}
```

### Command Fields

| Field         | Type   | Required | Description                                                   |
| ------------- | ------ | -------- | ------------------------------------------------------------- |
| `name`        | string | Yes      | Slash command name (user types `/name`)                       |
| `description` | string | Yes      | Shown in `/help` output                                       |
| `command`     | string | Yes      | Path to the executable (relative to plugin root, or absolute) |

---

## Hooks

Hooks run automatically before or after every tool call across all tools (built-in and plugin).

```json
{
  "hooks": {
    "PreToolUse": ["./hooks/audit-log.sh"],
    "PostToolUse": ["./hooks/notify.sh"]
  }
}
```

- **PreToolUse**: Runs before each tool execution. Exit code `2` denies the tool call.
- **PostToolUse**: Runs after each tool execution.

Multiple plugins can each register hooks; they all run in order.

---

## Lifecycle

Lifecycle scripts run once per session:

```json
{
  "lifecycle": {
    "Init": ["./lifecycle/init.sh"],
    "Shutdown": ["./lifecycle/shutdown.sh"]
  }
}
```

- **Init**: Runs when the Codineer session starts (after plugins are loaded).
- **Shutdown**: Runs when the session ends.

---

## Discovery & Installation

### Plugin locations

Plugins are discovered from multiple locations:

| Location                         | Source type | Description            |
| -------------------------------- | ----------- | ---------------------- |
| `<project>/.codineer/plugins/*/` | Project     | Project-local plugins  |
| `~/.codineer/plugins/*/`         | Installed   | User-installed plugins |
| Embedded in Codineer binary      | Bundled     | Shipped with Codineer  |

Each subdirectory must contain a `plugin.json` at its root.

### Managing plugins

Use the `/plugin` slash command (aliases: `/plugins`, `/marketplace`):

```bash
/plugin list                        # List all plugins with status
/plugin install ./path/to/plugin    # Install from a local directory
/plugin install https://github.com/user/repo.git  # Install from Git
/plugin enable my-plugin            # Enable a plugin
/plugin disable my-plugin           # Disable a plugin
/plugin update my-plugin@external   # Update an installed plugin
/plugin uninstall my-plugin@external  # Uninstall a plugin
```

Or from the CLI directly:

```bash
codineer plugins list
codineer plugins install ./my-plugin
```

### Configuration

Enable/disable plugins via `settings.json`:

```json
{
  "enabledPlugins": {
    "my-plugin@external": true,
    "another-plugin@external": false
  }
}
```

---

## Complete Example

Here is a full plugin that provides a tool, a command, and a hook:

### Directory structure

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
  "description": "DevOps automation: health checks, deployment, and audit logging",
  "permissions": ["read", "execute"],
  "defaultEnabled": true,
  "tools": [
    {
      "name": "check_service_health",
      "description": "Check the health status of a named service",
      "inputSchema": {
        "type": "object",
        "properties": {
          "service": {
            "type": "string",
            "description": "Service name to check"
          }
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
      "description": "Deploy the current project to staging",
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
echo "Deploying to staging..."
git push origin main:staging
echo "Done."
```

### hooks/audit-log.sh

```bash
#!/bin/sh
echo "[$(date -u +%FT%TZ)] tool_call plugin=$CODINEER_PLUGIN_ID tool=$CODINEER_TOOL_NAME" >> .codineer/audit.log
```

### Install and use

```bash
# Copy the plugin into project plugins directory
cp -r devops-toolkit .codineer/plugins/

# Or install via slash command
/plugin install ./devops-toolkit

# Enable it
/plugin enable devops-toolkit

# Now the AI can call check_service_health, and /deploy is available
```

---

## Note

This is an internal crate of the Codineer project. It is published to crates.io as a dependency of `codineer-cli` and is not intended for standalone use. API stability is not guaranteed outside the Codineer workspace.

## License

MIT — see [LICENSE](https://github.com/andeya/codineer/blob/main/LICENSE) for details.
