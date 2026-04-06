use std::env;
use std::fs;
use std::process::Command;
use std::sync::Arc;

use crate::input;
use commands::{
    handle_agents_slash_command, handle_branch_slash_command, handle_commit_push_pr_slash_command,
    handle_plugins_slash_command, handle_skills_slash_command, handle_worktree_slash_command,
    CommitPushPrRequest, SlashCommand,
};
use runtime::{
    CompactionConfig, ConfigLoader, ConversationRuntime, LspContextEnrichment, LspManager,
    PermissionMode, Session,
};
use serde_json::json;

use crate::cli::{
    create_mcp_manager, permission_mode_from_label, resolve_model_alias, AllowedToolSet,
    CliOutputFormat, SharedMcpManager,
};
use crate::help::{render_repl_help, render_unknown_repl_command, slash_command_entries};
use crate::progress::{InternalPromptProgressReporter, InternalPromptProgressRun};
use crate::reports::{
    format_compact_report, format_cost_report, format_model_report, format_model_switch_report,
    format_permissions_report, format_permissions_switch_report, format_resume_report,
    format_status_report, normalize_permission_mode, render_config_report, render_diff_report,
    render_export_text, render_last_tool_debug_report, render_memory_report,
    render_teleport_report, render_version_report, resolve_export_path, status_context,
    StatusUsage,
};
use crate::runtime_client::{
    build_runtime, CliPermissionPrompter, CliToolExecutor, DefaultRuntimeClient, RuntimeParams,
};
use crate::session_store::{
    create_managed_session_handle, render_session_list, resolve_session_reference, SessionHandle,
};
use crate::workspace::{
    command_exists, git_output, git_status_ok, parse_commit_push_pr_output, parse_titled_body,
    recent_user_context, sanitize_generated_message, truncate_for_prompt, write_temp_text_file,
};

use crate::{
    banner::{tilde_session_path, welcome_banner, BannerContext},
    build_plugin_manager, build_system_prompt, build_system_prompt_with_lsp, run_init,
    terminal_width::start_resize_monitor,
};
pub(crate) struct LiveCli {
    model: String,
    allowed_tools: Option<AllowedToolSet>,
    permission_mode: PermissionMode,
    system_prompt: Vec<String>,
    runtime: ConversationRuntime<DefaultRuntimeClient, CliToolExecutor>,
    session: SessionHandle,
    mcp_manager: SharedMcpManager,
    lsp_manager: Option<LspManager>,
}

impl LiveCli {
    fn runtime_params(&self, session: Session, emit_output: bool) -> RuntimeParams {
        RuntimeParams {
            session,
            model: self.model.clone(),
            system_prompt: self.system_prompt.clone(),
            enable_tools: true,
            emit_output,
            allowed_tools: self.allowed_tools.clone(),
            permission_mode: self.permission_mode,
            progress_reporter: None,
            mcp_manager: Arc::clone(&self.mcp_manager),
        }
    }

    pub(crate) fn new(
        model: String,
        enable_tools: bool,
        allowed_tools: Option<AllowedToolSet>,
        permission_mode: PermissionMode,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let system_prompt = build_system_prompt()?;
        let session = create_managed_session_handle()?;
        let mcp_manager = create_mcp_manager();
        let (runtime, resolved_model) = build_runtime(RuntimeParams {
            session: Session::new(),
            model: model.clone(),
            system_prompt: system_prompt.clone(),
            enable_tools,
            emit_output: true,
            allowed_tools: allowed_tools.clone(),
            permission_mode,
            progress_reporter: None,
            mcp_manager: Arc::clone(&mcp_manager),
        })?;
        let lsp_manager = env::current_dir()
            .ok()
            .and_then(|cwd| crate::lsp_detect::detect_lsp_servers(&cwd));
        let cli = Self {
            model: resolved_model,
            allowed_tools,
            permission_mode,
            system_prompt,
            runtime,
            session,
            mcp_manager,
            lsp_manager,
        };
        cli.persist_session()?;
        Ok(cli)
    }

    /// Run a single conversation turn.  Errors are printed to the terminal
    /// but never propagated — the REPL must stay alive.
    fn run_turn(&mut self, input: &str) {
        self.run_turn_blocks(vec![runtime::ContentBlock::Text {
            text: input.to_string(),
        }]);
    }

