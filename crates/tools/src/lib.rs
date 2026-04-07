use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;

use runtime::{
    edit_file, execute_bash, glob_search, grep_search, read_file, write_file, BashCommandInput,
    GrepSearchInput,
};

mod agent;
pub mod builtin;
pub mod tool_output;
mod collab;
mod config_tool;
mod cron;
mod lsp_tool;
mod mcp_resource;
mod notebook;
mod plan_mode;
mod powershell;
mod registry;
mod specs;
mod task;
mod types;
mod web;
mod worktree;

pub use collab::{register_slash_command, SlashCommandHandler};
pub use lsp_tool::initialize_lsp_manager;
pub use mcp_resource::{register_mcp_resource, McpResource};
pub use plan_mode::is_plan_mode;
pub use registry::{GlobalToolRegistry, ToolManifestEntry, ToolRegistry, ToolSource, ToolSpec};
pub use specs::mvp_tool_specs;
pub use types::ToolSearchInput;

#[cfg(test)]
pub(crate) use agent::{
    agent_permission_policy, allowed_tools_for_subagent, execute_agent_with_spawn,
    final_assistant_text, persist_agent_terminal_state, push_output_block, SubagentToolExecutor,
};
pub(crate) use types::AgentInput;
#[cfg(test)]
pub(crate) use types::AgentJob;

use crate::types::{
    AskUserQuestionInput, AskUserQuestionOutput, BriefInput, BriefOutput, BriefStatus, ConfigInput,
    CronCreateInput, CronDeleteInput, CronListInput, EditFileInput, EnterPlanModeInput,
    EnterWorktreeInput, ExitPlanModeInput, ExitWorktreeInput, GlobSearchInputValue,
    ListMcpResourcesInput, LspInput, McpSearchInput, MultiEditInput, MultiEditOutput,
    NotebookEditInput, PowerShellInput, QuestionOption, ReadFileInput, ReadMcpResourceInput,
    ReplInput, ReplOutput, ResolvedAttachment, SendMessageInput, SkillInput, SkillOutput,
    SlashCommandInput, SleepInput, SleepOutput, StructuredOutputInput, StructuredOutputResult,
    TaskCreateInput, TaskGetInput, TaskListInput, TaskStopInput, TaskUpdateInput, TeamCreateInput,
    TeamDeleteInput, TodoItem, TodoStatus, TodoWriteInput, TodoWriteOutput, ToolSearchOutput,
    UserQuestion, WebFetchInput, WebSearchInput, WriteFileInput,
};

