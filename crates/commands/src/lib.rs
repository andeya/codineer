mod discovery;
mod git;
mod plugins_cmd;
mod registry;
mod slash_help;
mod slash_spec;

#[cfg(test)]
mod tests;

pub use discovery::{handle_agents_slash_command, handle_skills_slash_command};
pub use git::{
    detect_default_branch, handle_branch_slash_command, handle_commit_push_pr_slash_command,
    handle_commit_slash_command, handle_worktree_slash_command, CommitPushPrRequest,
};
pub use plugins_cmd::{handle_plugins_slash_command, render_plugins_report};
pub use registry::{CommandManifestEntry, CommandRegistry, CommandSource};
pub use slash_help::{
    render_slash_command_help, resume_supported_slash_commands, suggest_slash_commands,
};
pub use slash_spec::{slash_command_specs, SlashCommand, SlashCommandCategory, SlashCommandSpec};

use runtime::{compact_session, CompactionConfig, Session};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashCommandResult {
    pub message: String,
    pub session: Session,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginEffect {
    None,
    ReloadRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginsCommandResult {
    pub message: String,
    pub effect: PluginEffect,
}

#[must_use]
pub fn handle_slash_command(
    input: &str,
    session: &Session,
    compaction: CompactionConfig,
) -> Option<SlashCommandResult> {
    match SlashCommand::parse(input)? {
        SlashCommand::Compact => {
            let result = compact_session(session, compaction);
            let message = if result.removed_message_count == 0 {
                "Compaction skipped: session is below the compaction threshold.".to_string()
            } else {
                format!(
                    "Compacted {} messages into a resumable system summary.",
                    result.removed_message_count
                )
            };
            Some(SlashCommandResult {
                message,
                session: result.compacted_session,
            })
        }
        SlashCommand::Help => Some(SlashCommandResult {
            message: render_slash_command_help(),
            session: session.clone(),
        }),
        SlashCommand::Status
        | SlashCommand::Branch { .. }
        | SlashCommand::Bughunter { .. }
        | SlashCommand::Worktree { .. }
        | SlashCommand::Commit
        | SlashCommand::CommitPushPr { .. }
        | SlashCommand::Pr { .. }
        | SlashCommand::Issue { .. }
        | SlashCommand::Ultraplan { .. }
        | SlashCommand::Teleport { .. }
        | SlashCommand::DebugToolCall
        | SlashCommand::Model { .. }
        | SlashCommand::Permissions { .. }
        | SlashCommand::Clear { .. }
        | SlashCommand::Cost
        | SlashCommand::Resume { .. }
        | SlashCommand::Config { .. }
        | SlashCommand::Memory
        | SlashCommand::Init
        | SlashCommand::Diff
        | SlashCommand::Version
        | SlashCommand::Export { .. }
        | SlashCommand::Session { .. }
        | SlashCommand::Plugins { .. }
        | SlashCommand::Models { .. }
        | SlashCommand::Providers
        | SlashCommand::Agents { .. }
        | SlashCommand::Skills { .. }
        | SlashCommand::Unknown(_) => None,
    }
}
