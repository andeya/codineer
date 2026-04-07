use std::env;
use std::path::Path;

use commands::{handle_agents_slash_command, handle_skills_slash_command, SlashCommand};
use runtime::{compact_session, CompactionConfig, Session, UsageTracker};

use crate::cli::default_permission_mode;
use crate::error::CliResult;
use crate::help::render_repl_help;
use crate::reports::{
    format_compact_report, format_cost_report, format_status_report, render_config_report,
    render_diff_report, render_export_text, render_memory_report, render_version_report,
    resolve_export_path, status_context, StatusUsage,
};

#[derive(Debug, Clone)]
pub(crate) struct ResumeCommandOutcome {
    pub(crate) session: Session,
    pub(crate) message: Option<String>,
}

impl ResumeCommandOutcome {
    pub(crate) fn keep(session: &Session, message: String) -> Self {
        Self {
            session: session.clone(),
            message: Some(message),
        }
    }
}

pub(crate) fn resume_session(session_path: &Path, commands: &[String]) {
    let session = match Session::load_from_path(session_path) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("failed to restore session: {error}");
            std::process::exit(1);
        }
    };

    if commands.is_empty() {
        println!(
            "Restored session from {} ({} messages).",
            session_path.display(),
            session.messages.len()
        );
        return;
    }

    let mut session = session;
    for raw_command in commands {
        let Some(command) = SlashCommand::parse(raw_command) else {
            eprintln!("unsupported resumed command: {raw_command}");
            std::process::exit(2);
        };
        match run_resume_command(session_path, &session, &command) {
            Ok(ResumeCommandOutcome {
                session: next_session,
                message,
            }) => {
                session = next_session;
                if let Some(message) = message {
                    println!("{message}");
                }
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        }
    }
}

pub(crate) fn run_resume_command(
    session_path: &Path,
    session: &Session,
    command: &SlashCommand,
) -> CliResult<ResumeCommandOutcome> {
    match command {
        SlashCommand::Help => Ok(ResumeCommandOutcome::keep(session, render_repl_help())),
        SlashCommand::Compact => run_resume_compact(session_path, session),
        SlashCommand::Clear { confirm } => run_resume_clear(session_path, session, *confirm),
        SlashCommand::Status => run_resume_status(session_path, session),
        SlashCommand::Cost => {
            let usage = UsageTracker::from_session(session).cumulative_usage();
            Ok(ResumeCommandOutcome::keep(
                session,
                format_cost_report(usage),
            ))
        }
        SlashCommand::Config { section } => Ok(ResumeCommandOutcome::keep(
            session,
            render_config_report(section.as_deref())?,
        )),
        SlashCommand::Memory => Ok(ResumeCommandOutcome::keep(session, render_memory_report()?)),
        SlashCommand::Init => Ok(ResumeCommandOutcome::keep(
            session,
            crate::init_codineer_md()?,
        )),
        SlashCommand::Diff => Ok(ResumeCommandOutcome::keep(session, render_diff_report()?)),
        SlashCommand::Version => Ok(ResumeCommandOutcome::keep(session, render_version_report())),
        SlashCommand::Export { path } => run_resume_export(session, path.as_deref()),
        SlashCommand::Agents { args } => {
            let cwd = env::current_dir()?;
            Ok(ResumeCommandOutcome::keep(
                session,
                handle_agents_slash_command(args.as_deref(), &cwd)?,
            ))
        }
        SlashCommand::Skills { args } => {
            let cwd = env::current_dir()?;
            Ok(ResumeCommandOutcome::keep(
                session,
                handle_skills_slash_command(args.as_deref(), &cwd)?,
            ))
        }
        _ => Err("unsupported resumed slash command".into()),
    }
}

pub(crate) fn run_resume_compact(
    session_path: &Path,
    session: &Session,
) -> CliResult<ResumeCommandOutcome> {
    let result = compact_session(
        session,
        CompactionConfig {
            max_estimated_tokens: 0,
            ..CompactionConfig::default()
        },
    );
    let removed = result.removed_message_count;
    let kept = result.compacted_session.messages.len();
    let skipped = removed == 0;
    result.compacted_session.save_to_path(session_path)?;
    Ok(ResumeCommandOutcome {
        session: result.compacted_session,
        message: Some(format_compact_report(removed, kept, skipped)),
    })
}

pub(crate) fn run_resume_clear(
    session_path: &Path,
    session: &Session,
    confirm: bool,
) -> CliResult<ResumeCommandOutcome> {
    if !confirm {
        return Ok(ResumeCommandOutcome::keep(
            session,
            "clear: confirmation required; rerun with /clear --confirm".to_string(),
        ));
    }
    let cleared = Session::new();
    cleared.save_to_path(session_path)?;
    Ok(ResumeCommandOutcome {
        session: cleared,
        message: Some(format!(
            "Cleared resumed session file {}.",
            session_path.display()
        )),
    })
}

pub(crate) fn run_resume_status(
    session_path: &Path,
    session: &Session,
) -> CliResult<ResumeCommandOutcome> {
    let tracker = UsageTracker::from_session(session);
    let usage = tracker.cumulative_usage();
    Ok(ResumeCommandOutcome::keep(
        session,
        format_status_report(
            "restored-session",
            StatusUsage {
                message_count: session.messages.len(),
                turns: tracker.turns(),
                latest: tracker.current_turn_usage(),
                cumulative: usage,
                estimated_tokens: 0,
            },
            default_permission_mode().as_str(),
            &status_context(Some(session_path))?,
        ),
    ))
}

pub(crate) fn run_resume_export(
    session: &Session,
    path: Option<&str>,
) -> CliResult<ResumeCommandOutcome> {
    let export_path = resolve_export_path(path, session)?;
    std::fs::write(&export_path, render_export_text(session))?;
    Ok(ResumeCommandOutcome::keep(
        session,
        format!(
            "Export\n  Result           wrote transcript\n  File             {}\n  Messages         {}",
            export_path.display(),
            session.messages.len(),
        ),
    ))
}
