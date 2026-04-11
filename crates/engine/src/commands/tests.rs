use super::discovery::{
    load_agents_from_roots, load_skills_from_roots, parse_skill_frontmatter, render_agents_report,
    render_skills_report, DefinitionSource, SkillRoot,
};
use super::slash_spec::SlashCommand;
use super::{
    handle_branch_slash_command, handle_commit_slash_command, handle_plugins_slash_command,
    handle_slash_command_simple, handle_worktree_slash_command, render_plugins_report,
    render_slash_command_help, resume_supported_slash_commands, slash_command_specs,
    suggest_slash_commands, PluginEffect,
};
use crate::{CompactionConfig, ContentBlock, ConversationMessage, MessageRole, Session};
use aineer_plugins::{
    PluginKind, PluginManager, PluginManagerConfig, PluginMetadata, PluginSummary,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use {
    super::{handle_commit_push_pr_slash_command, CommitPushPrRequest},
    std::env,
    std::os::unix::fs::PermissionsExt,
    std::sync::{Mutex, OnceLock},
};

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("commands-plugin-{label}-{nanos}"))
}

#[cfg(unix)]
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env lock")
}

fn run_command(cwd: &Path, program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("command should run");
    assert!(
        output.status.success(),
        "{} {} failed: {}",
        program,
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout should be utf8")
}

fn init_git_repo(label: &str) -> PathBuf {
    let root = temp_dir(label);
    fs::create_dir_all(&root).expect("repo root");

    let init = Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&root)
        .output()
        .expect("git init should run");
    if !init.status.success() {
        let fallback = Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output()
            .expect("fallback git init should run");
        assert!(
            fallback.status.success(),
            "fallback git init should succeed"
        );
        let rename = Command::new("git")
            .args(["branch", "-m", "main"])
            .current_dir(&root)
            .output()
            .expect("git branch -m should run");
        assert!(rename.status.success(), "git branch -m main should succeed");
    }

    run_command(&root, "git", &["config", "user.name", "Aineer Tests"]);
    run_command(
        &root,
        "git",
        &["config", "user.email", "aineer@example.com"],
    );
    fs::write(root.join("README.md"), "seed\n").expect("seed file");
    run_command(&root, "git", &["add", "README.md"]);
    run_command(&root, "git", &["commit", "-m", "chore: seed repo"]);
    root
}

#[cfg(unix)]
fn init_bare_repo(label: &str) -> PathBuf {
    let root = temp_dir(label);
    let output = Command::new("git")
        .args(["init", "--bare"])
        .arg(&root)
        .output()
        .expect("bare repo should initialize");
    assert!(output.status.success(), "git init --bare should succeed");
    root
}

#[cfg(unix)]
fn write_fake_gh(bin_dir: &Path, log_path: &Path, url: &str) {
    fs::create_dir_all(bin_dir).expect("bin dir");
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'gh 1.0.0'\n  exit 0\nfi\nprintf '%s\\n' \"$*\" >> \"{}\"\nif [ \"$1\" = \"pr\" ] && [ \"$2\" = \"create\" ]; then\n  echo '{}'\n  exit 0\nfi\nif [ \"$1\" = \"pr\" ] && [ \"$2\" = \"view\" ]; then\n  echo '{{\"url\":\"{}\"}}'\n  exit 0\nfi\nexit 0\n",
        log_path.display(),
        url,
        url,
    );
    let path = bin_dir.join("gh");
    fs::write(&path, script).expect("gh stub");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("chmod");
}

fn write_external_plugin(root: &Path, name: &str, version: &str) {
    fs::create_dir_all(root).expect("plugin dir");
    fs::write(
        root.join("plugin.json"),
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"version\": \"{version}\",\n  \"description\": \"commands plugin\"\n}}"
        ),
    )
    .expect("write manifest");
}

fn write_bundled_plugin(root: &Path, name: &str, version: &str, default_enabled: bool) {
    fs::create_dir_all(root).expect("plugin dir");
    fs::write(
        root.join("plugin.json"),
        format!(
            "{{\n  \"name\": \"{name}\",\n  \"version\": \"{version}\",\n  \"description\": \"bundled commands plugin\",\n  \"defaultEnabled\": {}\n}}",
            if default_enabled { "true" } else { "false" }
        ),
    )
    .expect("write bundled manifest");
}

