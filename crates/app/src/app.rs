use std::path::PathBuf;
use std::sync::{mpsc, Arc};

use eframe::egui;
use egui::{FontId, Key, RichText, Vec2};

use terminal::{BackendCommand, FontSettings, PtyEvent, TerminalFont, TerminalView};
use ui::cards::{Card, ChatCard, ShellCard, SystemCard};
use ui::diff_panel::DiffPanel;
use ui::git_watcher::GitWatcher;
use ui::input_bar::{CardPickerItem, InputBar, SlashMenuItem, SubmitAction};
use ui::settings::SettingsPanel;
use ui::timeline::{Timeline, TimelineAction};
use ui::widgets::{
    ActivityBar, ActivityItem, CommandPalette, ExplorerAction, ExplorerPanel, PaletteItem,
    StatusBar, StatusSegment, ToastManager, ACTIVITY_BAR_WIDTH,
};

use crate::agent::{AgentEvent, AgentHandle, ToolApproval};
use crate::ssh::{SshManager, SshProfile};
use crate::tabs::TabManager;
use crate::theme;

struct ActiveStream {
    tab_id: u64,
    card_id: u64,
    event_rx: mpsc::Receiver<AgentEvent>,
    approval_tx: tokio::sync::mpsc::Sender<ToolApproval>,
}

pub struct TabState {
    pub timeline: Timeline,
    pub input_bar: InputBar,
    pub fullscreen_overlay: bool,
}

impl TabState {
    fn new() -> Self {
        let mut timeline = Timeline::new();
        let welcome_id = timeline.next_card_id();
        timeline.push_card(Card::System(SystemCard::new(
            welcome_id,
            format!(
                "Welcome to {} — {}\nPress Enter to run shell commands, Ctrl+Enter to chat with AI.",
                crate::branding::APP_NAME,
                crate::branding::APP_TAGLINE
            ),
        )));

        Self {
            timeline,
            input_bar: InputBar::new(),
            fullscreen_overlay: false,
        }
    }

    fn from_tab_session(tab_session: &crate::session::TabSession) -> Self {
        let mut timeline = Timeline::new();
        for card_data in &tab_session.cards {
            let card = card_data.to_card();
            timeline.push_card(card);
        }
        if timeline.cards.is_empty() {
            return Self::new();
        }
        let sys_id = timeline.next_card_id();
        timeline.push_card(Card::System(SystemCard::new(
            sys_id,
            "Session restored.".to_string(),
        )));
        Self {
            timeline,
            input_bar: InputBar::new(),
            fullscreen_overlay: false,
        }
    }
}

pub struct AineerApp {
    tab_manager: TabManager,
    tab_states: Vec<(u64, TabState)>,
    pty_sender: mpsc::Sender<(u64, PtyEvent)>,
    pty_receiver: mpsc::Receiver<(u64, PtyEvent)>,
    terminal_theme: terminal::TerminalTheme,
    font_size: f32,
    diff_panel: DiffPanel,
    git_watcher: Option<GitWatcher>,
    git_status: Option<Arc<ui::git_diff::GitStatus>>,
    gateway_status: tokio::sync::watch::Receiver<gateway::GatewayStatus>,
    settings_panel: SettingsPanel,
    agent: AgentHandle,
    active_streams: Vec<ActiveStream>,
    _tokio_rt: Arc<tokio::runtime::Runtime>,
    slash_items: Vec<SlashMenuItem>,
    ssh_manager: SshManager,
    show_ssh_dialog: bool,
    ssh_draft: SshProfile,
    update_status: Arc<std::sync::Mutex<crate::updater::UpdateStatus>>,
    command_palette: CommandPalette,
    activity_bar: ActivityBar,
    explorer: ExplorerPanel,
    toasts: ToastManager,
    /// Per-tab PTY write generation at last card snapshot, for live output refresh.
    last_snapshot_gen: Vec<(u64, u64)>,
}

