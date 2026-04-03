use std::path::Path;
use std::process::Command;

use crate::error::PluginError;
use crate::types::{PluginLifecycle, PluginMetadata};

pub(crate) fn run_lifecycle_commands(
    metadata: &PluginMetadata,
    lifecycle: &PluginLifecycle,
    phase: &str,
    commands: &[String],
) -> Result<(), PluginError> {
    if lifecycle.is_empty() || commands.is_empty() {
        return Ok(());
    }

    for command in commands {
        let mut process = if Path::new(command).exists() {
            if cfg!(windows) && command.ends_with(".sh") {
                let mut p = Command::new("bash");
                p.arg(command);
                p
            } else if cfg!(windows) {
                let mut p = Command::new("cmd");
                p.arg("/C").arg(command);
                p
            } else {
                let mut p = Command::new("sh");
                p.arg(command);
                p
            }
        } else if cfg!(windows) {
            let mut p = Command::new("cmd");
            p.arg("/C").arg(command);
            p
        } else {
            let mut p = Command::new("sh");
            p.arg("-lc").arg(command);
            p
        };
        if let Some(root) = &metadata.root {
            process.current_dir(root);
        }
        let output = process.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(PluginError::CommandFailed(format!(
                "plugin `{}` {} failed for `{}`: {}",
                metadata.id,
                phase,
                command,
                if stderr.is_empty() {
                    format!("exit status {}", output.status)
                } else {
                    stderr
                }
            )));
        }
    }

    Ok(())
}
