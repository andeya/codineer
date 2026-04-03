use std::borrow::ToOwned;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::discovery::normalize_optional_args;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitPushPrRequest {
    pub commit_message: Option<String>,
    pub pr_title: String,
    pub pr_body: String,
    pub branch_name_hint: String,
}

pub fn handle_branch_slash_command(
    action: Option<&str>,
    target: Option<&str>,
    cwd: &Path,
) -> io::Result<String> {
    match normalize_optional_args(action) {
        None | Some("list") => {
            let branches = git_stdout(cwd, &["branch", "--list", "--verbose"])?;
            let trimmed = branches.trim();
            Ok(if trimmed.is_empty() {
                "Branch\n  Result           no branches found".to_string()
            } else {
                format!("Branch\n  Result           listed\n\n{trimmed}")
            })
        }
        Some("create") => {
            let Some(target) = target.filter(|value| !value.trim().is_empty()) else {
                return Ok("Usage: /branch create <name>".to_string());
            };
            git_status_ok(cwd, &["switch", "-c", target])?;
            Ok(format!(
                "Branch\n  Result           created and switched\n  Branch           {target}"
            ))
        }
        Some("switch") => {
            let Some(target) = target.filter(|value| !value.trim().is_empty()) else {
                return Ok("Usage: /branch switch <name>".to_string());
            };
            git_status_ok(cwd, &["switch", target])?;
            Ok(format!(
                "Branch\n  Result           switched\n  Branch           {target}"
            ))
        }
        Some(other) => Ok(format!(
            "Unknown /branch action '{other}'. Use /branch list, /branch create <name>, or /branch switch <name>."
        )),
    }
}

pub fn handle_worktree_slash_command(
    action: Option<&str>,
    path: Option<&str>,
    branch: Option<&str>,
    cwd: &Path,
) -> io::Result<String> {
    match normalize_optional_args(action) {
        None | Some("list") => {
            let worktrees = git_stdout(cwd, &["worktree", "list"])?;
            let trimmed = worktrees.trim();
            Ok(if trimmed.is_empty() {
                "Worktree\n  Result           no worktrees found".to_string()
            } else {
                format!("Worktree\n  Result           listed\n\n{trimmed}")
            })
        }
        Some("add") => {
            let Some(path) = path.filter(|value| !value.trim().is_empty()) else {
                return Ok("Usage: /worktree add <path> [branch]".to_string());
            };
            if let Some(branch) = branch.filter(|value| !value.trim().is_empty()) {
                if branch_exists(cwd, branch) {
                    git_status_ok(cwd, &["worktree", "add", path, branch])?;
                } else {
                    git_status_ok(cwd, &["worktree", "add", path, "-b", branch])?;
                }
                Ok(format!(
                    "Worktree\n  Result           added\n  Path             {path}\n  Branch           {branch}"
                ))
            } else {
                git_status_ok(cwd, &["worktree", "add", path])?;
                Ok(format!(
                    "Worktree\n  Result           added\n  Path             {path}"
                ))
            }
        }
        Some("remove") => {
            let Some(path) = path.filter(|value| !value.trim().is_empty()) else {
                return Ok("Usage: /worktree remove <path>".to_string());
            };
            git_status_ok(cwd, &["worktree", "remove", path])?;
            Ok(format!(
                "Worktree\n  Result           removed\n  Path             {path}"
            ))
        }
        Some("prune") => {
            git_status_ok(cwd, &["worktree", "prune"])?;
            Ok("Worktree\n  Result           pruned".to_string())
        }
        Some(other) => Ok(format!(
            "Unknown /worktree action '{other}'. Use /worktree list, /worktree add <path> [branch], /worktree remove <path>, or /worktree prune."
        )),
    }
}

pub fn handle_commit_slash_command(message: &str, cwd: &Path) -> io::Result<String> {
    let status = git_stdout(cwd, &["status", "--short"])?;
    if status.trim().is_empty() {
        return Ok(
            "Commit\n  Result           skipped\n  Reason           no workspace changes"
                .to_string(),
        );
    }

    let message = message.trim();
    if message.is_empty() {
        return Err(io::Error::other("generated commit message was empty"));
    }

    git_status_ok(cwd, &["add", "-A"])?;
    let path = write_temp_text_file("codineer-commit-message", "txt", message)?;
    let path_string = path.to_string_lossy().into_owned();
    git_status_ok(cwd, &["commit", "--file", path_string.as_str()])?;

    Ok(format!(
        "Commit\n  Result           created\n  Message file     {}\n\n{}",
        path.display(),
        message
    ))
}