pub fn execute_tool(name: &str, input: &Value) -> Result<String, String> {
    // Try trait-based dispatch first (migrated tools)
    if let Some(tool) = builtin::find_builtin(name) {
        return tool
            .dispatch(input)
            .map(|o| o.content)
            .map_err(|e| e.to_string());
    }
    // Fallback: legacy match (tools not yet migrated to BuiltinTool trait)
    match name {
        "bash" => from_value::<BashCommandInput>(input).and_then(run_bash),
        "read_file" => from_value::<ReadFileInput>(input).and_then(run_read_file),
        "write_file" => from_value::<WriteFileInput>(input).and_then(run_write_file),
        "edit_file" => from_value::<EditFileInput>(input).and_then(run_edit_file),
        "glob_search" => from_value::<GlobSearchInputValue>(input).and_then(run_glob_search),
        "grep_search" => from_value::<GrepSearchInput>(input).and_then(run_grep_search),
        "WebFetch" => from_value::<WebFetchInput>(input).and_then(run_web_fetch),
        "WebSearch" => from_value::<WebSearchInput>(input).and_then(run_web_search),
        "TodoWrite" => from_value::<TodoWriteInput>(input).and_then(run_todo_write),
        "Skill" => from_value::<SkillInput>(input).and_then(run_skill),
        "Agent" => from_value::<AgentInput>(input).and_then(run_agent),
        "ToolSearch" => from_value::<ToolSearchInput>(input).and_then(run_tool_search),
        "NotebookEdit" => from_value::<NotebookEditInput>(input).and_then(run_notebook_edit),
        "SendUserMessage" | "Brief" => from_value::<BriefInput>(input).and_then(run_brief),
        "Config" => from_value::<ConfigInput>(input).and_then(run_config),
        "REPL" => from_value::<ReplInput>(input).and_then(run_repl),
        "PowerShell" => from_value::<PowerShellInput>(input).and_then(run_powershell),
        "MultiEdit" => from_value::<MultiEditInput>(input).and_then(run_multi_edit),
        "AskUserQuestion" => {
            from_value::<AskUserQuestionInput>(input).and_then(run_ask_user_question)
        }
        "Lsp" => from_value::<LspInput>(input).and_then(run_lsp),
        "TaskCreate" => from_value::<TaskCreateInput>(input).and_then(run_task_create),
        "TaskGet" => from_value::<TaskGetInput>(input).and_then(run_task_get),
        "TaskList" => from_value::<TaskListInput>(input).and_then(run_task_list),
        "TaskUpdate" => from_value::<TaskUpdateInput>(input).and_then(run_task_update),
        "TaskStop" => from_value::<TaskStopInput>(input).and_then(run_task_stop),
        "EnterPlanMode" => from_value::<EnterPlanModeInput>(input).and_then(run_enter_plan_mode),
        "ExitPlanMode" => from_value::<ExitPlanModeInput>(input).and_then(run_exit_plan_mode),
        "EnterWorktree" => from_value::<EnterWorktreeInput>(input).and_then(run_enter_worktree),
        "ExitWorktree" => from_value::<ExitWorktreeInput>(input).and_then(run_exit_worktree),
        "CronCreate" => from_value::<CronCreateInput>(input).and_then(run_cron_create),
        "CronDelete" => from_value::<CronDeleteInput>(input).and_then(run_cron_delete),
        "CronList" => from_value::<CronListInput>(input).and_then(run_cron_list),
        "ListMcpResources" => {
            from_value::<ListMcpResourcesInput>(input).and_then(run_list_mcp_resources)
        }
        "ReadMcpResource" => {
            from_value::<ReadMcpResourceInput>(input).and_then(run_read_mcp_resource)
        }
        "MCPSearch" => from_value::<McpSearchInput>(input).and_then(run_mcp_search),
        "TeamCreate" => from_value::<TeamCreateInput>(input).and_then(run_team_create),
        "TeamDelete" => from_value::<TeamDeleteInput>(input).and_then(run_team_delete),
        "SendMessage" => from_value::<SendMessageInput>(input).and_then(run_send_message),
        "SlashCommand" => from_value::<SlashCommandInput>(input).and_then(run_slash_command),
        _ => Err(format!("unsupported tool: {name}")),
    }
}

fn from_value<T: for<'de> Deserialize<'de>>(input: &Value) -> Result<T, String> {
    serde_json::from_value(input.clone()).map_err(|error| error.to_string())
}

