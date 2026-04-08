# codineer-commands

Slash commands and agent/skill discovery for [Codineer](https://github.com/andeya/codineer).

[中文文档](README_CN.md)

This crate implements the REPL slash-command system and provides agent and skill discovery used by the CLI interface.

### Command categories

| Category      | Examples                                                              |
| ------------- | --------------------------------------------------------------------- |
| **Core**      | `/help`, `/status`, `/version`, `/model`, `/cost`, `/config`, `/memory` |
| **Session**   | `/compact`, `/clear`, `/session`, `/resume`, `/export`                |
| **Git**       | `/diff`, `/branch`, `/commit`, `/commit-push-pr`, `/pr`, `/issue`, `/worktree` |
| **Agents**    | `/agents`, `/skills`, `/plugin`                                       |
| **Advanced**  | `/ultraplan`, `/bughunter`, `/teleport`, `/debug-tool-call`, `/vim`   |
| **Diagnostics** | `/doctor`                                                           |
| **Update**    | `/update [check\|apply\|dismiss\|status]`                            |
| **Navigation** | `/init`, `/permissions`, `/exit`                                     |

`/compact` triggers model-based context compaction via the runtime. `/permissions` interacts with the fine-grained permission rules engine.

## Note

This is an internal crate of the Codineer project. It is published to crates.io as a dependency of `codineer-cli` and is not intended for standalone use. API stability is not guaranteed outside the Codineer workspace.

## License

MIT — see [LICENSE](https://github.com/andeya/codineer/blob/main/LICENSE) for details.