impl AineerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let saved_settings = ui::settings::load_settings();
        let theme_mode = ui::theme::ThemeMode::parse(
            &saved_settings
                .as_ref()
                .map(|s| s.theme.clone())
                .unwrap_or_else(|| "dark".to_string()),
        );
        theme::setup(&cc.egui_ctx, theme_mode);

        let (pty_sender, pty_receiver) = mpsc::channel();

        let mut tab_manager = TabManager::new();
        let session = crate::session::load_session();

        let tab_states: Vec<(u64, TabState)> = if let Some(ref sess) = session {
            let mut states = Vec::new();
            for tab_session in &sess.tabs {
                tab_manager.create_tab(cc.egui_ctx.clone(), pty_sender.clone());
                let tab_id = tab_manager.active_tab_id().unwrap();
                if !tab_session.title.is_empty() {
                    tab_manager.set_title(tab_id, tab_session.title.clone());
                }
                states.push((tab_id, TabState::from_tab_session(tab_session)));
            }
            if states.is_empty() {
                tab_manager.create_tab(cc.egui_ctx.clone(), pty_sender.clone());
                let tab_id = tab_manager.active_tab_id().unwrap();
                states.push((tab_id, TabState::new()));
            } else if let Some(&(active_id, _)) = states.get(sess.active_tab_index) {
                tab_manager.set_active(active_id);
            }
            states
        } else {
            tab_manager.create_tab(cc.egui_ctx.clone(), pty_sender.clone());
            let tab_id = tab_manager.active_tab_id().unwrap();
            vec![(tab_id, TabState::new())]
        };

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let git_watcher = Some(GitWatcher::start(cwd));

        // Start tokio runtime for async tasks (Gateway, Agent, etc.)
        let rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime"),
        );

        // Start Gateway in background
        let gateway_config = gateway::GatewayConfig::default();
        let gateway = gateway::GatewayServer::new(gateway_config);
        let gateway_status = gateway.status_rx();
        rt.spawn(async move {
            if let Err(e) = gateway.start().await {
                tracing::error!("Gateway error: {e}");
            }
        });

        let agent = AgentHandle::spawn(&rt);

        let slash_items = engine::commands::slash_command_specs()
            .iter()
            .map(|spec| SlashMenuItem {
                name: spec.name.to_string(),
                summary: spec.summary.to_string(),
            })
            .collect();

        Self {
            tab_manager,
            tab_states,
            pty_sender,
            pty_receiver,
            terminal_theme: theme::aineer_terminal_theme(),
            font_size: 14.0,
            diff_panel: DiffPanel::new(),
            git_watcher,
            git_status: None,
            gateway_status,
            settings_panel: SettingsPanel::new(),
            agent,
            active_streams: Vec::new(),
            _tokio_rt: rt,
            slash_items,
            ssh_manager: SshManager::new(),
            show_ssh_dialog: false,
            ssh_draft: SshProfile::default(),
            update_status: {
                let status = Arc::new(std::sync::Mutex::new(
                    crate::updater::UpdateStatus::Checking,
                ));
                let s = status.clone();
                std::thread::spawn(move || {
                    let result = crate::updater::check_for_update();
                    if let Ok(mut lock) = s.lock() {
                        *lock = result;
                    }
                });
                status
            },
            command_palette: {
                let mut cp = CommandPalette::new();
                cp.set_items(vec![
                    PaletteItem {
                        id: "toggle_settings".into(),
                        label: "Toggle Settings".into(),
                        category: "View".into(),
                        shortcut: Some("⌘ ,".into()),
                    },
                    PaletteItem {
                        id: "toggle_diff".into(),
                        label: "Toggle Diff Panel".into(),
                        category: "View".into(),
                        shortcut: None,
                    },
                    PaletteItem {
                        id: "new_tab".into(),
                        label: "New Tab".into(),
                        category: "Tab".into(),
                        shortcut: Some("⌘ T".into()),
                    },
                    PaletteItem {
                        id: "close_tab".into(),
                        label: "Close Tab".into(),
                        category: "Tab".into(),
                        shortcut: Some("⌘ W".into()),
                    },
                    PaletteItem {
                        id: "toggle_theme".into(),
                        label: "Toggle Dark/Light Theme".into(),
                        category: "Appearance".into(),
                        shortcut: None,
                    },
                    PaletteItem {
                        id: "ssh_connect".into(),
                        label: "SSH Remote Connection".into(),
                        category: "Connection".into(),
                        shortcut: None,
                    },
                ]);
                cp
            },
            activity_bar: ActivityBar::new(),
            explorer: ExplorerPanel::new(),
            toasts: ToastManager::default(),
            last_snapshot_gen: Vec::new(),
        }
    }

    fn poll_git_status(&mut self) {
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            if let Some(cwd) = tab.backend.current_cwd() {
                if let Some(watcher) = &mut self.git_watcher {
                    if watcher.watched_path() != cwd {
                        watcher.switch_directory(cwd.to_path_buf());
                    }
                }
            }
        }
        if let Some(watcher) = &self.git_watcher {
            if let Some(status) = watcher.try_recv() {
                self.diff_panel.update_status(status.clone());
                self.git_status = Some(status);
            }
        }
    }

    fn tab_state_mut(&mut self, tab_id: u64) -> Option<&mut TabState> {
        self.tab_states
            .iter_mut()
            .find(|(id, _)| *id == tab_id)
            .map(|(_, state)| state)
    }

    fn ensure_tab_state(&mut self, tab_id: u64) {
        if !self.tab_states.iter().any(|(id, _)| *id == tab_id) {
            self.tab_states.push((tab_id, TabState::new()));
        }
    }

    fn process_pty_events(&mut self, ctx: &egui::Context) {
        while let Ok((tab_id, event)) = self.pty_receiver.try_recv() {
            match event {
                PtyEvent::Exit => {
                    self.tab_manager.remove_tab(tab_id);
                    self.tab_states.retain(|(id, _)| *id != tab_id);
                    if self.tab_manager.is_empty() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                PtyEvent::Title(title) => {
                    self.tab_manager.set_title(tab_id, title);
                }
                PtyEvent::ChildExit(exit_code) => {
                    // Snapshot styled output on child exit
                    if let Some(tab) = self.tab_manager.tab_mut(tab_id) {
                        let output = tab.backend.visible_text();
                        let styled: Vec<ui::cards::OutputLine> = tab
                            .backend
                            .visible_styled_lines(&self.terminal_theme)
                            .into_iter()
                            .map(|sl| ui::cards::OutputLine {
                                segments: sl
                                    .segments
                                    .into_iter()
                                    .map(|s| ui::cards::OutputSegment {
                                        text: s.text,
                                        fg: s.fg,
                                        bold: s.bold,
                                    })
                                    .collect(),
                            })
                            .collect();
                        let cwd = tab
                            .backend
                            .current_cwd()
                            .map(|p| p.to_string_lossy().to_string());
                        if let Some(state) = self.tab_state_mut(tab_id) {
                            if let Some(card) = state.timeline.last_shell_card_mut() {
                                if card.running {
                                    card.output_lines = output;
                                    card.styled_output = styled;
                                    if let Some(dir) = cwd {
                                        card.working_dir = dir;
                                    }
                                    card.running = false;
                                    card.exit_code = Some(exit_code);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Refresh running ShellCards with live terminal output when the PTY has
    /// produced new data since our last snapshot.
    fn refresh_running_cards(&mut self) {
        let theme = &self.terminal_theme;

        // Collect tab IDs that have a running card.
        let running_tabs: Vec<u64> = self
            .tab_states
            .iter()
            .filter_map(|(tab_id, state)| {
                state
                    .timeline
                    .last_shell_card()
                    .filter(|c| c.running)
                    .map(|_| *tab_id)
            })
            .collect();

        for tab_id in running_tabs {
            let Some(tab) = self.tab_manager.tab_mut(tab_id) else {
                continue;
            };

            let gen = tab.backend.write_generation();
            let prev = self
                .last_snapshot_gen
                .iter()
                .find(|(id, _)| *id == tab_id)
                .map(|(_, g)| *g)
                .unwrap_or(0);
            if gen == prev {
                continue;
            }

            let output = tab.backend.visible_text();
            let styled: Vec<ui::cards::OutputLine> = tab
                .backend
                .visible_styled_lines(theme)
                .into_iter()
                .map(|sl| ui::cards::OutputLine {
                    segments: sl
                        .segments
                        .into_iter()
                        .map(|s| ui::cards::OutputSegment {
                            text: s.text,
                            fg: s.fg,
                            bold: s.bold,
                        })
                        .collect(),
                })
                .collect();
            let cwd = tab
                .backend
                .current_cwd()
                .map(|p| p.to_string_lossy().to_string());

            if let Some((_, state)) = self.tab_states.iter_mut().find(|(id, _)| *id == tab_id) {
                if let Some(card) = state.timeline.last_shell_card_mut() {
                    card.output_lines = output;
                    card.styled_output = styled;
                    if let Some(dir) = cwd {
                        card.working_dir = dir;
                    }
                }
            }

            if let Some(entry) = self
                .last_snapshot_gen
                .iter_mut()
                .find(|(id, _)| *id == tab_id)
            {
                entry.1 = gen;
            } else {
                self.last_snapshot_gen.push((tab_id, gen));
            }
        }
    }

    fn handle_shell_submit(&mut self, command: String) {
        let Some(tab_id) = self.tab_manager.active_tab_id() else {
            return;
        };

        // Capture output from previous running ShellCard (snapshot terminal text + styled)
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            let output = tab.backend.visible_text();
            let styled = tab
                .backend
                .visible_styled_lines(&self.terminal_theme)
                .into_iter()
                .map(|sl| ui::cards::OutputLine {
                    segments: sl
                        .segments
                        .into_iter()
                        .map(|s| ui::cards::OutputSegment {
                            text: s.text,
                            fg: s.fg,
                            bold: s.bold,
                        })
                        .collect(),
                })
                .collect();
            let cwd = tab
                .backend
                .current_cwd()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "~".to_string());
            if let Some(state) = self.tab_state_mut(tab_id) {
                if let Some(prev_card) = state.timeline.last_shell_card_mut() {
                    if prev_card.running {
                        prev_card.output_lines = output;
                        prev_card.styled_output = styled;
                        prev_card.working_dir = cwd.clone();
                        prev_card.running = false;
                        prev_card.exit_code = None;
                    }
                }
            }
        }

        // Resolve CWD for new card
        let cwd = if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.backend
                .current_cwd()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "~".to_string())
        } else {
            "~".to_string()
        };

        // Create new ShellCard
        if let Some(state) = self.tab_state_mut(tab_id) {
            let card_id = state.timeline.next_card_id();
            state
                .timeline
                .push_card(Card::Shell(ShellCard::new(card_id, command.clone(), cwd)));
        }

        // Write command to PTY
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            let cmd_bytes = format!("{command}\n").into_bytes();
            tab.backend
                .process_command(BackendCommand::Write(cmd_bytes));
        }
    }

    fn handle_chat_submit(&mut self, text: String, refs: Vec<u64>) {
        let Some(tab_id) = self.tab_manager.active_tab_id() else {
            return;
        };

        let card_id;
        if let Some(state) = self.tab_state_mut(tab_id) {
            card_id = state.timeline.next_card_id();
            let mut card = ChatCard::new(card_id, text.clone(), refs.clone());
            card.response = "Thinking...".to_string();
            card.streaming = true;
            state.timeline.push_card(Card::Chat(card));
        } else {
            return;
        }

        // Collect context from referenced cards
        let context: Vec<String> = if let Some(state) = self.tab_state_mut(tab_id) {
            refs.iter()
                .filter_map(|ref_id| state.timeline.card_summary(*ref_id))
                .collect()
        } else {
            Vec::new()
        };

        // Start streaming from agent
        let (event_rx, approval_tx) = self.agent.send_chat(text, context);
        self.active_streams.push(ActiveStream {
            tab_id,
            card_id,
            event_rx,
            approval_tx,
        });
    }

    fn handle_timeline_action(&mut self, tab_id: u64, action: TimelineAction) {
        match action {
            TimelineAction::None => {}
            TimelineAction::ToolApprove {
                card_id,
                tool_use_id: _,
            } => {
                if let Some(stream) = self
                    .active_streams
                    .iter()
                    .find(|s| s.tab_id == tab_id && s.card_id == card_id)
                {
                    let _ = stream.approval_tx.try_send(ToolApproval::Allow);
                }
            }
            TimelineAction::ToolDeny {
                card_id,
                tool_use_id: _,
            } => {
                if let Some(stream) = self
                    .active_streams
                    .iter()
                    .find(|s| s.tab_id == tab_id && s.card_id == card_id)
                {
                    let _ = stream.approval_tx.try_send(ToolApproval::Deny);
                }
            }
            TimelineAction::ToolApproveAll { card_id } => {
                if let Some(stream) = self
                    .active_streams
                    .iter()
                    .find(|s| s.tab_id == tab_id && s.card_id == card_id)
                {
                    let _ = stream.approval_tx.try_send(ToolApproval::AllowAll);
                }
            }
            TimelineAction::AddRef { card_id } => {
                if let Some(state) = self.tab_state_mut(tab_id) {
                    state.input_bar.add_ref(card_id);
                }
            }
        }
    }

    /// Open a new tab connected to a remote host via system SSH.
    fn connect_ssh(&mut self, ctx: &egui::Context, profile: &SshProfile) {
        let ssh_args = profile.to_ssh_args();
        let settings = terminal::BackendSettings {
            shell: "ssh".to_string(),
            args: ssh_args,
            working_directory: None,
        };
        let id = self.tab_manager.create_tab_with_settings(
            ctx.clone(),
            self.pty_sender.clone(),
            settings,
        );
        if let Some(new_id) = id {
            self.ensure_tab_state(new_id);
            self.tab_manager
                .set_title(new_id, format!("{}@{}", profile.user, profile.host));
        }
    }

    fn poll_agent_streams(&mut self) {
        let mut completed = Vec::new();

        for (i, stream) in self.active_streams.iter().enumerate() {
            let mut events = Vec::new();
            while let Ok(event) = stream.event_rx.try_recv() {
                let is_done = matches!(event, AgentEvent::Done);
                events.push(event);
                if is_done {
                    completed.push(i);
                    break;
                }
            }

            if let Some(state) = self
                .tab_states
                .iter_mut()
                .find(|(id, _)| *id == stream.tab_id)
                .map(|(_, s)| s)
            {
                for event in events {
                    match event {
                        AgentEvent::TextDelta(text) => {
                            state.timeline.append_chat_response(stream.card_id, &text);
                        }
                        AgentEvent::Error(msg) => {
                            state.timeline.append_chat_response(
                                stream.card_id,
                                &format!("\n\n**Error:** {msg}"),
                            );
                        }
                        AgentEvent::Done => {
                            state.timeline.finish_chat_streaming(stream.card_id);
                        }
                        AgentEvent::ToolPending {
                            tool_use_id,
                            name,
                            input,
                        } => {
                            state.timeline.add_tool_pending(
                                stream.card_id,
                                tool_use_id,
                                name,
                                input,
                            );
                        }
                        AgentEvent::ToolRunning { tool_use_id } => {
                            state
                                .timeline
                                .set_tool_running(stream.card_id, &tool_use_id);
                        }
                        AgentEvent::ToolResult {
                            tool_use_id,
                            name: _,
                            output,
                            is_error,
                        } => {
                            state.timeline.set_tool_result(
                                stream.card_id,
                                &tool_use_id,
                                output,
                                is_error,
                            );
                        }
                    }
                }
            }
        }

        for i in completed.into_iter().rev() {
            self.active_streams.remove(i);
        }
    }

    fn show_tab_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        use ui::theme as t;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;

            ui.label(
                RichText::new(crate::branding::APP_NAME)
                    .color(t::ACCENT_LIGHT())
                    .size(13.0)
                    .strong(),
            );
            ui.add_space(8.0);

            let tabs = self.tab_manager.tab_ids_and_titles();
            let active_id = self.tab_manager.active_tab_id();
            let mut tab_to_close: Option<u64> = None;
            let mut close_others_of: Option<u64> = None;
            let mut close_right_of: Option<(u64, usize)> = None;
            let mut duplicate_tab: Option<u64> = None;

            // Tab overflow: only show up to N tabs; show "▾ N more" button for the rest
            const MAX_VISIBLE_TABS: usize = 8;
            let overflow_count = tabs.len().saturating_sub(MAX_VISIBLE_TABS);
            let visible_tabs = &tabs[..tabs.len().min(MAX_VISIBLE_TABS)];
            let hidden_tabs = if overflow_count > 0 {
                &tabs[MAX_VISIBLE_TABS..]
            } else {
                &[]
            };

            for (idx, (id, title)) in visible_tabs.iter().enumerate() {
                let is_active = active_id == Some(*id);
                let fill = if is_active {
                    t::TAB_ACTIVE_BG()
                } else {
                    egui::Color32::TRANSPARENT
                };
                let stroke = if is_active {
                    egui::Stroke::new(1.0, t::alpha(t::ACCENT(), 60))
                } else {
                    egui::Stroke::NONE
                };

                // Check if this tab has a running shell command (spinner indicator)
                let has_running = self
                    .tab_states
                    .iter()
                    .find(|(tid, _)| tid == id)
                    .and_then(|(_, state)| {
                        state.timeline.cards.iter().rev().find_map(|c| {
                            if let ui::cards::Card::Shell(sc) = c {
                                Some(sc.running)
                            } else {
                                None
                            }
                        })
                    })
                    .unwrap_or(false);

                let resp = ui
                    .push_id(id, |ui| {
                        egui::Frame::new()
                            .fill(fill)
                            .stroke(stroke)
                            .corner_radius(ui::theme::radius::MD)
                            .inner_margin(egui::Margin::symmetric(8, 3))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    if has_running {
                                        ui.spinner();
                                    }
                                    ui.label(RichText::new(title).size(12.0).color(if is_active {
                                        t::FG()
                                    } else {
                                        t::FG_DIM()
                                    }));
                                    if tabs.len() > 1 {
                                        let close_resp = ui.add(
                                            egui::Button::new(
                                                RichText::new("×").size(11.0).color(t::FG_MUTED()),
                                            )
                                            .fill(egui::Color32::TRANSPARENT)
                                            .frame(false),
                                        );
                                        if close_resp.clicked() {
                                            tab_to_close = Some(*id);
                                        }
                                    }
                                });
                            })
                            .response
                    })
                    .inner;

                if resp.clicked() {
                    self.tab_manager.set_active(*id);
                }
                if resp.middle_clicked() {
                    tab_to_close = Some(*id);
                }

                let tab_id_copy = *id;
                resp.context_menu(|ui| {
                    if ui.button("Close Tab").clicked() {
                        tab_to_close = Some(tab_id_copy);
                        ui.close();
                    }
                    if tabs.len() > 1 && ui.button("Close Other Tabs").clicked() {
                        close_others_of = Some(tab_id_copy);
                        ui.close();
                    }
                    if idx + 1 < tabs.len() && ui.button("Close Tabs to the Right").clicked() {
                        close_right_of = Some((tab_id_copy, idx));
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Duplicate Tab").clicked() {
                        duplicate_tab = Some(tab_id_copy);
                        ui.close();
                    }
                });
            }

            // Tab overflow dropdown: "▾ N more"
            if !hidden_tabs.is_empty() {
                let btn = ui.add(
                    egui::Button::new(
                        RichText::new(format!("▾ {} more", hidden_tabs.len()))
                            .size(11.0)
                            .color(t::FG_DIM()),
                    )
                    .fill(egui::Color32::TRANSPARENT),
                );
                btn.context_menu(|ui| {
                    for (hid, htitle) in hidden_tabs {
                        if ui.button(htitle).clicked() {
                            self.tab_manager.set_active(*hid);
                            ui.close();
                        }
                    }
                });
                if btn.clicked() {
                    // On regular click, also open the popup via context_menu simulation
                    // (egui doesn't support this directly; the context_menu handles it)
                }
            }

            // Handle context menu actions
            if let Some(close_id) = tab_to_close {
                self.tab_manager.remove_tab(close_id);
                self.tab_states.retain(|(tid, _)| *tid != close_id);
                if self.tab_manager.is_empty() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                return;
            }
            if let Some(keep_id) = close_others_of {
                let ids_to_remove: Vec<u64> = tabs
                    .iter()
                    .map(|(id, _)| *id)
                    .filter(|id| *id != keep_id)
                    .collect();
                for id in ids_to_remove {
                    self.tab_manager.remove_tab(id);
                    self.tab_states.retain(|(tid, _)| *tid != id);
                }
                self.tab_manager.set_active(keep_id);
                return;
            }
            if let Some((_, from_idx)) = close_right_of {
                let ids_to_remove: Vec<u64> =
                    tabs.iter().skip(from_idx + 1).map(|(id, _)| *id).collect();
                for id in ids_to_remove {
                    self.tab_manager.remove_tab(id);
                    self.tab_states.retain(|(tid, _)| *tid != id);
                }
                return;
            }
            if duplicate_tab.is_some() {
                self.tab_manager
                    .create_tab(ctx.clone(), self.pty_sender.clone());
                let new_id = self.tab_manager.active_tab_id().unwrap();
                self.ensure_tab_state(new_id);
            }

            if ui
                .add(
                    egui::Button::new(RichText::new("+").size(12.0).color(t::FG_DIM()))
                        .fill(egui::Color32::TRANSPARENT)
                        .corner_radius(ui::theme::radius::SM),
                )
                .on_hover_text("New tab (⌘T)")
                .clicked()
            {
                self.tab_manager
                    .create_tab(ctx.clone(), self.pty_sender.clone());
                let new_id = self.tab_manager.active_tab_id().unwrap();
                self.ensure_tab_state(new_id);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;

                if ui
                    .add(
                        egui::Button::new(RichText::new("⚙").size(14.0).color(t::FG_SOFT()))
                            .fill(egui::Color32::TRANSPARENT),
                    )
                    .on_hover_text("Settings")
                    .clicked()
                {
                    self.settings_panel.toggle();
                }

                if ui
                    .add(
                        egui::Button::new(RichText::new("SSH").size(10.0).color(t::FG_SOFT()))
                            .fill(egui::Color32::TRANSPARENT),
                    )
                    .on_hover_text("SSH Remote Connection")
                    .clicked()
                {
                    self.show_ssh_dialog = true;
                }

                let diff_icon = if self.diff_panel.visible {
                    "◁"
                } else {
                    "▷"
                };
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!("{diff_icon} Diff"))
                                .size(10.0)
                                .color(t::FG_SOFT()),
                        )
                        .fill(egui::Color32::TRANSPARENT),
                    )
                    .on_hover_text("Toggle diff panel")
                    .clicked()
                {
                    self.diff_panel.toggle();
                }
            });
        });
    }

    fn show_status_bar(&self, ui: &mut egui::Ui) {
        use ui::theme as t;

        let mut bar = StatusBar::new();

        // Git branch
        if let Some(ref status) = self.git_status {
            if let Some(ref branch) = status.branch {
                bar = bar.left(
                    StatusSegment::new(format!("⎇ {branch}"), t::ACCENT_LIGHT())
                        .tooltip("Current git branch"),
                );
            }
            let changed = status.diffs.len();
            if changed > 0 {
                bar = bar.left(
                    StatusSegment::new(format!("△ {changed}"), t::WARNING())
                        .tooltip(format!("{changed} file(s) changed")),
                );
            }
        }

        // CWD
        let cwd = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.backend.current_cwd())
            .map(|p| {
                let s = p.to_string_lossy().to_string();
                if let Ok(home) = std::env::var("HOME") {
                    if let Some(rest) = s.strip_prefix(&home) {
                        return format!("~{rest}");
                    }
                }
                s
            })
            .unwrap_or_else(|| "~".into());
        bar = bar.left(StatusSegment::new(cwd, t::FG_DIM()).tooltip("Working directory"));

        // Update status
        if let Ok(status) = self.update_status.lock() {
            match &*status {
                crate::updater::UpdateStatus::Available { tag, url, body } => {
                    let tip = if body.is_empty() {
                        format!("{tag} available")
                    } else {
                        let truncated = if body.len() > 200 {
                            format!("{}…", &body[..200])
                        } else {
                            body.clone()
                        };
                        format!("{tag} available\n\n{truncated}")
                    };
                    bar = bar.right(
                        StatusSegment::new(format!("⬆ {tag}"), t::SUCCESS())
                            .tooltip(tip)
                            .url(url.clone()),
                    );
                }
                crate::updater::UpdateStatus::Checking => {
                    bar = bar.right(
                        StatusSegment::new("⟳", t::FG_MUTED()).tooltip("Checking for updates..."),
                    );
                }
                crate::updater::UpdateStatus::Error(msg) => {
                    bar = bar.right(
                        StatusSegment::new("⚠", t::FG_MUTED())
                            .tooltip(format!("Update check failed: {msg}")),
                    );
                }
                _ => {}
            }
        }

        // Gateway status
        let gw_status = *self.gateway_status.borrow();
        let (gw_text, gw_color, gw_tip) = match gw_status {
            gateway::GatewayStatus::Running => ("● GW", t::SUCCESS(), "Gateway: Running"),
            gateway::GatewayStatus::Starting => ("◐ GW", t::WARNING(), "Gateway: Starting"),
            gateway::GatewayStatus::Error => ("● GW", t::ERROR(), "Gateway: Error"),
            gateway::GatewayStatus::Stopped => ("○ GW", t::FG_MUTED(), "Gateway: Stopped"),
        };
        bar = bar.right(StatusSegment::new(gw_text, gw_color).tooltip(gw_tip));

        bar.show(ui);
    }

    fn handle_palette_action(&mut self, ctx: &egui::Context, action: &str) {
        match action {
            "toggle_settings" => self.settings_panel.toggle(),
            "toggle_diff" => self.diff_panel.toggle(),
            "new_tab" => {
                self.tab_manager
                    .create_tab(ctx.clone(), self.pty_sender.clone());
                let new_id = self.tab_manager.active_tab_id().unwrap();
                self.ensure_tab_state(new_id);
            }
            "close_tab" => {
                if let Some(id) = self.tab_manager.active_tab_id() {
                    self.tab_manager.remove_tab(id);
                    self.tab_states.retain(|(tid, _)| *tid != id);
                }
            }
            "toggle_theme" => {
                let mode = ui::theme::current_mode();
                let new_mode = match mode {
                    ui::theme::ThemeMode::Dark => ui::theme::ThemeMode::Light,
                    ui::theme::ThemeMode::Light => ui::theme::ThemeMode::Dark,
                };
                ui::theme::apply(ctx, new_mode);
                self.settings_panel.draft.theme = new_mode.as_str().to_string();
            }
            "ssh_connect" => self.show_ssh_dialog = true,
            _ => {}
        }
    }

    fn show_ssh_popup(&mut self, ctx: &egui::Context) {
        use ui::theme as t;

        if !self.show_ssh_dialog {
            return;
        }

        let mut open = self.show_ssh_dialog;
        let mut connect_profile: Option<SshProfile> = None;
        let mut remove_idx: Option<usize> = None;

        egui::Window::new("SSH Connection")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(400.0)
            .show(ctx, |ui| {
                if !self.ssh_manager.profiles.is_empty() {
                    ui.label(
                        RichText::new("Saved Profiles")
                            .strong()
                            .size(12.0)
                            .color(t::FG()),
                    );
                    ui.add_space(4.0);

                    for (i, profile) in self.ssh_manager.profiles.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let label = if profile.name.is_empty() {
                                format!("{}@{}:{}", profile.user, profile.host, profile.port)
                            } else {
                                profile.name.clone()
                            };
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(&label).size(12.0).color(t::FG()),
                                    )
                                    .fill(t::SURFACE())
                                    .corner_radius(4.0),
                                )
                                .clicked()
                            {
                                connect_profile = Some(profile.clone());
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("✕").size(11.0).color(t::FG_DIM()),
                                    )
                                    .fill(egui::Color32::TRANSPARENT),
                                )
                                .on_hover_text("Remove profile")
                                .clicked()
                            {
                                remove_idx = Some(i);
                            }
                        });
                    }
                    ui.separator();
                }

                ui.label(
                    RichText::new("New Connection")
                        .strong()
                        .size(12.0)
                        .color(t::FG()),
                );
                ui.add_space(4.0);

                egui::Grid::new("ssh_form")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Name:").size(11.0).color(t::FG_SOFT()));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.ssh_draft.name)
                                .desired_width(200.0)
                                .hint_text("optional"),
                        );
                        ui.end_row();

                        ui.label(RichText::new("Host:").size(11.0).color(t::FG_SOFT()));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.ssh_draft.host)
                                .desired_width(200.0),
                        );
                        ui.end_row();

                        ui.label(RichText::new("Port:").size(11.0).color(t::FG_SOFT()));
                        let mut port_str = self.ssh_draft.port.to_string();
                        if ui
                            .add(egui::TextEdit::singleline(&mut port_str).desired_width(60.0))
                            .changed()
                        {
                            if let Ok(p) = port_str.parse::<u16>() {
                                self.ssh_draft.port = p;
                            }
                        }
                        ui.end_row();

                        ui.label(RichText::new("User:").size(11.0).color(t::FG_SOFT()));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.ssh_draft.user)
                                .desired_width(200.0),
                        );
                        ui.end_row();

                        ui.label(
                            RichText::new("Identity file:")
                                .size(11.0)
                                .color(t::FG_SOFT()),
                        );
                        let mut key_path = self.ssh_draft.identity_file.clone().unwrap_or_default();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut key_path)
                                    .desired_width(200.0)
                                    .hint_text("~/.ssh/id_rsa"),
                            )
                            .changed()
                        {
                            self.ssh_draft.identity_file = if key_path.is_empty() {
                                None
                            } else {
                                Some(key_path)
                            };
                        }
                        ui.end_row();
                    });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let can_connect =
                        !self.ssh_draft.host.is_empty() && !self.ssh_draft.user.is_empty();

                    if ui
                        .add_enabled(
                            can_connect,
                            egui::Button::new(RichText::new("Connect").size(12.0).color(t::FG()))
                                .fill(t::ACCENT()),
                        )
                        .clicked()
                    {
                        connect_profile = Some(self.ssh_draft.clone());
                    }

                    if ui
                        .add_enabled(
                            can_connect,
                            egui::Button::new(
                                RichText::new("Save & Connect").size(12.0).color(t::FG()),
                            )
                            .fill(t::ACCENT()),
                        )
                        .clicked()
                    {
                        self.ssh_manager.add_profile(self.ssh_draft.clone());
                        connect_profile = Some(self.ssh_draft.clone());
                    }
                });
            });

        self.show_ssh_dialog = open;

        if let Some(idx) = remove_idx {
            self.ssh_manager.remove_profile(idx);
        }

        if let Some(profile) = connect_profile {
            self.connect_ssh(ctx, &profile);
            self.show_ssh_dialog = false;
            self.ssh_draft = SshProfile::default();
        }
    }
}

