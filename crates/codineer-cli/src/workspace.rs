use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use runtime::{ContentBlock, MessageRole, Session};

pub(crate) fn parse_git_status_metadata(status: Option<&str>) -> (Option<PathBuf>, Option<String>) {
    let Some(status) = status else {
        return (None, None);
    };
    let branch = status.lines().next().and_then(|line| {
        line.strip_prefix("## ")
            .map(|line| {
                line.split(['.', ' '])
                    .next()
                    .unwrap_or_default()
                    .to_string()
            })
            .filter(|value| !value.is_empty())
    });
    let project_root = find_git_root().ok();
    (project_root, branch)
}

pub(crate) fn find_git_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(env::current_dir()?)
        .output()?;
    if !output.status.success() {
        return Err("not a git repository".into());
    }
    let path = String::from_utf8(output.stdout)?.trim().to_string();
    if path.is_empty() {
        return Err("empty git root".into());
    }
    Ok(PathBuf::from(path))
}

pub(crate) fn git_output(args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(env::current_dir()?)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("git {} failed: {stderr}", args.join(" ")).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

pub(crate) fn git_status_ok(args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(env::current_dir()?)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("git {} failed: {stderr}", args.join(" ")).into());
    }
    Ok(())
}

pub(crate) fn command_exists(name: &str) -> bool {
    let check = if cfg!(windows) { "where" } else { "which" };
    Command::new(check)
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub(crate) fn write_temp_text_file(
    filename: &str,
    contents: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let unique_name = format!("{nanos}-{filename}");
    let path = env::temp_dir().join(unique_name);
    fs::write(&path, contents)?;
    Ok(path)
}

pub(crate) fn recent_user_context(session: &Session, limit: usize) -> String {
    let requests = session
        .messages
        .iter()
        .filter(|message| message.role == MessageRole::User)
        .filter_map(|message| {
            message.blocks.iter().find_map(|block| match block {
                ContentBlock::Text { text } => Some(text.trim().to_string()),
                _ => None,
            })
        })
        .rev()
        .take(limit)
        .collect::<Vec<_>>();

    if requests.is_empty() {
        "<no prior user messages>".to_string()
    } else {
        requests
            .into_iter()
            .rev()
            .enumerate()
            .map(|(index, text)| format!("{}. {}", index + 1, text))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub(crate) fn truncate_for_prompt(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.trim().to_string()
    } else {
        let truncated = value.chars().take(limit).collect::<String>();
        format!("{}\n…[truncated]", truncated.trim_end())
    }
}

pub(crate) fn sanitize_generated_message(value: &str) -> String {
    value.trim().trim_matches('`').trim().replace("\r\n", "\n")
}

pub(crate) fn parse_titled_body(value: &str) -> Option<(String, String)> {
    let normalized = sanitize_generated_message(value);
    let title = normalized
        .lines()
        .find_map(|line| line.strip_prefix("TITLE:").map(str::trim))?;
    let body_start = normalized.find("BODY:")?;
    let body = normalized[body_start + "BODY:".len()..].trim();
    Some((title.to_string(), body.to_string()))
}

/// Parse the AI-generated output for `/commit-push-pr`.
/// Expected format:
///   COMMIT: <commit message>
///   TITLE: <pr title>
///   BODY:
///   <pr body markdown>
///   BRANCH_HINT: <branch name hint>
pub(crate) fn parse_commit_push_pr_output(
    value: &str,
) -> Result<(String, String, String, Option<String>), String> {
    let normalized = sanitize_generated_message(value);
    let commit = normalized
        .lines()
        .find_map(|l| l.strip_prefix("COMMIT:").map(str::trim))
        .ok_or("missing COMMIT: line")?
        .to_string();
    let title = normalized
        .lines()
        .find_map(|l| l.strip_prefix("TITLE:").map(str::trim))
        .ok_or("missing TITLE: line")?
        .to_string();
    let body_start = normalized.find("BODY:").ok_or("missing BODY: section")?;
    let after_body = &normalized[body_start + "BODY:".len()..];
    let (body, branch_hint) = if let Some(hint_start) = after_body.find("BRANCH_HINT:") {
        let body = after_body[..hint_start].trim().to_string();
        let hint = after_body[hint_start + "BRANCH_HINT:".len()..]
            .trim()
            .to_string();
        (body, Some(hint).filter(|h| !h.is_empty()))
    } else {
        (after_body.trim().to_string(), None)
    };
    Ok((commit, title, body, branch_hint))
}