    fn run_turn_blocks(&mut self, blocks: Vec<runtime::ContentBlock>) {
        if let Some(enrichment) = self.collect_lsp_diagnostics() {
            if let Ok(refreshed) = build_system_prompt_with_lsp(Some(&enrichment)) {
                self.system_prompt = refreshed;
                self.runtime
                    .update_system_prompt(self.system_prompt.clone());
            }
        }

        let pe = crate::style::Palette::for_stderr();
        eprintln!("{}  ∴ thinking…{}", pe.dim, pe.r);
        let mut permission_prompter = CliPermissionPrompter::new(self.permission_mode);
        let result = self
            .runtime
            .run_turn_with_blocks(blocks, Some(&mut permission_prompter));
        println!();
        match result {
            Ok(_) => {
                if let Err(e) = self.persist_session() {
                    let p = crate::style::Palette::for_stdout();
                    eprintln!(
                        "{}  ⎿  {}{}Warning: failed to save session: {}{}",
                        p.dim, p.r, p.bold_yellow, e, p.r
                    );
                }
            }
            Err(error) => {
                let p = crate::style::Palette::for_stdout();
                let indent = "         ";
                println!(
                    "{}  ⎿  {}{}Error: {}{}",
                    p.dim,
                    p.r,
                    p.red_fg,
                    error.to_string().replace('\n', &format!("\n{indent}")),
                    p.r
                );
            }
        }
    }

    pub(crate) fn run_turn_with_output(
        &mut self,
        input: &str,
        output_format: CliOutputFormat,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match output_format {
            CliOutputFormat::Text => {
                self.run_turn(input);
                Ok(())
            }
            CliOutputFormat::Json => self.run_prompt_json(input),
        }
    }

    fn run_prompt_json(&mut self, input: &str) -> Result<(), Box<dyn std::error::Error>> {
        let session = self.runtime.session().clone();
        let (mut runtime, _) = build_runtime(self.runtime_params(session, false))?;
        let mut permission_prompter = CliPermissionPrompter::new(self.permission_mode);
        let summary = runtime.run_turn(input, Some(&mut permission_prompter))?;
        self.runtime = runtime;
        self.persist_session()?;
        println!(
            "{}",
            json!({
                "message": final_assistant_text(&summary),
                "model": self.model,
                "iterations": summary.iterations,
                "tool_uses": collect_tool_uses(&summary),
                "tool_results": collect_tool_results(&summary),
                "usage": {
                    "input_tokens": summary.usage.input_tokens,
                    "output_tokens": summary.usage.output_tokens,
                    "cache_creation_input_tokens": summary.usage.cache_creation_input_tokens,
                    "cache_read_input_tokens": summary.usage.cache_read_input_tokens,
                }
            })
        );
        Ok(())
    }

    fn handle_repl_command(
        &mut self,
        command: SlashCommand,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        Ok(match command {
            SlashCommand::Help => {
                println!("{}", render_repl_help());
                false
            }
            SlashCommand::Status => {
                self.print_status();
                false
            }
            SlashCommand::Cost => {
                self.print_cost();
                false
            }
            SlashCommand::Compact => {
                self.compact()?;
                false
            }
            SlashCommand::Init => {
                run_init()?;
                false
            }
            SlashCommand::Diff => {
                Self::print_diff()?;
                false
            }
            SlashCommand::Version => {
                Self::print_version();
                false
            }
            SlashCommand::Memory => {
                Self::print_memory()?;
                false
            }
            SlashCommand::DebugToolCall => {
                self.run_debug_tool_call()?;
                false
            }
            SlashCommand::Commit => {
                self.run_commit()?;
                true
            }
            SlashCommand::Bughunter { scope } => {
                self.run_bughunter(scope.as_deref())?;
                false
            }
            SlashCommand::Pr { context } => {
                self.run_pr(context.as_deref())?;
                false
            }
            SlashCommand::Issue { context } => {
                self.run_issue(context.as_deref())?;
                false
            }
            SlashCommand::Ultraplan { task } => {
                self.run_ultraplan(task.as_deref())?;
                false
            }
            SlashCommand::Teleport { target } => {
                Self::run_teleport(target.as_deref())?;
                false
            }
            SlashCommand::Export { path } => {
                self.export_session(path.as_deref())?;
                false
            }
            SlashCommand::Config { section } => {
                Self::print_config(section.as_deref())?;
                false
            }
            SlashCommand::Agents { args } => {
                Self::print_agents(args.as_deref())?;
                false
            }
            SlashCommand::Skills { args } => {
                Self::print_skills(args.as_deref())?;
                false
            }
            SlashCommand::Model { model } => self.set_model(model)?,
            SlashCommand::Permissions { mode } => self.set_permissions(mode)?,
            SlashCommand::Clear { confirm } => self.clear_session(confirm)?,
            SlashCommand::Resume { session_path } => self.resume_session(session_path)?,
            SlashCommand::Session { action, target } => {
                self.handle_session_command(action.as_deref(), target.as_deref())?
            }
            SlashCommand::Plugins { action, target } => {
                self.handle_plugins_command(action.as_deref(), target.as_deref())?
            }
            SlashCommand::Branch { action, target } => {
                let cwd = env::current_dir()?;
                println!(
                    "{}",
                    handle_branch_slash_command(action.as_deref(), target.as_deref(), &cwd)?
                );
                false
            }
            SlashCommand::Worktree {
                action,
                path,
                branch,
            } => {
                let cwd = env::current_dir()?;
                println!(
                    "{}",
                    handle_worktree_slash_command(
                        action.as_deref(),
                        path.as_deref(),
                        branch.as_deref(),
                        &cwd,
                    )?
                );
                false
            }
            SlashCommand::CommitPushPr { context } => {
                self.run_commit_push_pr(context.as_deref())?;
                true
            }
            SlashCommand::Unknown(name) => {
                eprintln!("{}", render_unknown_repl_command(&name));
                false
            }
        })
    }