fn build_card_picker_items(timeline: &Timeline) -> Vec<CardPickerItem> {
    timeline
        .cards
        .iter()
        .map(|card| match card {
            Card::Shell(sc) => CardPickerItem {
                id: sc.id,
                kind: "Shell",
                label: sc.command.clone(),
            },
            Card::Chat(cc) => {
                let label = if cc.prompt.len() > 50 {
                    format!("{}…", &cc.prompt[..50])
                } else {
                    cc.prompt.clone()
                };
                CardPickerItem {
                    id: cc.id,
                    kind: "Chat",
                    label,
                }
            }
            Card::System(sc) => {
                let label = if sc.message.len() > 50 {
                    format!("{}…", &sc.message[..50])
                } else {
                    sc.message.clone()
                };
                CardPickerItem {
                    id: sc.id,
                    kind: "System",
                    label,
                }
            }
        })
        .collect()
}

impl eframe::App for AineerApp {
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        let tab_titles: Vec<(u64, String)> = self.tab_manager.tab_ids_and_titles();
        let active_id = self.tab_manager.active_tab_id();

        let mut active_tab_index = 0usize;
        let tabs: Vec<crate::session::TabSession> = self
            .tab_states
            .iter()
            .enumerate()
            .map(|(i, (tid, state))| {
                if active_id == Some(*tid) {
                    active_tab_index = i;
                }
                let title = tab_titles
                    .iter()
                    .find(|(id, _)| id == tid)
                    .map(|(_, t)| t.clone())
                    .unwrap_or_default();
                let working_dir = self
                    .tab_manager
                    .tab(*tid)
                    .and_then(|tab| tab.backend.current_cwd())
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                crate::session::TabSession {
                    title,
                    working_dir,
                    cards: state
                        .timeline
                        .cards
                        .iter()
                        .map(crate::session::CardData::from_card)
                        .collect(),
                }
            })
            .collect();