fn write_agent(root: &Path, name: &str, description: &str, model: &str, reasoning: &str) {
    fs::create_dir_all(root).expect("agent root");
    fs::write(
        root.join(format!("{name}.toml")),
        format!(
            "name = \"{name}\"\ndescription = \"{description}\"\nmodel = \"{model}\"\nmodel_reasoning_effort = \"{reasoning}\"\n"
        ),
    )
    .expect("write agent");
}

fn write_skill(root: &Path, name: &str, description: &str) {
    let skill_root = root.join(name);
    fs::create_dir_all(&skill_root).expect("skill root");
    fs::write(
        skill_root.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n"),
    )
    .expect("write skill");
}

#[allow(clippy::too_many_lines)]
#[test]
fn parses_supported_slash_commands() {
    assert_eq!(SlashCommand::parse("/help"), Some(SlashCommand::Help));
    assert_eq!(SlashCommand::parse(" /status "), Some(SlashCommand::Status));
    assert_eq!(
        SlashCommand::parse("/bughunter runtime"),
        Some(SlashCommand::Bughunter {
            scope: Some("runtime".to_string())
        })
    );
    assert_eq!(
        SlashCommand::parse("/branch create feature/demo"),
        Some(SlashCommand::Branch {
            action: Some("create".to_string()),
            target: Some("feature/demo".to_string()),
        })
    );
    assert_eq!(
        SlashCommand::parse("/worktree add ../demo wt-demo"),
        Some(SlashCommand::Worktree {
            action: Some("add".to_string()),
            path: Some("../demo".to_string()),
            branch: Some("wt-demo".to_string()),
        })
    );
    assert_eq!(SlashCommand::parse("/commit"), Some(SlashCommand::Commit));
    assert_eq!(
        SlashCommand::parse("/commit-push-pr ready for review"),
        Some(SlashCommand::CommitPushPr {
            context: Some("ready for review".to_string())
        })
    );
    assert_eq!(
        SlashCommand::parse("/pr ready for review"),
        Some(SlashCommand::Pr {
            context: Some("ready for review".to_string())
        })
    );
    assert_eq!(
        SlashCommand::parse("/issue flaky test"),
        Some(SlashCommand::Issue {
            context: Some("flaky test".to_string())
        })
    );
    assert_eq!(
        SlashCommand::parse("/ultraplan ship both features"),
        Some(SlashCommand::Ultraplan {
            task: Some("ship both features".to_string())
        })
    );
    assert_eq!(
        SlashCommand::parse("/teleport conversation.rs"),
        Some(SlashCommand::Teleport {
            target: Some("conversation.rs".to_string())
        })
    );
    assert_eq!(
        SlashCommand::parse("/debug-tool-call"),
        Some(SlashCommand::DebugToolCall)
    );
    assert_eq!(
        SlashCommand::parse("/model opus"),
        Some(SlashCommand::Model {
            model: Some("opus".to_string()),
        })
    );
    assert_eq!(
        SlashCommand::parse("/model"),
        Some(SlashCommand::Model { model: None })
    );
    assert_eq!(
        SlashCommand::parse("/permissions read-only"),
        Some(SlashCommand::Permissions {
            mode: Some("read-only".to_string()),
        })
    );
    assert_eq!(
        SlashCommand::parse("/clear"),
        Some(SlashCommand::Clear { confirm: false })
    );
    assert_eq!(
        SlashCommand::parse("/clear --confirm"),
        Some(SlashCommand::Clear { confirm: true })
    );
    assert_eq!(SlashCommand::parse("/cost"), Some(SlashCommand::Cost));
    assert_eq!(
        SlashCommand::parse("/resume session.json"),
        Some(SlashCommand::Resume {
            session_path: Some("session.json".to_string()),
        })
    );
    assert_eq!(
        SlashCommand::parse("/config"),
        Some(SlashCommand::Config { section: None })
    );
    assert_eq!(
        SlashCommand::parse("/config env"),
        Some(SlashCommand::Config {
            section: Some("env".to_string())
        })
    );
    assert_eq!(SlashCommand::parse("/memory"), Some(SlashCommand::Memory));
    assert_eq!(SlashCommand::parse("/init"), Some(SlashCommand::Init));
    assert_eq!(SlashCommand::parse("/diff"), Some(SlashCommand::Diff));
    assert_eq!(SlashCommand::parse("/version"), Some(SlashCommand::Version));
    assert_eq!(
        SlashCommand::parse("/export notes.txt"),
        Some(SlashCommand::Export {
            path: Some("notes.txt".to_string())
        })
    );
    assert_eq!(
        SlashCommand::parse("/session switch abc123"),
        Some(SlashCommand::Session {
            action: Some("switch".to_string()),
            target: Some("abc123".to_string())
        })
    );
    assert_eq!(
        SlashCommand::parse("/plugins install demo"),
        Some(SlashCommand::Plugins {
            action: Some("install".to_string()),
            target: Some("demo".to_string())
        })
    );
    assert_eq!(
        SlashCommand::parse("/plugins list"),
        Some(SlashCommand::Plugins {
            action: Some("list".to_string()),
            target: None
        })
    );
    assert_eq!(
        SlashCommand::parse("/plugins enable demo"),
        Some(SlashCommand::Plugins {
            action: Some("enable".to_string()),
            target: Some("demo".to_string())
        })
    );
    assert_eq!(
        SlashCommand::parse("/plugins disable demo"),
        Some(SlashCommand::Plugins {
            action: Some("disable".to_string()),
            target: Some("demo".to_string())
        })
    );
}