fn run_bash(input: BashCommandInput) -> Result<String, String> {
    serde_json::to_string_pretty(&execute_bash(input).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

#[allow(clippy::needless_pass_by_value)]
fn run_read_file(input: ReadFileInput) -> Result<String, String> {
    to_pretty_json(read_file(&input.path, input.offset, input.limit).map_err(io_to_string)?)
}

#[allow(clippy::needless_pass_by_value)]
fn run_write_file(input: WriteFileInput) -> Result<String, String> {
    to_pretty_json(write_file(&input.path, &input.content).map_err(io_to_string)?)
}

#[allow(clippy::needless_pass_by_value)]
fn run_edit_file(input: EditFileInput) -> Result<String, String> {
    to_pretty_json(
        edit_file(
            &input.path,
            &input.old_string,
            &input.new_string,
            input.replace_all.unwrap_or(false),
            input.last_modified_at,
        )
        .map_err(io_to_string)?,
    )
}

#[allow(clippy::needless_pass_by_value)]
fn run_glob_search(input: GlobSearchInputValue) -> Result<String, String> {
    to_pretty_json(glob_search(&input.pattern, input.path.as_deref()).map_err(io_to_string)?)
}

#[allow(clippy::needless_pass_by_value)]
fn run_grep_search(input: GrepSearchInput) -> Result<String, String> {
    to_pretty_json(grep_search(&input).map_err(io_to_string)?)
}

#[allow(clippy::needless_pass_by_value)]
fn run_web_fetch(input: WebFetchInput) -> Result<String, String> {
    to_pretty_json(crate::web::execute_web_fetch(&input)?)
}

#[allow(clippy::needless_pass_by_value)]
fn run_web_search(input: WebSearchInput) -> Result<String, String> {
    to_pretty_json(crate::web::execute_web_search(&input)?)
}

fn run_todo_write(input: TodoWriteInput) -> Result<String, String> {
    to_pretty_json(execute_todo_write(input)?)
}

fn run_skill(input: SkillInput) -> Result<String, String> {
    to_pretty_json(execute_skill(input)?)
}

fn run_agent(input: AgentInput) -> Result<String, String> {
    to_pretty_json(crate::agent::execute_agent(input)?)
}

fn run_tool_search(input: ToolSearchInput) -> Result<String, String> {
    to_pretty_json(execute_tool_search(input))
}

fn run_notebook_edit(input: NotebookEditInput) -> Result<String, String> {
    to_pretty_json(crate::notebook::execute_notebook_edit(input)?)
}

fn run_brief(input: BriefInput) -> Result<String, String> {
    to_pretty_json(execute_brief(input)?)
}

fn run_config(input: ConfigInput) -> Result<String, String> {
    to_pretty_json(crate::config_tool::execute_config(input)?)
}

fn run_repl(input: ReplInput) -> Result<String, String> {
    to_pretty_json(execute_repl(input)?)
}

fn run_powershell(input: PowerShellInput) -> Result<String, String> {
    to_pretty_json(crate::powershell::execute_powershell(input).map_err(|error| error.to_string())?)
}

fn run_multi_edit(input: MultiEditInput) -> Result<String, String> {
    to_pretty_json(execute_multi_edit(input)?)
}

fn run_ask_user_question(input: AskUserQuestionInput) -> Result<String, String> {
    to_pretty_json(execute_ask_user_question(input)?)
}

fn run_lsp(input: LspInput) -> Result<String, String> {
    crate::lsp_tool::execute_lsp(input)
}

fn run_task_create(input: TaskCreateInput) -> Result<String, String> {
    crate::task::execute_task_create(input)
}

fn run_task_get(input: TaskGetInput) -> Result<String, String> {
    crate::task::execute_task_get(input)
}

fn run_task_list(input: TaskListInput) -> Result<String, String> {
    crate::task::execute_task_list(input)
}

fn run_task_update(input: TaskUpdateInput) -> Result<String, String> {
    crate::task::execute_task_update(input)
}

fn run_task_stop(input: TaskStopInput) -> Result<String, String> {
    crate::task::execute_task_stop(input)
}

fn run_enter_plan_mode(input: EnterPlanModeInput) -> Result<String, String> {
    crate::plan_mode::execute_enter_plan_mode(input)
}

fn run_exit_plan_mode(input: ExitPlanModeInput) -> Result<String, String> {
    crate::plan_mode::execute_exit_plan_mode(input)
}

fn run_enter_worktree(input: EnterWorktreeInput) -> Result<String, String> {
    crate::worktree::execute_enter_worktree(input)
}

fn run_exit_worktree(input: ExitWorktreeInput) -> Result<String, String> {
    crate::worktree::execute_exit_worktree(input)
}

fn run_cron_create(input: CronCreateInput) -> Result<String, String> {
    crate::cron::execute_cron_create(input)
}

fn run_cron_delete(input: CronDeleteInput) -> Result<String, String> {
    crate::cron::execute_cron_delete(input)
}

fn run_cron_list(input: CronListInput) -> Result<String, String> {
    crate::cron::execute_cron_list(input)
}

fn run_list_mcp_resources(input: ListMcpResourcesInput) -> Result<String, String> {
    crate::mcp_resource::execute_list_mcp_resources(input)
}

fn run_read_mcp_resource(input: ReadMcpResourceInput) -> Result<String, String> {
    crate::mcp_resource::execute_read_mcp_resource(input)
}

fn run_mcp_search(input: McpSearchInput) -> Result<String, String> {
    crate::mcp_resource::execute_mcp_search(input)
}

fn run_team_create(input: TeamCreateInput) -> Result<String, String> {
    crate::collab::execute_team_create(input)
}

fn run_team_delete(input: TeamDeleteInput) -> Result<String, String> {
    crate::collab::execute_team_delete(input)
}

fn run_send_message(input: SendMessageInput) -> Result<String, String> {
    crate::collab::execute_send_message(input)
}

fn run_slash_command(input: SlashCommandInput) -> Result<String, String> {
    crate::collab::execute_slash_command(input)
}

fn to_pretty_json<T: serde::Serialize>(value: T) -> Result<String, String> {
    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
}

#[allow(clippy::needless_pass_by_value)]
fn io_to_string(error: std::io::Error) -> String {
    error.to_string()
}

fn execute_todo_write(input: TodoWriteInput) -> Result<TodoWriteOutput, String> {
    validate_todos(&input.todos)?;
    let store_path = todo_store_path()?;
    let old_todos = if store_path.exists() {
        serde_json::from_str::<Vec<TodoItem>>(
            &std::fs::read_to_string(&store_path).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };

    let all_done = input
        .todos
        .iter()
        .all(|todo| matches!(todo.status, TodoStatus::Completed));
    let persisted = if all_done {
        Vec::new()
    } else {
        input.todos.clone()
    };

    if let Some(parent) = store_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(
        &store_path,
        serde_json::to_string_pretty(&persisted).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let verification_nudge_needed = (all_done
        && input.todos.len() >= 3
        && !input
            .todos
            .iter()
            .any(|todo| todo.content.to_lowercase().contains("verif")))
    .then_some(true);

    Ok(TodoWriteOutput {
        old_todos,
        new_todos: input.todos,
        verification_nudge_needed,
    })
}

fn execute_skill(input: SkillInput) -> Result<SkillOutput, String> {
    let skill_path = resolve_skill_path(&input.skill)?;
    let prompt = std::fs::read_to_string(&skill_path).map_err(|error| error.to_string())?;
    let description = parse_skill_description(&prompt);

    Ok(SkillOutput {
        skill: input.skill,
        path: skill_path.display().to_string(),
        args: input.args,
        description,
        prompt,
    })
}

fn validate_todos(todos: &[TodoItem]) -> Result<(), String> {
    if todos.is_empty() {
        return Err(String::from("todos must not be empty"));
    }
    // Allow multiple in_progress items for parallel workflows
    if todos.iter().any(|todo| todo.content.trim().is_empty()) {
        return Err(String::from("todo content must not be empty"));
    }
    if todos.iter().any(|todo| todo.active_form.trim().is_empty()) {
        return Err(String::from("todo activeForm must not be empty"));
    }
    Ok(())
}

fn todo_store_path() -> Result<std::path::PathBuf, String> {
    if let Ok(path) = std::env::var("CODINEER_TODO_STORE") {
        return Ok(std::path::PathBuf::from(path));
    }
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    Ok(runtime::codineer_runtime_dir(&cwd).join("todos.json"))
}

fn resolve_skill_path(skill: &str) -> Result<std::path::PathBuf, String> {
    let requested = skill.trim().trim_start_matches('/').trim_start_matches('$');
    if requested.is_empty() {
        return Err(String::from("skill must not be empty"));
    }

    if requested.contains("..") || requested.contains('/') || requested.contains('\\') {
        return Err(format!(
            "invalid skill name '{requested}': must not contain path separators or '..'"
        ));
    }

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(runtime::codineer_runtime_dir(&cwd).join("skills"));
    }
    if let Ok(codineer_home) = std::env::var("CODINEER_CONFIG_HOME") {
        candidates.push(std::path::PathBuf::from(codineer_home).join("skills"));
    }
    if let Some(home) = runtime::home_dir() {
        candidates.push(home.join(".codineer").join("skills"));
    }

    for root in candidates {
        let direct = root.join(requested).join("SKILL.md");
        if direct.exists() {
            return Ok(direct);
        }

        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path().join("SKILL.md");
                if !path.exists() {
                    continue;
                }
                if entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(requested)
                {
                    return Ok(path);
                }
            }
        }
    }

    Err(format!("unknown skill: {requested}"))
}

fn execute_tool_search(input: ToolSearchInput) -> ToolSearchOutput {
    execute_tool_search_with_context(input, None)
}

pub fn execute_tool_search_with_context(
    input: ToolSearchInput,
    pending_mcp_servers: Option<Vec<String>>,
) -> ToolSearchOutput {
    let deferred = deferred_tool_specs();
    let max_results = input.max_results.unwrap_or(5).max(1);
    let query = input.query.trim().to_string();
    let normalized_query = normalize_tool_search_query(&query);
    let matches = search_tool_specs(&query, max_results, &deferred);

    ToolSearchOutput {
        matches,
        query,
        normalized_query,
        total_deferred_tools: deferred.len(),
        pending_mcp_servers: pending_mcp_servers.filter(|servers| !servers.is_empty()),
    }
}

fn deferred_tool_specs() -> Vec<ToolSpec> {
    mvp_tool_specs()
        .into_iter()
        .filter(|spec| {
            !matches!(
                spec.name,
                "bash" | "read_file" | "write_file" | "edit_file" | "glob_search" | "grep_search"
            )
        })
        .collect()
}

fn search_tool_specs(query: &str, max_results: usize, specs: &[ToolSpec]) -> Vec<String> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let lowered = query.to_lowercase();
    if let Some(selection) = lowered.strip_prefix("select:") {
        return selection
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .filter_map(|wanted| {
                let wanted = canonical_tool_token(wanted);
                specs
                    .iter()
                    .find(|spec| canonical_tool_token(spec.name) == wanted)
                    .map(|spec| spec.name.to_string())
            })
            .take(max_results)
            .collect();
    }

    let mut required = Vec::new();
    let mut optional = Vec::new();
    for term in lowered.split_whitespace() {
        if let Some(rest) = term.strip_prefix('+') {
            if !rest.is_empty() {
                required.push(rest);
            }
        } else {
            optional.push(term);
        }
    }
    let terms = if required.is_empty() {
        optional.clone()
    } else {
        required.iter().chain(optional.iter()).copied().collect()
    };

    let mut scored = specs
        .iter()
        .filter_map(|spec| {
            let name = spec.name.to_lowercase();
            let canonical_name = canonical_tool_token(spec.name);
            let normalized_description = normalize_tool_search_query(spec.description);
            let haystack = format!(
                "{name} {} {canonical_name}",
                spec.description.to_lowercase()
            );
            let normalized_haystack = format!("{canonical_name} {normalized_description}");
            if required.iter().any(|term| !haystack.contains(term)) {
                return None;
            }

            let mut score = 0_i32;
            for term in &terms {
                let canonical_term = canonical_tool_token(term);
                if haystack.contains(term) {
                    score += 2;
                }
                if name == *term {
                    score += 8;
                }
                if name.contains(term) {
                    score += 4;
                }
                if canonical_name == canonical_term {
                    score += 12;
                }
                if normalized_haystack.contains(&canonical_term) {
                    score += 3;
                }
            }

            if score == 0 && !lowered.is_empty() {
                return None;
            }
            Some((score, spec.name.to_string()))
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    scored
        .into_iter()
        .map(|(_, name)| name)
        .take(max_results)
        .collect()
}

fn normalize_tool_search_query(query: &str) -> String {
    query
        .trim()
        .split(|ch: char| ch.is_whitespace() || ch == ',')
        .filter(|term| !term.is_empty())
        .map(canonical_tool_token)
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn canonical_tool_token(value: &str) -> String {
    let mut canonical = value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if let Some(stripped) = canonical.strip_suffix("tool") {
        canonical = stripped.to_string();
    }
    canonical
}

#[cfg(test)]
pub(crate) const MAX_SLEEP_MS: u64 = 5 * 60 * 1000;
#[cfg(not(test))]
const MAX_SLEEP_MS: u64 = 5 * 60 * 1000;

#[cfg(test)]
pub(crate) fn clamp_sleep(requested_ms: u64) -> (u64, String) {
    clamp_sleep_inner(requested_ms)
}

fn clamp_sleep_inner(requested_ms: u64) -> (u64, String) {
    let clamped = requested_ms.min(MAX_SLEEP_MS);
    let message = if clamped < requested_ms {
        format!("Slept for {clamped}ms (clamped from {requested_ms}ms)")
    } else {
        format!("Slept for {clamped}ms")
    };
    (clamped, message)
}

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn execute_sleep(input: SleepInput) -> SleepOutput {
    let (duration_ms, message) = clamp_sleep_inner(input.duration_ms);
    std::thread::sleep(Duration::from_millis(duration_ms));
    SleepOutput {
        duration_ms,
        message,
    }
}

fn execute_brief(input: BriefInput) -> Result<BriefOutput, String> {
    if input.message.trim().is_empty() {
        return Err(String::from("message must not be empty"));
    }

    let attachments = input
        .attachments
        .as_ref()
        .map(|paths| {
            paths
                .iter()
                .map(|path| resolve_attachment(path))
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?;

    let message = match input.status {
        BriefStatus::Normal | BriefStatus::Proactive => input.message,
    };

    Ok(BriefOutput {
        message,
        attachments,
        sent_at: crate::config_tool::iso8601_timestamp(),
    })
}

fn resolve_attachment(path: &str) -> Result<ResolvedAttachment, String> {
    let resolved = std::fs::canonicalize(path).map_err(|error| error.to_string())?;
    let metadata = std::fs::metadata(&resolved).map_err(|error| error.to_string())?;
    Ok(ResolvedAttachment {
        path: resolved.display().to_string(),
        size: metadata.len(),
        is_image: is_image_path(&resolved),
    })
}

fn is_image_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg")
    )
}

pub(crate) fn execute_structured_output(input: StructuredOutputInput) -> StructuredOutputResult {
    StructuredOutputResult {
        data: String::from("Structured output provided successfully"),
        structured_output: input.0,
    }
}

fn execute_repl(input: ReplInput) -> Result<ReplOutput, String> {
    if input.code.trim().is_empty() {
        return Err(String::from("code must not be empty"));
    }
    let timeout_ms = input.timeout_ms.unwrap_or(30_000).max(1_000);
    let runtime = resolve_repl_runtime(&input.language)?;
    let started = Instant::now();
    let child = Command::new(runtime.program)
        .args(runtime.args)
        .arg(&input.code)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;

    let pid = child.id();
    let timeout = Duration::from_millis(timeout_ms);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => Ok(ReplOutput {
            language: input.language,
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code().unwrap_or(1),
            duration_ms: started.elapsed().as_millis(),
        }),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => {
            kill_process(pid);
            Ok(ReplOutput {
                language: input.language,
                stdout: String::new(),
                stderr: format!("REPL execution timed out after {timeout_ms}ms"),
                exit_code: 124,
                duration_ms: started.elapsed().as_millis(),
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplLanguage {
    Python,
    JavaScript,
    Shell,
}

impl ReplLanguage {
    fn parse(input: &str) -> Result<Self, String> {
        match input.trim().to_ascii_lowercase().as_str() {
            "python" | "py" => Ok(Self::Python),
            "javascript" | "js" | "node" => Ok(Self::JavaScript),
            "sh" | "shell" | "bash" => Ok(Self::Shell),
            other => Err(format!("unsupported REPL language: {other}")),
        }
    }

    fn command_candidates(self) -> &'static [&'static str] {
        match self {
            Self::Python => &["python3", "python"],
            Self::JavaScript => &["node"],
            Self::Shell => &["bash", "sh"],
        }
    }

    fn eval_args(self) -> &'static [&'static str] {
        match self {
            Self::Python => &["-c"],
            Self::JavaScript => &["-e"],
            Self::Shell => &["-lc"],
        }
    }
}

struct ReplRuntime {
    program: &'static str,
    args: &'static [&'static str],
}

fn resolve_repl_runtime(language: &str) -> Result<ReplRuntime, String> {
    let lang = ReplLanguage::parse(language)?;
    let program = detect_first_command(lang.command_candidates())
        .ok_or_else(|| format!("{language} runtime not found"))?;
    Ok(ReplRuntime {
        program,
        args: lang.eval_args(),
    })
}

fn detect_first_command(commands: &[&'static str]) -> Option<&'static str> {
    commands
        .iter()
        .copied()
        .find(|command| crate::powershell::command_exists(command))
}

fn parse_skill_description(contents: &str) -> Option<String> {
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("description:") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn execute_multi_edit(input: MultiEditInput) -> Result<MultiEditOutput, String> {
    if input.edits.is_empty() {
        return Err(String::from("edits must not be empty"));
    }
    for (i, op) in input.edits.iter().enumerate() {
        edit_file(
            &input.path,
            &op.old_string,
            &op.new_string,
            op.replace_all.unwrap_or(false),
            None, // MultiEdit does not track per-op mtime
        )
        .map_err(|error| format!("edit[{i}] failed: {error}"))?;
    }
    Ok(MultiEditOutput {
        path: input.path,
        edits_applied: input.edits.len(),
    })
}

fn execute_ask_user_question(input: AskUserQuestionInput) -> Result<AskUserQuestionOutput, String> {
    if input.questions.is_empty() {
        return Err(String::from("questions must not be empty"));
    }
    if input.questions.len() > 4 {
        return Err(String::from("at most 4 questions are allowed per call"));
    }
    for (qi, q) in input.questions.iter().enumerate() {
        if q.question.trim().is_empty() {
            return Err(format!("questions[{qi}].question must not be empty"));
        }
        if q.options.len() < 2 {
            return Err(format!(
                "questions[{qi}] must have at least 2 options, got {}",
                q.options.len()
            ));
        }
        if q.options.len() > 26 {
            return Err(format!(
                "questions[{qi}] must have at most 26 options, got {}",
                q.options.len()
            ));
        }
    }

    let formatted_message = format_questions(&input.questions);
    Ok(AskUserQuestionOutput {
        questions: input.questions,
        formatted_message,
        pending_user_response: true,
    })
}

fn format_questions(questions: &[UserQuestion]) -> String {
    let mut out = String::from("Please answer the following question(s):\n\n");
    for (i, q) in questions.iter().enumerate() {
        if let Some(header) = &q.header {
            out.push_str(&format!("**{}**\n", header));
        }
        let select_hint = if q.multi_select {
            " (select one or more)"
        } else {
            " (select one)"
        };
        out.push_str(&format!("{}. {}{}\n", i + 1, q.question, select_hint));
        for (oi, opt) in q.options.iter().enumerate() {
            out.push_str(&format_option(oi, opt));
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn format_option(index: usize, opt: &QuestionOption) -> String {
    // index is validated to be 0..=25 by execute_ask_user_question
    let letter = char::from(b'a' + index as u8);
    match &opt.description {
        Some(desc) if !desc.trim().is_empty() => {
            format!("  {letter}) {} — {}\n", opt.label, desc)
        }
        _ => format!("  {letter}) {}\n", opt.label),
    }
}

pub(crate) fn kill_process(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-9", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

#[cfg(test)]
mod tests;