        let data = crate::session::SessionData {
            tabs,
            active_tab_index,
            split_fraction: 0.6,
        };

        if let Err(e) = crate::session::save_session(&data) {
            tracing::warn!("Failed to save session: {e}");
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Global shortcuts
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(Key::T) {
                self.tab_manager
                    .create_tab(ctx.clone(), self.pty_sender.clone());
                let new_id = self.tab_manager.active_tab_id().unwrap();
                self.ensure_tab_state(new_id);
            }
            if i.modifiers.command && i.key_pressed(Key::W) {
                if let Some(id) = self.tab_manager.active_tab_id() {
                    self.tab_manager.remove_tab(id);
                    self.tab_states.retain(|(tid, _)| *tid != id);
                }
            }
        });

        // Ctrl+Shift+P / Cmd+Shift+P: command palette
        if ctx.input(|i| i.modifiers.command && i.modifiers.shift && i.key_pressed(Key::P)) {
            self.command_palette.toggle();
        }

        self.process_pty_events(ctx);
        self.refresh_running_cards();
        self.poll_git_status();
        self.poll_agent_streams();

        // Re-sync terminal theme if the UI theme mode changed
        {
            let current_mode = ui::theme::current_mode();
            let need_refresh = ctx.memory_mut(|m| {
                let prev: Option<String> = m.data.get_temp(egui::Id::new("__last_theme_mode"));
                let cur = current_mode.as_str().to_string();
                let changed = prev.as_deref() != Some(current_mode.as_str());
                m.data.insert_temp(egui::Id::new("__last_theme_mode"), cur);
                changed
            });
            if need_refresh {
                self.terminal_theme = theme::aineer_terminal_theme();
            }
        }

        if self.tab_manager.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let active_tab_id = self.tab_manager.active_tab_id().unwrap_or(0);

        // Check if we're in fullscreen overlay mode (interactive command like vim)
        let is_alternate_screen = self
            .tab_manager
            .active_tab_mut()
            .map(|t| {
                t.backend.sync();
                t.backend.is_alternate_screen()
            })
            .unwrap_or(false);

        // Update overlay state
        if let Some(state) = self.tab_state_mut(active_tab_id) {
            if is_alternate_screen && !state.fullscreen_overlay {
                state.fullscreen_overlay = true;
            } else if !is_alternate_screen && state.fullscreen_overlay {
                state.fullscreen_overlay = false;
            }
            state.input_bar.set_shell_paused(state.fullscreen_overlay);
        }

        let fullscreen_overlay = self
            .tab_state_mut(active_tab_id)
            .map(|s| s.fullscreen_overlay)
            .unwrap_or(false);

        use ui::theme as t;

        egui::TopBottomPanel::top("tab_bar")
            .exact_height(t::TOOLBAR_HEIGHT)
            .frame(
                egui::Frame::new()
                    .fill(t::BG_ELEVATED())
                    .stroke(egui::Stroke::new(1.0, t::BORDER_SUBTLE()))
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ctx, |ui| {
                self.show_tab_bar(ui, ctx);
            });

        // Command palette overlay
        if let Some(action) = self.command_palette.show(ctx) {
            self.handle_palette_action(ctx, &action);
        }

        if fullscreen_overlay {
            // Fullscreen terminal overlay for interactive commands (vim, htop, etc.)
            egui::TopBottomPanel::bottom("overlay_bar")
                .exact_height(28.0)
                .frame(
                    egui::Frame::new()
                        .fill(t::blend(t::BG(), t::WARNING(), 0.08))
                        .stroke(egui::Stroke::new(
                            1.0,
                            t::blend(t::BORDER_SUBTLE(), t::WARNING(), 0.3),
                        )),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Interactive mode — exit the program to return to cards")
                                .small()
                                .color(t::WARNING()),
                        );
                    });
                });

            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(t::BG()))
                .show(ctx, |ui| {
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        let terminal = TerminalView::new(ui, &mut tab.backend)
                            .set_focus(true)
                            .set_theme(self.terminal_theme.clone())
                            .set_font(TerminalFont::new(FontSettings {
                                font_type: FontId::monospace(self.font_size),
                            }))
                            .set_size(Vec2::new(ui.available_width(), ui.available_height()));
                        ui.add(terminal);
                    }
                });
        } else {
            // Activity bar (leftmost)
            {
                let show_explorer = ctx
                    .data(|d| d.get_temp::<bool>(egui::Id::new("show_explorer")))
                    .unwrap_or(false);
                let mut active_item = None;
                if show_explorer {
                    active_item = Some(ActivityItem::Explorer);
                } else if self.diff_panel.visible {
                    active_item = Some(ActivityItem::Diff);
                } else if self.settings_panel.open {
                    active_item = Some(ActivityItem::Settings);
                } else if self.show_ssh_dialog {
                    active_item = Some(ActivityItem::Ssh);
                }
                self.activity_bar.set_active(active_item);

                egui::SidePanel::left("activity_bar")
                    .exact_width(ACTIVITY_BAR_WIDTH)
                    .frame(
                        egui::Frame::new()
                            .fill(t::BG_ELEVATED())
                            .stroke(egui::Stroke::new(1.0, t::BORDER_SUBTLE()))
                            .inner_margin(egui::Margin::same(0)),
                    )
                    .show(ctx, |ui| {
                        if let Some(item) = self.activity_bar.show(ui) {
                            match item {
                                ActivityItem::Terminal => {
                                    self.settings_panel.open = false;
                                    self.diff_panel.visible = false;
                                    ctx.data_mut(|d| {
                                        d.insert_temp(egui::Id::new("show_explorer"), false)
                                    });
                                }
                                ActivityItem::Explorer => {
                                    let cur = ctx
                                        .data(|d| {
                                            d.get_temp::<bool>(egui::Id::new("show_explorer"))
                                        })
                                        .unwrap_or(false);
                                    ctx.data_mut(|d| {
                                        d.insert_temp(egui::Id::new("show_explorer"), !cur)
                                    });
                                    self.settings_panel.open = false;
                                    self.diff_panel.visible = false;
                                }
                                ActivityItem::Diff => {
                                    self.diff_panel.toggle();
                                    self.settings_panel.open = false;
                                    ctx.data_mut(|d| {
                                        d.insert_temp(egui::Id::new("show_explorer"), false)
                                    });
                                }
                                ActivityItem::Settings => {
                                    self.settings_panel.toggle();
                                    self.diff_panel.visible = false;
                                    ctx.data_mut(|d| {
                                        d.insert_temp(egui::Id::new("show_explorer"), false)
                                    });
                                }
                                ActivityItem::Ssh => {
                                    self.show_ssh_dialog = !self.show_ssh_dialog;
                                }
                            }
                        }
                    });
            }

            // Explorer panel (left side drawer)
            {
                let show_explorer = ctx
                    .data(|d| d.get_temp::<bool>(egui::Id::new("show_explorer")))
                    .unwrap_or(false);
                if show_explorer {
                    // Sync root with active terminal's CWD
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        if let Some(cwd) = tab.backend.current_cwd() {
                            self.explorer.set_root(cwd);
                        }
                    }
                    egui::SidePanel::left("explorer_panel")
                        .default_width(220.0)
                        .min_width(160.0)
                        .max_width(400.0)
                        .frame(
                            egui::Frame::new()
                                .fill(t::PANEL_BG())
                                .stroke(egui::Stroke::new(1.0, t::BORDER_SUBTLE()))
                                .inner_margin(egui::Margin::same(8)),
                        )
                        .show(ctx, |ui| {
                            let action = self.explorer.show(ui);
                            if let ExplorerAction::ChangeDir(path) = action {
                                if let Some(tab) = self.tab_manager.active_tab_mut() {
                                    let cmd =
                                        format!("cd {}\n", shell_escape(&path.to_string_lossy()));
                                    tab.backend
                                        .process_command(BackendCommand::Write(cmd.into_bytes()));
                                }
                            }
                        });
                }
            }

            // Settings panel (rightmost)
            if self.settings_panel.open {
                egui::SidePanel::right("settings_panel")
                    .default_width(380.0)
                    .min_width(300.0)
                    .max_width(600.0)
                    .frame(
                        egui::Frame::new()
                            .fill(t::BG_ELEVATED())
                            .stroke(egui::Stroke::new(1.0, t::BORDER_SUBTLE()))
                            .inner_margin(egui::Margin::symmetric(16, 12)),
                    )
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Settings")
                                    .size(16.0)
                                    .strong()
                                    .color(t::FG()),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("✕").color(t::FG_DIM()),
                                            )
                                            .fill(egui::Color32::TRANSPARENT)
                                            .corner_radius(t::BUTTON_CORNER_RADIUS),
                                        )
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        self.settings_panel.open = false;
                                    }
                                },
                            );
                        });
                        ui.add_space(4.0);
                        let save_result = self.settings_panel.show(ui);
                        if let Some(ok) = save_result {
                            if ok {
                                self.toasts.success("Settings saved");
                            } else {
                                self.toasts.error("Failed to save settings");
                            }
                        }
                    });
            }

            // Diff panel (right side drawer)
            if self.diff_panel.visible {
                egui::SidePanel::right("diff_panel")
                    .default_width(320.0)
                    .min_width(240.0)
                    .max_width(600.0)
                    .frame(
                        egui::Frame::new()
                            .fill(t::PANEL_BG())
                            .stroke(egui::Stroke::new(1.0, t::BORDER_SUBTLE()))
                            .inner_margin(egui::Margin::same(10)),
                    )
                    .show(ctx, |ui| {
                        ui.heading(RichText::new("Changes").size(14.0));
                        ui.separator();
                        let diff_action = self.diff_panel.show(ui);
                        if let ui::diff_panel::DiffAction::RevertHunk { file, hunk_idx } =
                            diff_action
                        {
                            if let Some(status) = &self.git_status {
                                if let Some(file_diff) = status.diffs.get(&file) {
                                    if let Some(hunk) = file_diff.hunks.get(hunk_idx) {
                                        if let Err(e) = ui::git_diff::revert_hunk(
                                            &status.repo_root,
                                            &file,
                                            hunk,
                                        ) {
                                            tracing::warn!("Revert hunk failed: {e}");
                                        }
                                    }
                                }
                            }
                        }
                    });
            }

            // Status bar (very bottom)
            egui::TopBottomPanel::bottom("status_bar")
                .exact_height(t::STATUS_BAR_HEIGHT)
                .frame(
                    egui::Frame::new()
                        .fill(t::BG_ELEVATED())
                        .stroke(egui::Stroke::new(1.0, t::BORDER_SUBTLE()))
                        .inner_margin(egui::Margin::symmetric(4, 0)),
                )
                .show(ctx, |ui| {
                    self.show_status_bar(ui);
                });

            // Input bar at bottom (above status bar)
            egui::TopBottomPanel::bottom("input_bar")
                .frame(
                    egui::Frame::new()
                        .fill(t::BG_ELEVATED())
                        .stroke(egui::Stroke::new(1.0, t::BORDER_SUBTLE())),
                )
                .show(ctx, |ui| {
                    let tab_id = active_tab_id;
                    let settings_open = self.settings_panel.open;
                    let cwd_str = self
                        .tab_manager
                        .active_tab_mut()
                        .and_then(|t| t.backend.current_cwd())
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "~".to_string());
                    let git_branch = self.git_status.as_ref().and_then(|s| s.branch.clone());
                    if let Some(state) = self
                        .tab_states
                        .iter_mut()
                        .find(|(id, _)| *id == tab_id)
                        .map(|(_, s)| s)
                    {
                        state
                            .input_bar
                            .set_prompt_context(&cwd_str, git_branch.as_deref());
                        let card_items = build_card_picker_items(&state.timeline);
                        state.input_bar.focus_enabled = !settings_open;
                        let action = state.input_bar.show(ui, &card_items, &self.slash_items);
                        match action {
                            SubmitAction::Shell(cmd) => {
                                // We need to defer this because we borrow self mutably above
                                ctx.memory_mut(|m| {
                                    m.data.insert_temp(egui::Id::new("pending_shell_cmd"), cmd);
                                });
                            }
                            SubmitAction::Chat { text, refs } => {
                                ctx.memory_mut(|m| {
                                    m.data.insert_temp(egui::Id::new("pending_chat_text"), text);
                                    m.data.insert_temp(egui::Id::new("pending_chat_refs"), refs);
                                });
                            }
                            SubmitAction::None => {}
                        }
                    }
                });

            // Process deferred actions
            let shell_cmd: Option<String> = ctx.memory_mut(|m| {
                m.data
                    .get_temp::<String>(egui::Id::new("pending_shell_cmd"))
            });
            if let Some(cmd) = shell_cmd {
                ctx.memory_mut(|m| {
                    m.data.remove::<String>(egui::Id::new("pending_shell_cmd"));
                });
                self.handle_shell_submit(cmd);
            }

            let chat_text: Option<String> = ctx.memory_mut(|m| {
                m.data
                    .get_temp::<String>(egui::Id::new("pending_chat_text"))
            });
            if let Some(text) = chat_text {
                let refs: Vec<u64> = ctx
                    .memory_mut(|m| {
                        m.data
                            .get_temp::<Vec<u64>>(egui::Id::new("pending_chat_refs"))
                    })
                    .unwrap_or_default();
                ctx.memory_mut(|m| {
                    m.data.remove::<String>(egui::Id::new("pending_chat_text"));
                    m.data
                        .remove::<Vec<u64>>(egui::Id::new("pending_chat_refs"));
                });
                self.handle_chat_submit(text, refs);
            }

            // Main content: full-height timeline (live terminal view removed)
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(t::BG()))
                .show(ctx, |ui| {
                    let timeline_action = if let Some(state) = self.tab_state_mut(active_tab_id) {
                        state.timeline.show(ui)
                    } else {
                        TimelineAction::None
                    };
                    self.handle_timeline_action(active_tab_id, timeline_action);
                });
        }

        self.show_ssh_popup(ctx);

        // Toast notifications — rendered on top of all other UI
        self.toasts.show(ctx);
    }
}

/// Escape a path for safe use in a shell `cd` command.
fn shell_escape(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}