pub fn handle_commit_push_pr_slash_command(
    request: &CommitPushPrRequest,
    cwd: &Path,
) -> io::Result<String> {
    if !command_exists("gh") {
        return Err(io::Error::other("gh CLI is required for /commit-push-pr"));
    }

    let default_branch = detect_default_branch(cwd)?;
    let mut branch = current_branch(cwd)?;
    let mut created_branch = false;
    if branch == default_branch {
        let hint = if request.branch_name_hint.trim().is_empty() {
            request.pr_title.as_str()
        } else {
            request.branch_name_hint.as_str()
        };
        let next_branch = build_branch_name(hint);
        git_status_ok(cwd, &["switch", "-c", next_branch.as_str()])?;
        branch = next_branch;
        created_branch = true;
    }

    let workspace_has_changes = !git_stdout(cwd, &["status", "--short"])?.trim().is_empty();
    let commit_report = if workspace_has_changes {
        let Some(message) = request.commit_message.as_deref() else {
            return Err(io::Error::other(
                "commit message is required when workspace changes are present",
            ));
        };
        Some(handle_commit_slash_command(message, cwd)?)
    } else {
        None
    };

    let branch_diff = git_stdout(
        cwd,
        &["diff", "--stat", &format!("{default_branch}...HEAD")],
    )?;
    if branch_diff.trim().is_empty() {
        return Ok(
            "Commit/Push/PR\n  Result           skipped\n  Reason           no branch changes to push or open as a pull request"
                .to_string(),
        );
    }

    git_status_ok(cwd, &["push", "--set-upstream", "origin", branch.as_str()])?;

    let body_path = write_temp_text_file("codineer-pr-body", "md", request.pr_body.trim())?;
    let body_path_string = body_path.to_string_lossy().into_owned();
    let create = Command::new("gh")
        .args([
            "pr",
            "create",
            "--title",
            request.pr_title.as_str(),
            "--body-file",
            body_path_string.as_str(),
            "--base",
            default_branch.as_str(),
        ])
        .current_dir(cwd)
        .output()?;

    let (result, url) = if create.status.success() {
        (
            "created",
            parse_pr_url(&String::from_utf8_lossy(&create.stdout))
                .unwrap_or_else(|| "<unknown>".to_string()),
        )
    } else {
        let view = Command::new("gh")
            .args(["pr", "view", "--json", "url"])
            .current_dir(cwd)
            .output()?;
        if !view.status.success() {
            return Err(io::Error::other(command_failure(
                "gh",
                &["pr", "create"],
                &create,
            )));
        }
        (
            "existing",
            parse_pr_json_url(&String::from_utf8_lossy(&view.stdout))
                .unwrap_or_else(|| "<unknown>".to_string()),
        )
    };

    let mut lines = vec![
        "Commit/Push/PR".to_string(),
        format!("  Result           {result}"),
        format!("  Branch           {branch}"),
        format!("  Base             {default_branch}"),
        format!("  Body file        {}", body_path.display()),
        format!("  URL              {url}"),
    ];
    if created_branch {
        lines.insert(2, "  Branch action    created and switched".to_string());
    }
    if let Some(report) = commit_report {
        lines.push(String::new());
        lines.push(report);
    }
    Ok(lines.join("\n"))
}

pub fn detect_default_branch(cwd: &Path) -> io::Result<String> {
    if let Ok(reference) = git_stdout(cwd, &["symbolic-ref", "refs/remotes/origin/HEAD"]) {
        if let Some(branch) = reference
            .trim()
            .rsplit('/')
            .next()
            .filter(|value| !value.is_empty())
        {
            return Ok(branch.to_string());
        }
    }

    for branch in ["main", "master"] {
        if branch_exists(cwd, branch) {
            return Ok(branch.to_string());
        }
    }

    current_branch(cwd)
}

fn git_stdout(cwd: &Path, args: &[&str]) -> io::Result<String> {
    run_command_stdout("git", args, cwd)
}

fn git_status_ok(cwd: &Path, args: &[&str]) -> io::Result<()> {
    run_command_success("git", args, cwd)
}

fn run_command_stdout(program: &str, args: &[&str], cwd: &Path) -> io::Result<String> {
    let output = Command::new(program).args(args).current_dir(cwd).output()?;
    if !output.status.success() {
        return Err(io::Error::other(command_failure(program, args, &output)));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn run_command_success(program: &str, args: &[&str], cwd: &Path) -> io::Result<()> {
    let output = Command::new(program).args(args).current_dir(cwd).output()?;
    if !output.status.success() {
        return Err(io::Error::other(command_failure(program, args, &output)));
    }
    Ok(())
}

fn command_failure(program: &str, args: &[&str], output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if stderr.is_empty() { stdout } else { stderr };
    if detail.is_empty() {
        format!("{program} {} failed", args.join(" "))
    } else {
        format!("{program} {} failed: {detail}", args.join(" "))
    }
}

fn branch_exists(cwd: &Path, branch: &str) -> bool {
    Command::new("git")
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ])
        .current_dir(cwd)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn current_branch(cwd: &Path) -> io::Result<String> {
    let branch = git_stdout(cwd, &["branch", "--show-current"])?;
    let branch = branch.trim();
    if branch.is_empty() {
        Err(io::Error::other("unable to determine current git branch"))
    } else {
        Ok(branch.to_string())
    }
}

fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn write_temp_text_file(prefix: &str, extension: &str, contents: &str) -> io::Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let path = env::temp_dir().join(format!("{prefix}-{nanos}.{extension}"));
    fs::write(&path, contents)?;
    Ok(path)
}

fn build_branch_name(hint: &str) -> String {
    let slug = slugify(hint);
    let owner = env::var("SAFEUSER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            env::var("USER")
                .ok()
                .filter(|value| !value.trim().is_empty())
        });
    match owner {
        Some(owner) => format!("{owner}/{slug}"),
        None => slug,
    }
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "change".to_string()
    } else {
        slug
    }
}

fn parse_pr_url(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("http://") || line.starts_with("https://"))
        .map(ToOwned::to_owned)
}

fn parse_pr_json_url(stdout: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(stdout)
        .ok()?
        .get("url")?
        .as_str()
        .map(ToOwned::to_owned)
}