#[test]
fn renders_help_from_shared_specs() {
    let help = render_slash_command_help();
    assert!(help.contains("available via aineer --resume SESSION.json"));
    assert!(help.contains("Core flow"));
    assert!(help.contains("Workspace & memory"));
    assert!(help.contains("Sessions & output"));
    assert!(help.contains("Git & GitHub"));
    assert!(help.contains("Automation & discovery"));
    assert!(help.contains("/help"));
    assert!(help.contains("/status"));
    assert!(help.contains("/compact"));
    assert!(help.contains("/bughunter [scope]"));
    assert!(help.contains("/branch [list|create <name>|switch <name>]"));
    assert!(help.contains("/worktree [list|add <path> [branch]|remove <path>|prune]"));
    assert!(help.contains("/commit"));
    assert!(help.contains("/commit-push-pr [context]"));
    assert!(help.contains("/pr [context]"));
    assert!(help.contains("/issue [context]"));
    assert!(help.contains("/ultraplan [task]"));
    assert!(help.contains("/teleport <symbol-or-path>"));
    assert!(help.contains("/debug-tool-call"));
    assert!(help.contains("/model [model]"));
    assert!(help.contains("/permissions [read-only|workspace-write|danger-full-access]"));
    assert!(help.contains("/clear [--confirm]"));
    assert!(help.contains("/cost"));
    assert!(help.contains("/resume <session-path>"));
    assert!(help.contains("/config [env|hooks|model|plugins]"));
    assert!(help.contains("/memory"));
    assert!(help.contains("/init"));
    assert!(help.contains("/diff"));
    assert!(help.contains("/version"));
    assert!(help.contains("/export [file]"));
    assert!(help.contains("/session [list|switch <session-id>]"));
    assert!(help.contains(
        "/plugin [list|install <path>|enable <name>|disable <name>|uninstall <id>|update <id>]"
    ));
    assert!(help.contains("aliases: /plugins, /marketplace"));
    assert!(help.contains("/models [provider]"));
    assert!(help.contains("/providers"));
    assert!(help.contains("/agents"));
    assert!(help.contains("/skills"));
    assert!(help.contains("/doctor"));
    assert!(help.contains("/update"));
    assert_eq!(slash_command_specs().len(), 32);
    assert_eq!(resume_supported_slash_commands().len(), 17);
}

#[test]
fn suggests_close_slash_commands() {
    let suggestions = suggest_slash_commands("stats", 3);
    assert!(!suggestions.is_empty());
    assert_eq!(suggestions[0], "/status");
}

#[test]
fn compacts_sessions_via_slash_command() {
    let session = Session {
        messages: vec![
            ConversationMessage::user_text("a ".repeat(200)),
            ConversationMessage::assistant(vec![ContentBlock::Text {
                text: "b ".repeat(200),
            }]),
            ConversationMessage::tool_result("1", "bash", "ok ".repeat(200), false),
            ConversationMessage::assistant(vec![ContentBlock::Text {
                text: "recent".to_string(),
            }]),
        ],
        ..Session::new()
    };

    let result = handle_slash_command_simple(
        "/compact",
        &session,
        CompactionConfig {
            preserve_recent_messages: 2,
            max_estimated_tokens: 1,
        },
    )
    .expect("slash command should be handled");

    assert!(result.message.contains("Compacted 2 messages"));
    assert_eq!(result.session.messages[0].role, MessageRole::System);
}