    fn persist_session(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.runtime.session().save_to_path(&self.session.path)?;
        Ok(())
    }

    fn collect_lsp_diagnostics(&self) -> Option<LspContextEnrichment> {
        let manager = self.lsp_manager.as_ref()?;
        let rt = tokio::runtime::Runtime::new().ok()?;
        let diagnostics = rt.block_on(manager.collect_workspace_diagnostics()).ok()?;
        let enrichment = LspContextEnrichment {
            file_path: env::current_dir().unwrap_or_default(),
            diagnostics,
            definitions: Vec::new(),
            references: Vec::new(),
        };
        if enrichment.is_empty() {
            None
        } else {
            Some(enrichment)
        }
    }

    fn shutdown_mcp(&self) {
        if let Ok(rt) = tokio::runtime::Runtime::new() {
            if let Ok(mut guard) = self.mcp_manager.lock() {
                let _ = rt.block_on(guard.shutdown());
            }
        }
    }

    fn shutdown_lsp(&self) {
        if let Some(manager) = &self.lsp_manager {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                let _ = rt.block_on(manager.shutdown());
            }
        }
    }

    fn print_status(&self) {
        let cumulative = self.runtime.usage().cumulative_usage();
        let latest = self.runtime.usage().current_turn_usage();
        println!(
            "{}",
            format_status_report(
                &self.model,
                StatusUsage {
                    message_count: self.runtime.session().messages.len(),
                    turns: self.runtime.usage().turns(),
                    latest,
                    cumulative,
                    estimated_tokens: self.runtime.estimated_tokens(),
                },
                self.permission_mode.as_str(),
                &status_context(Some(&self.session.path)).unwrap_or_default(),
            )
        );
    }

    fn set_model(&mut self, model: Option<String>) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(model) = model else {
            println!(
                "{}",
                format_model_report(
                    &self.model,
                    self.runtime.session().messages.len(),
                    self.runtime.usage().turns(),
                )
            );
            return Ok(false);
        };

        let model = resolve_model_alias(&model);

        if model == self.model {
            println!(
                "{}",
                format_model_report(
                    &self.model,
                    self.runtime.session().messages.len(),
                    self.runtime.usage().turns(),
                )
            );
            return Ok(false);
        }

        let previous = self.model.clone();
        let session = self.runtime.session().clone();
        let message_count = session.messages.len();
        let mut params = self.runtime_params(session, true);
        params.model = model.clone();
        let (runtime, resolved_model) = build_runtime(params)?;
        self.runtime = runtime;
        self.model.clone_from(&resolved_model);
        println!(
            "{}",
            format_model_switch_report(&previous, &model, message_count)
        );
        Ok(true)
    }

    fn set_permissions(
        &mut self,
        mode: Option<String>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(mode) = mode else {
            println!(
                "{}",
                format_permissions_report(self.permission_mode.as_str())
            );
            return Ok(false);
        };

        let normalized = normalize_permission_mode(&mode).ok_or_else(|| {
            format!(
                "unsupported permission mode '{mode}'. Use read-only, workspace-write, or danger-full-access."
            )
        })?;

        if normalized == self.permission_mode.as_str() {
            println!("{}", format_permissions_report(normalized));
            return Ok(false);
        }

        let previous = self.permission_mode.as_str().to_string();
        let session = self.runtime.session().clone();
        self.permission_mode = permission_mode_from_label(normalized)?;
        self.runtime = build_runtime(self.runtime_params(session, true))?.0;
        println!(
            "{}",
            format_permissions_switch_report(&previous, normalized)
        );
        Ok(true)
    }

    fn clear_session(&mut self, confirm: bool) -> Result<bool, Box<dyn std::error::Error>> {
        if !confirm {
            println!(
                "clear: confirmation required; run /clear --confirm to start a fresh session."
            );
            return Ok(false);
        }

        self.session = create_managed_session_handle()?;
        self.runtime = build_runtime(self.runtime_params(Session::new(), true))?.0;
        println!(
            "Session cleared\n  Mode             fresh session\n  Preserved model  {}\n  Permission mode  {}\n  Session          {}",
            self.model,
            self.permission_mode.as_str(),
            self.session.id,
        );
        Ok(true)
    }

    fn print_cost(&self) {
        let cumulative = self.runtime.usage().cumulative_usage();
        println!("{}", format_cost_report(cumulative));
    }

    fn activate_session(
        &mut self,
        handle: SessionHandle,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        let session = Session::load_from_path(&handle.path)?;
        let count = session.messages.len();
        self.runtime = build_runtime(self.runtime_params(session, true))?.0;
        self.session = handle;
        Ok(count)
    }

    fn resume_session(
        &mut self,
        session_path: Option<String>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(session_ref) = session_path else {
            println!("Usage: /resume <session-path>");
            return Ok(false);
        };
        let count = self.activate_session(resolve_session_reference(&session_ref)?)?;
        println!(
            "{}",
            format_resume_report(
                &self.session.path.display().to_string(),
                count,
                self.runtime.usage().turns(),
            )
        );
        Ok(true)
    }

    fn print_config(section: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        println!("{}", render_config_report(section)?);
        Ok(())
    }

    fn print_memory() -> Result<(), Box<dyn std::error::Error>> {
        println!("{}", render_memory_report()?);
        Ok(())
    }

    pub(crate) fn print_agents(args: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let cwd = env::current_dir()?;
        println!("{}", handle_agents_slash_command(args, &cwd)?);
        Ok(())
    }

    pub(crate) fn print_skills(args: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let cwd = env::current_dir()?;
        println!("{}", handle_skills_slash_command(args, &cwd)?);
        Ok(())
    }

    fn print_diff() -> Result<(), Box<dyn std::error::Error>> {
        println!("{}", render_diff_report()?);
        Ok(())
    }

    fn print_version() {
        println!("{}", render_version_report());
    }

    fn export_session(
        &self,
        requested_path: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let export_path = resolve_export_path(requested_path, self.runtime.session())?;
        fs::write(&export_path, render_export_text(self.runtime.session()))?;
        println!(
            "Export\n  Result           wrote transcript\n  File             {}\n  Messages         {}",
            export_path.display(),
            self.runtime.session().messages.len(),
        );
        Ok(())
    }

    fn handle_session_command(
        &mut self,
        action: Option<&str>,
        target: Option<&str>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        match action {
            None | Some("list") => {
                println!("{}", render_session_list(&self.session.id)?);
                Ok(false)
            }
            Some("switch") => {
                let Some(target) = target else {
                    println!("Usage: /session switch <session-id>");
                    return Ok(false);
                };
                let count = self.activate_session(resolve_session_reference(target)?)?;
                println!(
                    "Session switched\n  Active session   {}\n  File             {}\n  Messages         {}",
                    self.session.id,
                    self.session.path.display(),
                    count,
                );
                Ok(true)
            }
            Some(other) => {
                println!("Unknown /session action '{other}'. Use /session list or /session switch <session-id>.");
                Ok(false)
            }
        }
    }

    fn handle_plugins_command(
        &mut self,
        action: Option<&str>,
        target: Option<&str>,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let cwd = env::current_dir()?;
        let loader = ConfigLoader::default_for(&cwd);
        let runtime_config = loader.load()?;
        let mut manager = build_plugin_manager(&cwd, &loader, &runtime_config);
        let result = handle_plugins_slash_command(action, target, &mut manager)?;
        println!("{}", result.message);
        if result.effect == commands::PluginEffect::ReloadRuntime {
            self.reload_runtime_features()?;
        }
        Ok(false)
    }

    fn reload_runtime_features(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let session = self.runtime.session().clone();
        self.runtime = build_runtime(self.runtime_params(session, true))?.0;
        self.persist_session()
    }

    fn compact(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let result = self.runtime.compact(CompactionConfig::default());
        let removed = result.removed_message_count;
        let kept = result.compacted_session.messages.len();
        let skipped = removed == 0;
        self.runtime = build_runtime(self.runtime_params(result.compacted_session, true))?.0;
        self.persist_session()?;
        println!("{}", format_compact_report(removed, kept, skipped));
        Ok(())
    }

    fn run_internal_prompt_text_with_progress(
        &self,
        prompt: &str,
        enable_tools: bool,
        progress: Option<InternalPromptProgressReporter>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let session = self.runtime.session().clone();
        let mut params = self.runtime_params(session, false);
        params.enable_tools = enable_tools;
        params.progress_reporter = progress;
        let (mut runtime, _) = build_runtime(params)?;
        let mut permission_prompter = CliPermissionPrompter::new(self.permission_mode);
        let summary = runtime.run_turn(prompt, Some(&mut permission_prompter))?;
        Ok(final_assistant_text(&summary).trim().to_string())
    }

    fn run_internal_prompt_text(
        &self,
        prompt: &str,
        enable_tools: bool,
    ) -> Result<String, Box<dyn std::error::Error>> {
        self.run_internal_prompt_text_with_progress(prompt, enable_tools, None)
    }

    fn run_bughunter(&self, scope: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let scope = scope.unwrap_or("the current repository");
        let prompt = format!(
            "You are /bughunter. Inspect {scope} and identify the most likely bugs or correctness issues. Prioritize concrete findings with file paths, severity, and suggested fixes. Use tools if needed."
        );
        println!("{}", self.run_internal_prompt_text(&prompt, true)?);
        Ok(())
    }

    fn run_ultraplan(&self, task: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let task = task.unwrap_or("the current repo work");
        let prompt = format!(
            "You are /ultraplan. Produce a deep multi-step execution plan for {task}. Include goals, risks, implementation sequence, verification steps, and rollback considerations. Use tools if needed."
        );
        let mut progress = InternalPromptProgressRun::start_ultraplan(task);
        match self.run_internal_prompt_text_with_progress(&prompt, true, Some(progress.reporter()))
        {
            Ok(plan) => {
                progress.finish_success();
                println!("{plan}");
                Ok(())
            }
            Err(error) => {
                progress.finish_failure(&error.to_string());
                Err(error)
            }
        }
    }

    fn run_teleport(target: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let Some(target) = target.map(str::trim).filter(|value| !value.is_empty()) else {
            println!("Usage: /teleport <symbol-or-path>");
            return Ok(());
        };

        println!("{}", render_teleport_report(target)?);
        Ok(())
    }

    fn run_debug_tool_call(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("{}", render_last_tool_debug_report(self.runtime.session())?);
        Ok(())
    }

    fn run_commit(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let status = git_output(&["status", "--short"])?;
        if status.trim().is_empty() {
            println!("Commit\n  Result           skipped\n  Reason           no workspace changes");
            return Ok(());
        }

        git_status_ok(&["add", "-A"])?;
        let staged_stat = git_output(&["diff", "--cached", "--stat"])?;
        let prompt = format!(
            "Generate a git commit message in plain text Lore format only. Base it on this staged diff summary:\n\n{}\n\nRecent conversation context:\n{}",
            truncate_for_prompt(&staged_stat, 8_000),
            recent_user_context(self.runtime.session(), 6)
        );
        let message = sanitize_generated_message(&self.run_internal_prompt_text(&prompt, false)?);
        if message.trim().is_empty() {
            return Err("generated commit message was empty".into());
        }

        let path = write_temp_text_file("codineer-commit-message.txt", &message)?;
        let output = Command::new("git")
            .args(["commit", "--file"])
            .arg(&path)
            .current_dir(env::current_dir()?)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(format!("git commit failed: {stderr}").into());
        }

        println!(
            "Commit\n  Result           created\n  Message file     {}\n\n{}",
            path.display(),
            message.trim()
        );
        Ok(())
    }

    fn run_pr(&self, context: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let staged = git_output(&["diff", "--stat"])?;
        let prompt = format!(
            "Generate a pull request title and body from this conversation and diff summary. Output plain text in this format exactly:\nTITLE: <title>\nBODY:\n<body markdown>\n\nContext hint: {}\n\nDiff summary:\n{}",
            context.unwrap_or("none"),
            truncate_for_prompt(&staged, 10_000)
        );
        let draft = sanitize_generated_message(&self.run_internal_prompt_text(&prompt, false)?);
        let (title, body) = parse_titled_body(&draft)
            .ok_or_else(|| "failed to parse generated PR title/body".to_string())?;

        if command_exists("gh") {
            let body_path = write_temp_text_file("codineer-pr-body.md", &body)?;
            let output = Command::new("gh")
                .args(["pr", "create", "--title", &title, "--body-file"])
                .arg(&body_path)
                .current_dir(env::current_dir()?)
                .output()?;
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                println!(
                    "PR\n  Result           created\n  Title            {title}\n  URL              {}",
                    if stdout.is_empty() { "<unknown>" } else { &stdout }
                );
                return Ok(());
            }
        }

        println!("PR draft\n  Title            {title}\n\n{body}");
        Ok(())
    }

    fn run_commit_push_pr(&self, context: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let diff = git_output(&["diff", "--stat"])?;
        let prompt = format!(
            "Generate a commit message, PR title, and PR body from this conversation and diff. Output plain text in this format exactly:\nCOMMIT: <commit message>\nTITLE: <pr title>\nBODY:\n<pr body markdown>\nBRANCH_HINT: <short branch name hint>\n\nContext hint: {}\n\nDiff summary:\n{}\n\nRecent conversation:\n{}",
            context.unwrap_or("none"),
            truncate_for_prompt(&diff, 8_000),
            recent_user_context(self.runtime.session(), 6)
        );
        let draft = sanitize_generated_message(&self.run_internal_prompt_text(&prompt, false)?);
        let (commit_msg, title, body, branch_hint) = parse_commit_push_pr_output(&draft)?;
        let cwd = env::current_dir()?;
        let report = handle_commit_push_pr_slash_command(
            &CommitPushPrRequest {
                commit_message: Some(commit_msg),
                pr_title: title.clone(),
                pr_body: body,
                branch_name_hint: branch_hint.unwrap_or(title),
            },
            &cwd,
        )?;
        println!("{report}");
        Ok(())
    }

    fn run_issue(&self, context: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let prompt = format!(
            "Generate a GitHub issue title and body from this conversation. Output plain text in this format exactly:\nTITLE: <title>\nBODY:\n<body markdown>\n\nContext hint: {}\n\nConversation context:\n{}",
            context.unwrap_or("none"),
            truncate_for_prompt(&recent_user_context(self.runtime.session(), 10), 10_000)
        );
        let draft = sanitize_generated_message(&self.run_internal_prompt_text(&prompt, false)?);
        let (title, body) = parse_titled_body(&draft)
            .ok_or_else(|| "failed to parse generated issue title/body".to_string())?;

        if command_exists("gh") {
            let body_path = write_temp_text_file("codineer-issue-body.md", &body)?;
            let output = Command::new("gh")
                .args(["issue", "create", "--title", &title, "--body-file"])
                .arg(&body_path)
                .current_dir(env::current_dir()?)
                .output()?;
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                println!(
                    "Issue\n  Result           created\n  Title            {title}\n  URL              {}",
                    if stdout.is_empty() { "<unknown>" } else { &stdout }
                );
                return Ok(());
            }
        }

        println!("Issue draft\n  Title            {title}\n\n{body}");
        Ok(())
    }
}

