//! Slash command parsing, help rendering, and command dispatch.

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

use codineer_core::events::RuntimeEvent;
use codineer_core::observer::RuntimeObserver;
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

/// Handle a slash command, emitting lifecycle events through the observer.
pub fn handle_slash_command(
    input: &str,
    session: &Session,
    compaction: CompactionConfig,
    observer: &mut impl RuntimeObserver,
) -> Option<SlashCommandResult> {
    let parsed = SlashCommand::parse(input)?;
    let cmd_name = parsed.name().to_owned();
    let _ = observer.on_event(&RuntimeEvent::SlashCommandStart { command: &cmd_name });

    let result = dispatch_slash_command(parsed, session, compaction);
    let success = result.is_some();
    let _ = observer.on_event(&RuntimeEvent::SlashCommandComplete {
        command: &cmd_name,
        success,
    });
    result
}

/// Backward-compatible entry point (no observer).
#[must_use]
pub fn handle_slash_command_simple(
    input: &str,
    session: &Session,
    compaction: CompactionConfig,
) -> Option<SlashCommandResult> {
    handle_slash_command(input, session, compaction, &mut ())
}

fn dispatch_slash_command(
    command: SlashCommand,
    session: &Session,
    compaction: CompactionConfig,
) -> Option<SlashCommandResult> {
    match command {
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
        | SlashCommand::Doctor
        | SlashCommand::Unknown(_) => None,
    }
}
