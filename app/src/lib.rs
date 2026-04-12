pub mod blocks;
mod commands;
pub mod error;
pub mod pty_manager;
pub mod session;
mod system_tray;
pub mod version;

use crate::error::{AppError, AppResult};
use aineer_release_channel::ReleaseChannel;
use commands::{
    agent, ai, auto_update, cache, channels, files, gateway, git, lsp, mcp, memory, plugins,
    session as session_cmd, settings, shell, slash_commands,
};
use serde::Serialize;
#[cfg(target_os = "macos")]
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::Manager;

fn read_close_to_tray(app: &tauri::AppHandle) -> bool {
    let state = app.state::<settings::ManagedSettings>();
    state
        .merged()
        .ok()
        .and_then(|s| s.close_to_tray)
        .unwrap_or(true)
}

#[tauri::command]
fn get_close_to_tray(app: tauri::AppHandle) -> bool {
    read_close_to_tray(&app)
}

#[tauri::command]
fn set_close_to_tray(
    state: tauri::State<'_, settings::ManagedSettings>,
    enabled: bool,
) -> AppResult<()> {
    let updates = serde_json::json!({ "closeToTray": enabled });
    state.save_and_reload(&updates).map_err(AppError::Settings)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    version_suffix: &'static str,
    channel: &'static str,
    display_name: &'static str,
    github_url: &'static str,
    homepage: &'static str,
}

#[tauri::command]
fn get_app_info() -> AppInfo {
    let ch = ReleaseChannel::current();
    AppInfo {
        name: "Aineer",
        version: env!("CARGO_PKG_VERSION"),
        version_suffix: ch.version_suffix(),
        channel: match ch {
            ReleaseChannel::Dev => "dev",
            ReleaseChannel::Nightly => "nightly",
            ReleaseChannel::Preview => "preview",
            ReleaseChannel::Stable => "stable",
        },
        display_name: ch.display_name(),
        github_url: env!("CARGO_PKG_REPOSITORY"),
        homepage: env!("CARGO_PKG_HOMEPAGE"),
    }
}

pub fn run_desktop() {
    init_logging();

    let channel = ReleaseChannel::current();
    tracing::info!(
        "Starting {} v{} ({})",
        channel.display_name(),
        env!("CARGO_PKG_VERSION"),
        channel,
    );

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(pty_manager::PtyManager::new())
        .manage(settings::ManagedSettings::load())
        .manage(gateway::ManagedGateway::new())
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if read_close_to_tray(window.app_handle()) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(move |app| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_title(&version::display_title());
            }

            #[cfg(target_os = "macos")]
            {
                let display = channel.display_name();
                let about_item = MenuItemBuilder::with_id("menu-about", format!("About {display}"))
                    .build(app)?;
                let app_submenu = SubmenuBuilder::new(app, display)
                    .item(&about_item)
                    .separator()
                    .services()
                    .separator()
                    .hide()
                    .hide_others()
                    .show_all()
                    .separator()
                    .quit()
                    .build()?;
                let edit_submenu = SubmenuBuilder::new(app, "Edit")
                    .undo()
                    .redo()
                    .separator()
                    .cut()
                    .copy()
                    .paste()
                    .select_all()
                    .build()?;
                let menu = MenuBuilder::new(app)
                    .item(&app_submenu)
                    .item(&edit_submenu)
                    .build()?;
                app.set_menu(menu)?;
                app.on_menu_event(move |app, event| {
                    if event.id().as_ref() == "menu-about" {
                        system_tray::show_about_window(app);
                    }
                });
            }

            if let Err(e) = system_tray::setup(app) {
                tracing::warn!("Failed to setup system tray: {e}");
            }

            // Run scheduled auto-cleanup if due
            cache::maybe_run_auto_cleanup(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Shell / PTY
            shell::spawn_pty,
            shell::write_pty,
            shell::resize_pty,
            shell::kill_pty,
            shell::execute_command,
            shell::shell_complete,
            // AI
            ai::send_ai_message,
            ai::stop_ai_stream,
            // Agent
            agent::start_agent,
            agent::approve_tool,
            agent::deny_tool,
            agent::stop_agent,
            // Settings
            settings::get_settings,
            settings::update_settings,
            settings::get_api_key,
            settings::set_api_key,
            settings::list_model_groups,
            // Files
            files::get_project_root,
            files::list_dir,
            files::read_file,
            files::search_files,
            // Git
            git::git_status,
            git::git_branch,
            git::git_diff,
            git::git_list_branches,
            git::git_checkout,
            // Memory
            memory::search_memory,
            memory::remember,
            memory::forget,
            // Session
            session_cmd::save_session,
            session_cmd::load_session,
            session_cmd::list_sessions,
            // Slash commands
            slash_commands::get_slash_commands,
            slash_commands::execute_slash_command,
            // Cache
            cache::get_cache_stats,
            cache::save_attachment,
            cache::clear_cache,
            cache::list_chat_history,
            cache::delete_chat_history,
            cache::get_auto_cleanup,
            cache::set_auto_cleanup,
            // Auto-update
            auto_update::check_for_update,
            auto_update::get_update_channel,
            // Channels
            channels::list_channel_adapters,
            // MCP
            mcp::list_mcp_servers,
            mcp::start_mcp_server,
            mcp::stop_mcp_server,
            mcp::call_mcp_tool,
            // LSP
            lsp::lsp_diagnostics,
            lsp::lsp_hover,
            lsp::lsp_completions,
            // Plugins
            plugins::list_plugins,
            plugins::install_plugin,
            plugins::uninstall_plugin,
            // Gateway
            gateway::start_gateway,
            gateway::stop_gateway,
            gateway::get_gateway_status,
            // App-level
            get_close_to_tray,
            set_close_to_tray,
            get_app_info,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Aineer");

    #[cfg(target_os = "macos")]
    app.run(|app, event| {
        if let tauri::RunEvent::Reopen { .. } = &event {
            system_tray::show_main_window(app);
        }
    });

    #[cfg(not(target_os = "macos"))]
    app.run(|_app, _event| {});
}

fn init_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aineer=info".into()),
        )
        .init();
}