#[test]
fn help_command_is_non_mutating() {
    let session = Session::new();
    let result = handle_slash_command_simple("/help", &session, CompactionConfig::default())
        .expect("help command should be handled");
    assert_eq!(result.session, session);
    assert!(result.message.contains("Slash commands"));
}

#[test]
fn ignores_unknown_or_runtime_bound_slash_commands() {
    let session = Session::new();
    assert!(
        handle_slash_command_simple("/unknown", &session, CompactionConfig::default()).is_none()
    );
    assert!(
        handle_slash_command_simple("/status", &session, CompactionConfig::default()).is_none()
    );
    assert!(
        handle_slash_command_simple("/branch list", &session, CompactionConfig::default())
            .is_none()
    );
    assert!(
        handle_slash_command_simple("/bughunter", &session, CompactionConfig::default()).is_none()
    );
    assert!(
        handle_slash_command_simple("/worktree list", &session, CompactionConfig::default())
            .is_none()
    );
    assert!(
        handle_slash_command_simple("/commit", &session, CompactionConfig::default()).is_none()
    );
    assert!(handle_slash_command_simple(
        "/commit-push-pr review notes",
        &session,
        CompactionConfig::default()
    )
    .is_none());
    assert!(handle_slash_command_simple("/pr", &session, CompactionConfig::default()).is_none());
    assert!(handle_slash_command_simple("/issue", &session, CompactionConfig::default()).is_none());
    assert!(
        handle_slash_command_simple("/ultraplan", &session, CompactionConfig::default()).is_none()
    );
    assert!(
        handle_slash_command_simple("/teleport foo", &session, CompactionConfig::default())
            .is_none()
    );
    assert!(
        handle_slash_command_simple("/debug-tool-call", &session, CompactionConfig::default())
            .is_none()
    );
    assert!(
        handle_slash_command_simple("/model sonnet", &session, CompactionConfig::default())
            .is_none()
    );
    assert!(handle_slash_command_simple(
        "/permissions read-only",
        &session,
        CompactionConfig::default()
    )
    .is_none());
    assert!(handle_slash_command_simple("/clear", &session, CompactionConfig::default()).is_none());
    assert!(
        handle_slash_command_simple("/clear --confirm", &session, CompactionConfig::default())
            .is_none()
    );
    assert!(handle_slash_command_simple("/cost", &session, CompactionConfig::default()).is_none());
    assert!(handle_slash_command_simple(
        "/resume session.json",
        &session,
        CompactionConfig::default()
    )
    .is_none());
    assert!(
        handle_slash_command_simple("/config", &session, CompactionConfig::default()).is_none()
    );
    assert!(
        handle_slash_command_simple("/config env", &session, CompactionConfig::default()).is_none()
    );
    assert!(handle_slash_command_simple("/diff", &session, CompactionConfig::default()).is_none());
    assert!(
        handle_slash_command_simple("/version", &session, CompactionConfig::default()).is_none()
    );
    assert!(
        handle_slash_command_simple("/export note.txt", &session, CompactionConfig::default())
            .is_none()
    );
    assert!(
        handle_slash_command_simple("/session list", &session, CompactionConfig::default())
            .is_none()
    );
    assert!(
        handle_slash_command_simple("/plugins list", &session, CompactionConfig::default())
            .is_none()
    );
}

#[test]
fn renders_plugins_report_with_name_version_and_status() {
    let rendered = render_plugins_report(&[
        PluginSummary {
            metadata: PluginMetadata {
                id: "demo@external".to_string(),
                name: "demo".to_string(),
                version: "1.2.3".to_string(),
                description: "demo plugin".to_string(),
                kind: PluginKind::External,
                source: "demo".to_string(),
                default_enabled: false,
                root: None,
            },
            enabled: true,
        },
        PluginSummary {
            metadata: PluginMetadata {
                id: "sample@external".to_string(),
                name: "sample".to_string(),
                version: "0.9.0".to_string(),
                description: "sample plugin".to_string(),
                kind: PluginKind::External,
                source: "sample".to_string(),
                default_enabled: false,
                root: None,
            },
            enabled: false,
        },
    ]);

    assert!(rendered.contains("demo"));
    assert!(rendered.contains("v1.2.3"));
    assert!(rendered.contains("enabled"));
    assert!(rendered.contains("sample"));
    assert!(rendered.contains("v0.9.0"));
    assert!(rendered.contains("disabled"));
}

