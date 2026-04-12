use super::settings::ManagedSettings;
use crate::error::{AppError, AppResult};
use aineer_engine::commands::{
    handle_slash_command_simple, render_slash_command_help, slash_command_specs, SlashCommandSpec,
};
use aineer_engine::{CompactionConfig, Session};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub argument_hint: Option<String>,
    pub category: String,
}

impl From<&SlashCommandSpec> for SlashCommand {
    fn from(spec: &SlashCommandSpec) -> Self {
        Self {
            name: spec.name.to_string(),
            description: spec.summary.to_string(),
            argument_hint: spec.argument_hint.map(String::from),
            category: spec.category.to_string(),
        }
    }
}

#[tauri::command]
pub async fn get_slash_commands() -> AppResult<Vec<SlashCommand>> {
    let specs = slash_command_specs();
    Ok(specs.iter().map(SlashCommand::from).collect())
}

#[tauri::command]
pub async fn execute_slash_command(
    settings_state: tauri::State<'_, ManagedSettings>,
    name: String,
    args: Option<String>,
) -> AppResult<String> {
    tracing::info!("execute_slash_command: name={name}, args={args:?}");

    let input = match &args {
        Some(a) if !a.is_empty() => format!("/{name} {a}"),
        _ => format!("/{name}"),
    };

    let session = Session::default();
    let compaction = CompactionConfig::default();

    match handle_slash_command_simple(&input, &session, compaction) {
        Some(result) => Ok(result.message),
        None => desktop_fallback(&name, &settings_state).map_err(AppError::Settings),
    }
}

fn desktop_fallback(
    name: &str,
    settings_state: &tauri::State<'_, ManagedSettings>,
) -> Result<String, String> {
    match name {
        "help" => Ok(render_slash_command_help()),
        "version" => Ok(format!("Aineer v{}", env!("CARGO_PKG_VERSION"))),
        "clear" => Ok("Conversation cleared.".to_string()),
        "models" => {
            let merged = settings_state.merged()?;
            let groups = super::settings::build_model_groups(&merged);
            let mut lines = vec!["## Available model providers\n".to_string()];
            for g in &groups {
                lines.push(format!("**{}**", g.provider));
                for m in &g.models {
                    lines.push(format!("  - `{}/{m}`", g.provider));
                }
                lines.push(String::new());
            }
            lines.push(
                "Use the model selector in the status bar or `/model <name>` to switch.\n\
                 Configure custom providers in **Settings → Models & Intelligence**."
                    .to_string(),
            );
            Ok(lines.join("\n"))
        }
        "providers" => {
            let merged = settings_state.merged()?;
            let groups = super::settings::build_model_groups(&merged);
            let mut lines = vec![
                "## Configured providers\n".to_string(),
                "| Provider | Models | Status |".to_string(),
                "|----------|--------|--------|".to_string(),
            ];
            let no_key_providers = ["ollama"];
            for g in &groups {
                let status = if no_key_providers.contains(&g.provider.as_str()) {
                    "Local (no key needed)"
                } else {
                    "API key required"
                };
                lines.push(format!(
                    "| {} | {} | {} |",
                    g.provider,
                    g.models.len(),
                    status
                ));
            }
            lines.push(String::new());
            lines.push("Configure API keys in **Settings → Models & Intelligence**.".to_string());
            Ok(lines.join("\n"))
        }
        "status" => Ok(format!(
            "## Session status\n\n- **Version**: Aineer v{}\n- **Session**: Desktop mode\n- **Messages**: (managed by UI)",
            env!("CARGO_PKG_VERSION")
        )),
        "doctor" => Ok(format!(
            "## Environment diagnostics\n\n- **Aineer**: v{}\n- **Platform**: {}\n- **Arch**: {}\n- **Engine**: Standalone desktop mode",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
        )),
        "cost" => Ok("## Token usage\n\nNo tokens consumed in this session yet.".to_string()),
        _ => {
            let specs = slash_command_specs();
            let found = specs.iter().any(|s| s.name == name);
            if found {
                Ok(format!(
                    "> `/{name}` requires a connected engine session.\n>\n> This command will be available when the AI backend is fully initialized."
                ))
            } else {
                Err(format!("Unknown command: /{name}"))
            }
        }
    }
}