pub(crate) fn run_repl(
    model: String,
    allowed_tools: Option<AllowedToolSet>,
    permission_mode: PermissionMode,
    resume_path: Option<std::path::PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    start_resize_monitor();
    let mut cli = LiveCli::new(model, true, allowed_tools, permission_mode)?;

    // Restore a previous session if requested.  An empty session (0 messages)
    // is still valid — the user just wants to continue with the same session
    // file so that future history is appended to it.
    if let Some(path) = resume_path {
        let handle = crate::session_store::SessionHandle::from_path(path)?;
        let count = cli.activate_session(handle)?;
        println!(
            "{}",
            format_resume_report(&cli.session.path.display().to_string(), count, 0,)
        );
    }
    let p = crate::style::Palette::for_stdout();
    let prompt_string;
    let prompt = if p.violet.is_empty() {
        "❯ "
    } else {
        prompt_string = format!("{}❯{} ", p.violet, p.r);
        &prompt_string
    };
    // Capture banner context as owned data so the prefix closure can
    // regenerate the banner at the current terminal width on each resize.
    let color = crate::style::color_for_stdout();
    let cwd = env::current_dir().ok();
    let cwd_display = cwd
        .as_ref()
        .map_or_else(|| "<unknown>".to_string(), |p| p.display().to_string());
    let workspace_name = cwd
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("workspace")
        .to_string();
    let git_branch = status_context(Some(&cli.session.path))
        .ok()
        .and_then(|c| c.git_branch);
    let workspace_summary = git_branch.as_deref().map_or_else(
        || workspace_name.clone(),
        |b| format!("{workspace_name} · {b}"),
    );
    let has_codineer_md = cwd
        .as_ref()
        .is_some_and(|p| p.join("CODINEER.md").is_file());
    let b_model = cli.model.clone();
    let b_perms = cli.permission_mode.as_str().to_string();
    let b_session_id = cli.session.id.clone();
    let b_session_path = cli.session.path.clone();
    let hint_line = format!("{}? for shortcuts  ·  /help  ·  Esc clears{}", p.dim, p.r);
    let mut editor = input::LineEditor::new(prompt, slash_command_entries())
        .with_separator()
        .with_hint_line(hint_line)
        .with_prefix(move || {
            welcome_banner(
                color,
                BannerContext {
                    workspace_summary: &workspace_summary,
                    cwd_display: &cwd_display,
                    model: &b_model,
                    permissions: &b_perms,
                    session_id: &b_session_id,
                    session_path: &b_session_path,
                    has_codineer_md,
                },
            )
        });

    loop {
        match editor.read_line()? {
            input::ReadOutcome::Submit(payload) => {
                let input = &payload.text;
                let trimmed = input.trim();
                if trimmed.is_empty() && payload.images.is_empty() {
                    continue;
                }
                if matches!(trimmed, "/exit" | "/quit") {
                    let _ = cli.persist_session();
                    print_goodbye(&cli.session.path);
                    break;
                }
                if let Some(shell_cmd) = trimmed.strip_prefix('!') {
                    let shell_cmd = shell_cmd.trim();
                    if !shell_cmd.is_empty() {
                        editor.push_history(input);
                        let prompt = format!(
                            "Run this exact shell command and show me the output: `{shell_cmd}`"
                        );
                        cli.run_turn(&prompt);
                    }
                    continue;
                }
                if let Some(command) = SlashCommand::parse(trimmed) {
                    match cli.handle_repl_command(command) {
                        Ok(true) => {
                            let _ = cli.persist_session();
                        }
                        Err(e) => {
                            let p = crate::style::Palette::for_stdout();
                            eprintln!("{}  ⎿  {}{}Error: {}{}", p.dim, p.r, p.red_fg, e, p.r);
                        }
                        _ => {}
                    }
                    continue;
                }
                editor.push_history(input);

                let extra_images: Vec<runtime::ContentBlock> = payload
                    .images
                    .iter()
                    .filter_map(|img| {
                        crate::image_util::bytes_to_image_block(&img.bytes, Some(&img.media_type))
                            .ok()
                    })
                    .collect();

                let enriched = process_at_mentioned_files(input, extra_images);
                cli.run_turn_blocks(enriched.blocks);
            }
            input::ReadOutcome::Exit => {
                let _ = cli.persist_session();
                print_goodbye(&cli.session.path);
                break;
            }
        }
    }

    cli.shutdown_lsp();
    cli.shutdown_mcp();
    Ok(())
}

/// Print a farewell message including the full `codineer --resume` command so
/// users can always copy the path from their terminal scrollback.
fn print_goodbye(session_path: &std::path::Path) {
    let p = crate::style::Palette::for_stdout();
    let resume = tilde_session_path(session_path);
    println!("Goodbye!");
    println!();
    println!(
        "{}Resume this session:{}\n  codineer --resume {}",
        p.dim,
        p.r,
        resume.display()
    );
}

use crate::turn_helpers::{
    collect_tool_results, collect_tool_uses, final_assistant_text, process_at_mentioned_files,
};