#[test]
fn lists_agents_from_project_and_user_roots() {
    let workspace = temp_dir("agents-workspace");
    let project_agents = workspace.join(".aineer").join("agents");
    let user_home = temp_dir("agents-home");
    let user_agents = user_home.join(".aineer").join("agents");

    write_agent(
        &project_agents,
        "planner",
        "Project planner",
        "gpt-5.4",
        "medium",
    );
    write_agent(
        &user_agents,
        "planner",
        "User planner",
        "gpt-5.4-mini",
        "high",
    );
    write_agent(
        &user_agents,
        "verifier",
        "Verification agent",
        "gpt-5.4-mini",
        "high",
    );

    let roots = vec![
        (DefinitionSource::Project, project_agents),
        (DefinitionSource::User, user_agents),
    ];
    let report =
        render_agents_report(&load_agents_from_roots(&roots).expect("agent roots should load"));

    assert!(report.contains("Agents"));
    assert!(report.contains("2 active agents"));
    assert!(report.contains("Project (.aineer):"));
    assert!(report.contains("planner · Project planner · gpt-5.4 · medium"));
    assert!(report.contains("User (~/.aineer):"));
    assert!(report.contains("(shadowed by Project (.aineer)) planner · User planner"));
    assert!(report.contains("verifier · Verification agent · gpt-5.4-mini · high"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_dir_all(user_home);
}

#[test]
fn lists_skills_from_project_and_user_roots() {
    let workspace = temp_dir("skills-workspace");
    let project_skills = workspace.join(".aineer").join("skills");
    let user_home = temp_dir("skills-home");
    let user_skills = user_home.join(".aineer").join("skills");

    write_skill(&project_skills, "plan", "Project planning guidance");
    write_skill(&user_skills, "plan", "User planning guidance");
    write_skill(&user_skills, "help", "Help guidance");

    let roots = vec![
        SkillRoot {
            source: DefinitionSource::Project,
            path: project_skills,
        },
        SkillRoot {
            source: DefinitionSource::User,
            path: user_skills,
        },
    ];
    let report =
        render_skills_report(&load_skills_from_roots(&roots).expect("skill roots should load"));

    assert!(report.contains("Skills"));
    assert!(report.contains("2 available skills"));
    assert!(report.contains("Project (.aineer):"));
    assert!(report.contains("plan · Project planning guidance"));
    assert!(report.contains("User (~/.aineer):"));
    assert!(report.contains("(shadowed by Project (.aineer)) plan · User planning guidance"));
    assert!(report.contains("help · Help guidance"));

    let _ = fs::remove_dir_all(workspace);
    let _ = fs::remove_dir_all(user_home);
}

#[test]
fn agents_and_skills_usage_support_help_and_unexpected_args() {
    let cwd = temp_dir("slash-usage");

    let agents_help = super::handle_agents_slash_command(Some("help"), &cwd).expect("agents help");
    assert!(agents_help.contains("Usage            /agents"));
    assert!(agents_help.contains("Direct CLI       aineer agents"));

    let agents_unexpected =
        super::handle_agents_slash_command(Some("show planner"), &cwd).expect("agents usage");
    assert!(agents_unexpected.contains("Unexpected       show planner"));

    let skills_help =
        super::handle_skills_slash_command(Some("--help"), &cwd).expect("skills help");
    assert!(skills_help.contains("Usage            /skills"));
    assert!(skills_help.contains("~/.aineer/skills"));

    let skills_unexpected =
        super::handle_skills_slash_command(Some("show help"), &cwd).expect("skills usage");
    assert!(skills_unexpected.contains("Unexpected       show help"));

    let _ = fs::remove_dir_all(cwd);
}

#[test]
fn parses_quoted_skill_frontmatter_values() {
    let contents = "---\nname: \"hud\"\ndescription: 'Quoted description'\n---\n";
    let (name, description) = parse_skill_frontmatter(contents);
    assert_eq!(name.as_deref(), Some("hud"));
    assert_eq!(description.as_deref(), Some("Quoted description"));
}

#[test]
fn installs_plugin_from_path_and_lists_it() {
    let config_home = temp_dir("home");
    let source_root = temp_dir("source");
    write_external_plugin(&source_root, "demo", "1.0.0");

    let mut manager = PluginManager::new(PluginManagerConfig::new(&config_home));
    let install = handle_plugins_slash_command(
        Some("install"),
        Some(source_root.to_str().expect("utf8 path")),
        &mut manager,
    )
    .expect("install command should succeed");
    assert_eq!(install.effect, PluginEffect::ReloadRuntime);
    assert!(install.message.contains("installed demo@external"));
    assert!(install.message.contains("Name             demo"));
    assert!(install.message.contains("Version          1.0.0"));
    assert!(install.message.contains("Status           enabled"));

    let list = handle_plugins_slash_command(Some("list"), None, &mut manager)
        .expect("list command should succeed");
    assert_eq!(list.effect, PluginEffect::None);
    assert!(list.message.contains("demo"));
    assert!(list.message.contains("v1.0.0"));
    assert!(list.message.contains("enabled"));

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(source_root);
}

#[test]
fn enables_and_disables_plugin_by_name() {
    let config_home = temp_dir("toggle-home");
    let source_root = temp_dir("toggle-source");
    write_external_plugin(&source_root, "demo", "1.0.0");

    let mut manager = PluginManager::new(PluginManagerConfig::new(&config_home));
    handle_plugins_slash_command(
        Some("install"),
        Some(source_root.to_str().expect("utf8 path")),
        &mut manager,
    )
    .expect("install command should succeed");

    let disable = handle_plugins_slash_command(Some("disable"), Some("demo"), &mut manager)
        .expect("disable command should succeed");
    assert_eq!(disable.effect, PluginEffect::ReloadRuntime);
    assert!(disable.message.contains("disabled demo@external"));
    assert!(disable.message.contains("Name             demo"));
    assert!(disable.message.contains("Status           disabled"));

    let list = handle_plugins_slash_command(Some("list"), None, &mut manager)
        .expect("list command should succeed");
    assert!(list.message.contains("demo"));
    assert!(list.message.contains("disabled"));

    let enable = handle_plugins_slash_command(Some("enable"), Some("demo"), &mut manager)
        .expect("enable command should succeed");
    assert_eq!(enable.effect, PluginEffect::ReloadRuntime);
    assert!(enable.message.contains("enabled demo@external"));
    assert!(enable.message.contains("Name             demo"));
    assert!(enable.message.contains("Status           enabled"));

    let list = handle_plugins_slash_command(Some("list"), None, &mut manager)
        .expect("list command should succeed");
    assert!(list.message.contains("demo"));
    assert!(list.message.contains("enabled"));

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(source_root);
}

#[test]
fn lists_auto_installed_bundled_plugins_with_status() {
    let config_home = temp_dir("bundled-home");
    let bundled_root = temp_dir("bundled-root");
    let bundled_plugin = bundled_root.join("starter");
    write_bundled_plugin(&bundled_plugin, "starter", "0.1.0", false);

    let mut config = PluginManagerConfig::new(&config_home);
    config.bundled_root = Some(bundled_root.clone());
    let mut manager = PluginManager::new(config);

    let list = handle_plugins_slash_command(Some("list"), None, &mut manager)
        .expect("list command should succeed");
    assert_eq!(list.effect, PluginEffect::None);
    assert!(list.message.contains("starter"));
    assert!(list.message.contains("v0.1.0"));
    assert!(list.message.contains("disabled"));

    let _ = fs::remove_dir_all(config_home);
    let _ = fs::remove_dir_all(bundled_root);
}

#[test]
fn branch_and_worktree_commands_manage_git_state() {
    // given
    let repo = init_git_repo("branch-worktree");
    let worktree_path = repo
        .parent()
        .expect("repo should have parent")
        .join("branch-worktree-linked");

    // when
    let branch_list =
        handle_branch_slash_command(Some("list"), None, &repo).expect("branch list succeeds");
    let created = handle_branch_slash_command(Some("create"), Some("feature/demo"), &repo)
        .expect("branch create succeeds");
    let switched = handle_branch_slash_command(Some("switch"), Some("main"), &repo)
        .expect("branch switch succeeds");
    let added = handle_worktree_slash_command(
        Some("add"),
        Some(worktree_path.to_str().expect("utf8 path")),
        Some("wt-demo"),
        &repo,
    )
    .expect("worktree add succeeds");
    let listed_worktrees =
        handle_worktree_slash_command(Some("list"), None, None, &repo).expect("list succeeds");
    let removed = handle_worktree_slash_command(
        Some("remove"),
        Some(worktree_path.to_str().expect("utf8 path")),
        None,
        &repo,
    )
    .expect("remove succeeds");

    // then
    assert!(branch_list.contains("main"));
    assert!(created.contains("feature/demo"));
    assert!(switched.contains("main"));
    assert!(added.contains("wt-demo"));
    let wt_path_str = worktree_path.to_str().expect("utf8 path");
    let wt_path_forward = wt_path_str.replace('\\', "/");
    let wt_name = worktree_path
        .file_name()
        .expect("worktree path has a file name")
        .to_str()
        .expect("utf8 file name");
    assert!(
        listed_worktrees.contains(wt_path_str)
            || listed_worktrees.contains(&*wt_path_forward)
            || listed_worktrees.contains(wt_name),
        "listed worktrees did not contain worktree path\npath: {wt_path_str}\nname: {wt_name}\noutput: {listed_worktrees}"
    );
    assert!(removed.contains("Result           removed"));

    let _ = fs::remove_dir_all(repo);
    let _ = fs::remove_dir_all(worktree_path);
}

#[test]
fn commit_command_stages_and_commits_changes() {
    // given
    let repo = init_git_repo("commit-command");
    fs::write(repo.join("notes.txt"), "hello\n").expect("write notes");

    // when
    let report = handle_commit_slash_command("feat: add notes", &repo).expect("commit succeeds");
    let status = run_command(&repo, "git", &["status", "--short"]);
    let message = run_command(&repo, "git", &["log", "-1", "--pretty=%B"]);

    // then
    assert!(report.contains("Result           created"));
    assert!(status.trim().is_empty());
    assert_eq!(message.trim(), "feat: add notes");

    let _ = fs::remove_dir_all(repo);
}

#[cfg(unix)]
#[test]
fn commit_push_pr_command_commits_pushes_and_creates_pr() {
    // given
    let _guard = env_lock();
    let repo = init_git_repo("commit-push-pr");
    let remote = init_bare_repo("commit-push-pr-remote");
    run_command(
        &repo,
        "git",
        &[
            "remote",
            "add",
            "origin",
            remote.to_str().expect("utf8 remote"),
        ],
    );
    run_command(&repo, "git", &["push", "-u", "origin", "main"]);
    fs::write(repo.join("feature.txt"), "feature\n").expect("write feature file");

    let fake_bin = temp_dir("fake-gh-bin");
    let gh_log = fake_bin.join("gh.log");
    write_fake_gh(&fake_bin, &gh_log, "https://example.com/pr/123");

    let previous_path = env::var_os("PATH");
    let mut new_path = fake_bin.display().to_string();
    if let Some(path) = &previous_path {
        new_path.push(':');
        new_path.push_str(&path.to_string_lossy());
    }
    env::set_var("PATH", &new_path);
    let previous_safeuser = env::var_os("SAFEUSER");
    env::set_var("SAFEUSER", "tester");

    let request = CommitPushPrRequest {
        commit_message: Some("feat: add feature file".to_string()),
        pr_title: "Add feature file".to_string(),
        pr_body: "## Summary\n- add feature file".to_string(),
        branch_name_hint: "Add feature file".to_string(),
    };

    // when
    let report =
        handle_commit_push_pr_slash_command(&request, &repo).expect("commit-push-pr succeeds");
    let branch = run_command(&repo, "git", &["branch", "--show-current"]);
    let message = run_command(&repo, "git", &["log", "-1", "--pretty=%B"]);
    let gh_invocations = fs::read_to_string(&gh_log).expect("gh log should exist");

    // then
    assert!(report.contains("Result           created"));
    assert!(report.contains("URL              https://example.com/pr/123"));
    assert_eq!(branch.trim(), "tester/add-feature-file");
    assert_eq!(message.trim(), "feat: add feature file");
    assert!(gh_invocations.contains("pr create"));
    assert!(gh_invocations.contains("--base main"));

    if let Some(path) = previous_path {
        env::set_var("PATH", path);
    } else {
        env::remove_var("PATH");
    }
    if let Some(safeuser) = previous_safeuser {
        env::set_var("SAFEUSER", safeuser);
    } else {
        env::remove_var("SAFEUSER");
    }

    let _ = fs::remove_dir_all(repo);
    let _ = fs::remove_dir_all(remote);
    let _ = fs::remove_dir_all(fake_bin);
}
