use aineer_ui::blocks::*;
use chrono::Utc;
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::io::{BufRead, BufReader};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ═══════════════════════════════════════════════════════════════════
//  Actions
// ═══════════════════════════════════════════════════════════════════

actions!(
    workspace,
    [
        ToggleSidebar,
        NewTab,
        CloseTab,
        SwitchToShellMode,
        SwitchToAIMode,
        SwitchToAgentMode,
        FocusInput,
        ClearBlocks,
    ]
);

// ═══════════════════════════════════════════════════════════════════
//  Types
// ═══════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidebarPanel {
    Explorer,
    Search,
    Git,
    Context,
    Memory,
    Settings,
}

impl SidebarPanel {
    fn label(self) -> &'static str {
        match self {
            Self::Explorer => "EXPLORER",
            Self::Search => "SEARCH",
            Self::Git => "GIT",
            Self::Context => "CONTEXT",
            Self::Memory => "MEMORY",
            Self::Settings => "SETTINGS",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Explorer => "E",
            Self::Search => "S",
            Self::Git => "G",
            Self::Context => "C",
            Self::Memory => "M",
            Self::Settings => "⚙",
        }
    }
}

const ALL_PANELS: [SidebarPanel; 5] = [
    SidebarPanel::Explorer,
    SidebarPanel::Search,
    SidebarPanel::Git,
    SidebarPanel::Context,
    SidebarPanel::Memory,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Shell,
    AIChat,
    Agent,
}

impl InputMode {
    fn label(self) -> &'static str {
        match self {
            Self::Shell => "Shell",
            Self::AIChat => "AI",
            Self::Agent => "Agent",
        }
    }
    fn next(self) -> Self {
        match self {
            Self::Shell => Self::AIChat,
            Self::AIChat => Self::Agent,
            Self::Agent => Self::Shell,
        }
    }
}

// ─── Toast ───

#[derive(Clone)]
struct Toast {
    id: u64,
    kind: ToastKind,
    message: String,
}

#[derive(Clone, Copy, PartialEq)]
enum ToastKind {
    Success,
    Error,
    Info,
}

// ─── Search Result ───

#[derive(Clone)]
struct SearchResult {
    file_path: PathBuf,
    line_number: usize,
    line_content: String,
}

// ─── Per-Tab State ───

struct TabState {
    id: usize,
    title: String,
    blocks: Vec<Block>,
    next_block_id: u64,
    input_text: String,
    cursor_pos: usize,
    command_history: Vec<String>,
    history_index: Option<usize>,
    saved_input: String,
    cwd: PathBuf,
    file_tree: Vec<FileNode>,
    scroll_handle: ScrollHandle,
    cached_git_branch: Option<String>,
    input_mode: InputMode,
    ai_streaming: bool,
    ai_cancel: Arc<AtomicBool>,
    search_active: bool,
    search_query: String,
    command_palette_open: bool,
    command_palette_query: String,
    marked_range: Option<std::ops::Range<usize>>,
    previewing_file: Option<PathBuf>,
    file_preview_content: Option<String>,
    hovered_block: Option<usize>,
    toasts: Vec<Toast>,
    sidebar_search_query: String,
    sidebar_search_results: Vec<SearchResult>,
}

impl TabState {
    fn new(id: usize, title: String, cwd: PathBuf) -> Self {
        let file_tree = load_directory(&cwd, 0);
        let cached_git_branch = read_git_branch(&cwd);
        Self {
            id,
            title,
            blocks: Vec::new(),
            next_block_id: 1,
            input_text: String::new(),
            cursor_pos: 0,
            command_history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
            cwd,
            file_tree,
            scroll_handle: ScrollHandle::new(),
            cached_git_branch,
            input_mode: InputMode::Shell,
            ai_streaming: false,
            ai_cancel: Arc::new(AtomicBool::new(false)),
            search_active: false,
            search_query: String::new(),
            command_palette_open: false,
            command_palette_query: String::new(),
            marked_range: None,
            previewing_file: None,
            file_preview_content: None,
            hovered_block: None,
            toasts: Vec::new(),
            sidebar_search_query: String::new(),
            sidebar_search_results: Vec::new(),
        }
    }

    fn new_meta(&mut self) -> BlockMeta {
        let id = self.next_block_id;
        self.next_block_id += 1;
        BlockMeta {
            id,
            created_at: Utc::now(),
            collapsed: false,
            parent_id: None,
            tags: vec![],
        }
    }
}

// ─── File Tree ───

#[derive(Debug, Clone)]
struct FileNode {
    name: String,
    path: PathBuf,
    is_dir: bool,
    depth: usize,
    expanded: bool,
    children_loaded: bool,
}

// ═══════════════════════════════════════════════════════════════════
//  Color Palette (§2.1 Design Tokens)
// ═══════════════════════════════════════════════════════════════════

struct Clr;
impl Clr {
    const BG: u32 = 0x1e1e2e;
    const SURFACE: u32 = 0x232334;
    const ELEVATED: u32 = 0x2a2a3c;
    const TEXT: u32 = 0xe0e0e8;
    const TEXT2: u32 = 0x8888a0;
    const MUTED: u32 = 0x5c5c72;
    const ACCENT: u32 = 0x5b9cf5;
    const AI: u32 = 0xb07aff;
    const AGENT: u32 = 0xf0a050;
    const BORDER: u32 = 0x3a3a4e;
    const SUCCESS: u32 = 0x50c878;
    const ERROR: u32 = 0xf44747;
    const BAR: u32 = 0x1a1a28;
    const HOVER: u32 = 0x2e2e40;
}

// ═══════════════════════════════════════════════════════════════════
//  Workspace
// ═══════════════════════════════════════════════════════════════════

pub struct AineerWorkspace {
    focus_handle: FocusHandle,

    // Tabs (each tab owns its own state)
    tabs: Vec<TabState>,
    active_tab: usize,
    next_tab_id: usize,

    // Sidebar (shared across tabs)
    sidebar_visible: bool,
    sidebar_width: f32,
    active_panel: SidebarPanel,

    // Backend bridge
    bridge: crate::bridge::TokioBridge,

    // Gateway
    gateway_connected: bool,

    // Theme
    theme: aineer_theme::Theme,

    // Settings store
    settings_store: Option<aineer_settings::SettingsStore>,

    // Settings UI state
    settings_api_key_input: String,
    settings_editing_provider: Option<String>,
    settings_test_result: Option<(String, bool)>,

    // Flags
    needs_initial_focus: bool,
}

impl AineerWorkspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let bridge = crate::bridge::TokioBridge::new();

        let (tabs, active_tab, restored_sidebar_visible, restored_sidebar_width, max_tab_id) =
            restore_session(&cwd);

        let user_settings_path = dirs_or_home().join(".aineer").join("settings.json");
        let settings_store = aineer_settings::SettingsStore::load(user_settings_path, None).ok();

        Self {
            focus_handle,
            tabs,
            active_tab,
            next_tab_id: max_tab_id + 1,
            sidebar_visible: restored_sidebar_visible,
            sidebar_width: restored_sidebar_width,
            active_panel: SidebarPanel::Explorer,
            bridge,
            gateway_connected: false,
            theme: aineer_theme::Theme::dark_default(),
            settings_store,
            settings_api_key_input: String::new(),
            settings_editing_provider: None,
            settings_test_result: None,
            needs_initial_focus: true,
        }
    }

    fn tab(&self) -> &TabState {
        &self.tabs[self.active_tab]
    }

    fn tab_mut(&mut self) -> &mut TabState {
        &mut self.tabs[self.active_tab]
    }

    fn utf8_offset_to_utf16(&self, offset: usize) -> usize {
        self.tab()
            .input_text
            .get(..offset)
            .map(|s| s.encode_utf16().count())
            .unwrap_or(0)
    }

    fn utf16_to_utf8_offset(&self, utf16_offset: usize) -> usize {
        let text = &self.tab().input_text;
        let mut utf16_count = 0;
        for (byte_offset, ch) in text.char_indices() {
            if utf16_count >= utf16_offset {
                return byte_offset;
            }
            utf16_count += ch.len_utf16();
        }
        text.len()
    }
}

impl EntityInputHandler for AineerWorkspace {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let offset = self.utf8_offset_to_utf16(self.tab().cursor_pos);
        Some(UTF16Selection {
            range: offset..offset,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.tab().marked_range.clone()
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let start = self.utf16_to_utf8_offset(range_utf16.start);
        let end = self.utf16_to_utf8_offset(range_utf16.end);
        self.tab().input_text.get(start..end).map(|s| s.to_string())
    }

    fn replace_text_in_range(
        &mut self,
        replacement_range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab = self.tab_mut();
        tab.marked_range = None;
        let (start, end) = if let Some(range) = replacement_range {
            let s = utf16_to_utf8_offset_text(&tab.input_text, range.start);
            let e = utf16_to_utf8_offset_text(&tab.input_text, range.end);
            (s, e)
        } else {
            (tab.cursor_pos, tab.cursor_pos)
        };
        let start = start.min(tab.input_text.len());
        let end = end.min(tab.input_text.len());
        tab.input_text.replace_range(start..end, text);
        tab.cursor_pos = start + text.len();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let tab = self.tab_mut();
        let (start, end) = if let Some(range) = range_utf16 {
            let s = utf16_to_utf8_offset_text(&tab.input_text, range.start);
            let e = utf16_to_utf8_offset_text(&tab.input_text, range.end);
            (s, e)
        } else if let Some(ref marked) = tab.marked_range {
            let s = utf16_to_utf8_offset_text(&tab.input_text, marked.start);
            let e = utf16_to_utf8_offset_text(&tab.input_text, marked.end);
            (s, e)
        } else {
            (tab.cursor_pos, tab.cursor_pos)
        };
        let start = start.min(tab.input_text.len());
        let end = end.min(tab.input_text.len());
        tab.input_text.replace_range(start..end, new_text);
        tab.cursor_pos = start + new_text.len();

        let mark_start = tab.input_text[..start].encode_utf16().count();
        let mark_end = mark_start + new_text.encode_utf16().count();
        tab.marked_range = if new_text.is_empty() {
            None
        } else {
            Some(mark_start..mark_end)
        };
        cx.notify();
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.tab_mut().marked_range = None;
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(element_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.utf8_offset_to_utf16(self.tab().cursor_pos))
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Action Handlers
// ═══════════════════════════════════════════════════════════════════

impl AineerWorkspace {
    fn on_toggle_sidebar(&mut self, _: &ToggleSidebar, _w: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        cx.notify();
    }
    fn on_new_tab(&mut self, _: &NewTab, _w: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let cwd = self.tab().cwd.clone();
        self.tabs
            .push(TabState::new(id, format!("Terminal {}", id), cwd));
        self.active_tab = self.tabs.len() - 1;
        cx.notify();
    }
    fn on_close_tab(&mut self, _: &CloseTab, _w: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() > 1 {
            self.tabs.remove(self.active_tab);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
            self.save_session();
            cx.notify();
        }
    }
    fn on_shell_mode(&mut self, _: &SwitchToShellMode, _w: &mut Window, cx: &mut Context<Self>) {
        self.tab_mut().input_mode = InputMode::Shell;
        cx.notify();
    }
    fn on_ai_mode(&mut self, _: &SwitchToAIMode, _w: &mut Window, cx: &mut Context<Self>) {
        self.tab_mut().input_mode = InputMode::AIChat;
        cx.notify();
    }
    fn on_agent_mode(&mut self, _: &SwitchToAgentMode, _w: &mut Window, cx: &mut Context<Self>) {
        self.tab_mut().input_mode = InputMode::Agent;
        cx.notify();
    }
    fn on_focus_input(&mut self, _: &FocusInput, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window);
        cx.notify();
    }
    fn on_clear_blocks(&mut self, _: &ClearBlocks, _w: &mut Window, cx: &mut Context<Self>) {
        self.tab_mut().blocks.clear();
        cx.notify();
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Keyboard Input
// ═══════════════════════════════════════════════════════════════════

impl AineerWorkspace {
    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let mods = &event.keystroke.modifiers;

        // Route to command palette when open
        if self.tab().command_palette_open {
            match key {
                "escape" => {
                    self.tab_mut().command_palette_open = false;
                    self.tab_mut().command_palette_query.clear();
                }
                "enter" => {
                    self.execute_palette_command(cx);
                }
                "backspace" => {
                    let q = &mut self.tab_mut().command_palette_query;
                    if !q.is_empty() {
                        q.pop();
                    }
                }
                _ if !mods.platform && !mods.control && !mods.function => {
                    if let Some(ch) = &event.keystroke.key_char {
                        if !ch.is_empty() {
                            self.tab_mut().command_palette_query.push_str(ch);
                        }
                    }
                }
                "p" if mods.platform && mods.shift => {
                    self.tab_mut().command_palette_open = false;
                    self.tab_mut().command_palette_query.clear();
                }
                _ => {}
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }

        // Route to block search when active
        if self.tab().search_active && key != "f" {
            match key {
                "escape" => {
                    let tab = self.tab_mut();
                    tab.search_active = false;
                    tab.search_query.clear();
                }
                "backspace" => {
                    let q = &mut self.tab_mut().search_query;
                    if !q.is_empty() {
                        q.pop();
                    }
                }
                _ if !mods.platform && !mods.control && !mods.function => {
                    if let Some(ch) = &event.keystroke.key_char {
                        if !ch.is_empty() {
                            self.tab_mut().search_query.push_str(ch);
                        }
                    }
                }
                _ => {}
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }

        // Route to sidebar search when search panel is active and sidebar visible
        if self.sidebar_visible
            && self.active_panel == SidebarPanel::Search
            && self.settings_editing_provider.is_none()
        {
            match key {
                "backspace" if !self.tab().sidebar_search_query.is_empty() => {
                    self.tab_mut().sidebar_search_query.pop();
                    cx.notify();
                    return;
                }
                "enter" if !self.tab().sidebar_search_query.is_empty() => {
                    self.run_content_search();
                    cx.notify();
                    return;
                }
                _ if !mods.platform && !mods.control && !mods.function => {
                    if let Some(ch) = &event.keystroke.key_char {
                        if !ch.is_empty() {
                            self.tab_mut().sidebar_search_query.push_str(ch);
                            cx.notify();
                            return;
                        }
                    }
                }
                _ => {}
            }
        }

        // Route keyboard to settings API key input when editing
        if let Some(ref _provider) = self.settings_editing_provider {
            match key {
                "backspace" => {
                    if !self.settings_api_key_input.is_empty() {
                        self.settings_api_key_input.pop();
                    }
                    cx.notify();
                    return;
                }
                "escape" => {
                    self.settings_editing_provider = None;
                    self.settings_api_key_input.clear();
                    cx.notify();
                    return;
                }
                _ if !mods.platform && !mods.control && !mods.function => {
                    if let Some(ch) = &event.keystroke.key_char {
                        if !ch.is_empty() {
                            self.settings_api_key_input.push_str(ch);
                            cx.notify();
                            return;
                        }
                    }
                }
                _ => {}
            }
        }

        match key {
            "enter" if mods.platform && mods.shift => {
                self.tab_mut().input_mode = InputMode::Agent;
                self.submit_input(cx);
            }
            "enter" if mods.platform && !mods.shift => {
                self.tab_mut().input_mode = InputMode::AIChat;
                self.submit_input(cx);
            }
            "enter" if mods.shift && !mods.platform => {
                let tab = self.tab_mut();
                tab.input_text.insert(tab.cursor_pos, '\n');
                tab.cursor_pos += 1;
            }
            "enter" if !mods.shift && !mods.platform => {
                self.submit_input(cx);
            }
            "backspace" => {
                let tab = self.tab_mut();
                if tab.cursor_pos > 0 {
                    let remove_pos = tab.input_text[..tab.cursor_pos]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    tab.input_text.remove(remove_pos);
                    tab.cursor_pos = remove_pos;
                }
            }
            "left" => {
                let tab = self.tab_mut();
                if tab.cursor_pos > 0 {
                    tab.cursor_pos = tab.input_text[..tab.cursor_pos]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
            }
            "right" => {
                let tab = self.tab_mut();
                if tab.cursor_pos < tab.input_text.len() {
                    tab.cursor_pos += tab.input_text[tab.cursor_pos..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                }
            }
            "up" => {
                self.history_prev();
            }
            "down" => {
                self.history_next();
            }
            "escape" => {
                let tab = self.tab_mut();
                if !tab.input_text.is_empty() {
                    tab.input_text.clear();
                    tab.cursor_pos = 0;
                } else if tab.input_mode != InputMode::Shell {
                    tab.input_mode = InputMode::Shell;
                } else {
                    window.blur();
                }
            }
            "a" if mods.control => {
                self.tab_mut().cursor_pos = 0;
            }
            "e" if mods.control => {
                let len = self.tab().input_text.len();
                self.tab_mut().cursor_pos = len;
            }
            "u" if mods.control => {
                let tab = self.tab_mut();
                tab.input_text.drain(..tab.cursor_pos);
                tab.cursor_pos = 0;
            }
            "k" if mods.control => {
                let pos = self.tab().cursor_pos;
                self.tab_mut().input_text.truncate(pos);
            }
            "l" if mods.control => {
                self.tab_mut().blocks.clear();
            }
            "f" if mods.platform => {
                let tab = self.tab_mut();
                tab.search_active = !tab.search_active;
                if !tab.search_active {
                    tab.search_query.clear();
                }
            }
            "p" if mods.platform && mods.shift => {
                let tab = self.tab_mut();
                tab.command_palette_open = !tab.command_palette_open;
                tab.command_palette_query.clear();
            }
            _ => {
                if mods.platform || mods.function {
                    return;
                }
                if let Some(ch) = &event.keystroke.key_char {
                    if !ch.is_empty() {
                        let tab = self.tab_mut();
                        tab.input_text.insert_str(tab.cursor_pos, ch);
                        tab.cursor_pos += ch.len();
                        tab.history_index = None;
                    }
                }
            }
        }

        cx.stop_propagation();
        cx.notify();
    }

    fn history_prev(&mut self) {
        let tab = self.tab_mut();
        if tab.command_history.is_empty() {
            return;
        }
        match tab.history_index {
            None => {
                tab.saved_input = tab.input_text.clone();
                tab.history_index = Some(tab.command_history.len() - 1);
            }
            Some(0) => return,
            Some(ref mut idx) => *idx -= 1,
        }
        if let Some(idx) = tab.history_index {
            tab.input_text = tab.command_history[idx].clone();
            tab.cursor_pos = tab.input_text.len();
        }
    }

    fn history_next(&mut self) {
        let tab = self.tab_mut();
        match tab.history_index {
            None => return,
            Some(idx) => {
                if idx + 1 >= tab.command_history.len() {
                    tab.history_index = None;
                    tab.input_text = tab.saved_input.clone();
                } else {
                    tab.history_index = Some(idx + 1);
                    tab.input_text = tab.command_history[idx + 1].clone();
                }
            }
        }
        tab.cursor_pos = tab.input_text.len();
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Command Execution
// ═══════════════════════════════════════════════════════════════════

enum ShellEvent {
    Line(String),
    Exit(Option<i32>, std::time::Duration),
    Error(String),
}

enum AgentEvent {
    PlanReady(Vec<String>),
    StepStarted(usize),
    StepCompleted {
        index: usize,
        output: String,
        exit_code: Option<i32>,
    },
    StepFailed {
        index: usize,
        error: String,
    },
    AllDone,
    Error(String),
}

const DANGEROUS_COMMANDS: &[&str] = &[
    "rm ", "rm\t", "rmdir", "sudo ", "chmod ", "chown ", "dd ", "mkfs", "fdisk", "kill ", "pkill ",
    "shutdown", "reboot", "mv /", "format ",
];

fn is_dangerous_command(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    DANGEROUS_COMMANDS
        .iter()
        .any(|d| trimmed.starts_with(d) || trimmed.contains(&format!(" && {}", d)))
}

enum AiStreamEvent {
    Delta(String),
    Done { model: String, tokens: Option<u32> },
    Error(String),
}

impl AineerWorkspace {
    fn submit_input(&mut self, cx: &mut Context<Self>) {
        let text = self.tab().input_text.trim().to_string();
        if text.is_empty() {
            return;
        }

        let tab = self.tab_mut();
        tab.input_text.clear();
        tab.cursor_pos = 0;
        tab.history_index = None;

        let mode = self.tab().input_mode;

        match mode {
            InputMode::Shell => {
                self.tab_mut().command_history.push(text.clone());
                self.execute_shell(&text, cx);
            }
            InputMode::AIChat => {
                self.execute_ai(&text, cx);
            }
            InputMode::Agent => {
                self.execute_agent(&text, cx);
            }
        }
    }

    fn stop_ai_streaming(&mut self, cx: &mut Context<Self>) {
        let tab = self.tab_mut();
        tab.ai_cancel.store(true, Ordering::Relaxed);
        tab.ai_streaming = false;
        if let Some(Block::AI(ai)) = tab.blocks.last_mut() {
            if ai.streaming {
                ai.streaming = false;
                if ai.content.is_empty() {
                    ai.content = "(cancelled)".into();
                }
            }
        }
        cx.notify();
    }

    // ─── Agent execution with plan decomposition ───

    fn execute_agent(&mut self, goal: &str, cx: &mut Context<Self>) {
        let model_str = resolve_ai_model();
        if model_str.is_empty() {
            let tab = self.tab_mut();
            let meta = tab.new_meta();
            tab.blocks.push(Block::System(SystemBlock {
                meta,
                kind: SystemKind::Error,
                message: "No AI provider configured. Set ANTHROPIC_API_KEY, OPENAI_API_KEY, or XAI_API_KEY.".into(),
            }));
            tab.scroll_handle.scroll_to_bottom();
            cx.notify();
            return;
        }

        let tab = self.tab_mut();
        let meta = tab.new_meta();
        let plan_block_id = meta.id;
        tab.blocks.push(Block::AgentPlan(AgentPlanBlock {
            meta,
            goal: goal.to_string(),
            steps: vec![],
            state: AgentPlanState::Planning,
            approval_policy: ApprovalPolicy::DangerousOnly,
        }));
        tab.ai_streaming = true;
        let cancel = Arc::new(AtomicBool::new(false));
        tab.ai_cancel = cancel.clone();
        let cwd = tab.cwd.clone();
        tab.scroll_handle.scroll_to_bottom();
        cx.notify();

        let goal_owned = goal.to_string();
        let (tx, rx) = smol::channel::unbounded::<AgentEvent>();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(_) => return,
            };
            rt.block_on(async move {
                let client = match aineer_api::ProviderClient::from_model(&model_str) {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx
                            .send(AgentEvent::Error(format!("Provider error: {}", e)))
                            .await;
                        return;
                    }
                };

                // Phase 1: Plan
                let plan_prompt = format!(
                    "You are a task planner. Break down this goal into 2-5 concrete steps.\n\
                     Each step should be a single shell command or a brief action.\n\
                     Return ONLY a numbered list, one step per line, no explanations.\n\
                     Goal: {}",
                    goal_owned
                );
                let request = aineer_api::MessageRequest {
                    model: model_str.clone(),
                    max_tokens: 1024,
                    messages: vec![aineer_api::InputMessage::user_text(&plan_prompt)],
                    system: None,
                    tools: None,
                    tool_choice: None,
                    stream: false,
                    thinking: None,
                    gemini_cached_content: None,
                };

                let steps = match client.send_message(&request).await {
                    Ok(response) => {
                        let text: String = response
                            .content
                            .iter()
                            .filter_map(|b| match b {
                                aineer_api::OutputContentBlock::Text { text } => {
                                    Some(text.as_str())
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("");
                        let steps: Vec<String> = text
                            .lines()
                            .filter(|l: &&str| !l.trim().is_empty())
                            .map(|l: &str| {
                                l.trim_start_matches(|c: char| {
                                    c.is_ascii_digit() || c == '.' || c == ' ' || c == '-'
                                })
                                .trim()
                                .to_string()
                            })
                            .filter(|s: &String| !s.is_empty())
                            .collect();
                        let _ = tx.send(AgentEvent::PlanReady(steps.clone())).await;
                        steps
                    }
                    Err(e) => {
                        let _ = tx
                            .send(AgentEvent::Error(format!("Planning failed: {}", e)))
                            .await;
                        return;
                    }
                };

                if cancel.load(Ordering::Relaxed) {
                    return;
                }

                // Phase 2: Execute steps
                for (i, step) in steps.iter().enumerate() {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }

                    // Check for dangerous commands
                    if is_dangerous_command(step) {
                        let _ = tx
                            .send(AgentEvent::StepFailed {
                                index: i,
                                error: format!("Skipped: dangerous command detected — {}", step),
                            })
                            .await;
                        continue;
                    }

                    let _ = tx.send(AgentEvent::StepStarted(i)).await;

                    let output = std::process::Command::new("sh")
                        .arg("-c")
                        .arg(step)
                        .current_dir(&cwd)
                        .output();

                    match output {
                        Ok(result) => {
                            let stdout = String::from_utf8_lossy(&result.stdout).to_string();
                            let stderr = String::from_utf8_lossy(&result.stderr).to_string();
                            let combined = if stderr.is_empty() {
                                stdout
                            } else {
                                format!("{}\n{}", stdout, stderr)
                            };
                            let code = result.status.code();
                            let _ = tx
                                .send(AgentEvent::StepCompleted {
                                    index: i,
                                    output: combined,
                                    exit_code: code,
                                })
                                .await;
                        }
                        Err(e) => {
                            let _ = tx
                                .send(AgentEvent::StepFailed {
                                    index: i,
                                    error: format!("Failed to execute: {}", e),
                                })
                                .await;
                        }
                    }
                }

                let _ = tx.send(AgentEvent::AllDone).await;
            });
        });

        cx.spawn(async move |entity, cx| {
            while let Ok(event) = rx.recv().await {
                let is_done = matches!(event, AgentEvent::AllDone | AgentEvent::Error(_));
                let _ = cx.update(|cx| {
                    if let Some(entity) = entity.upgrade() {
                        entity.update(cx, |this, cx| {
                            let tab = this.tab_mut();
                            if let Some(block) = tab
                                .blocks
                                .iter_mut()
                                .rev()
                                .find(|b| b.id() == plan_block_id)
                            {
                                if let Block::AgentPlan(plan) = block {
                                    match &event {
                                        AgentEvent::PlanReady(steps) => {
                                            plan.steps = steps
                                                .iter()
                                                .enumerate()
                                                .map(|(i, s)| AgentStep {
                                                    index: i,
                                                    description: s.clone(),
                                                    state: StepState::Pending,
                                                    child_block_id: None,
                                                    duration: None,
                                                    is_dangerous: is_dangerous_command(s),
                                                })
                                                .collect();
                                            plan.state = AgentPlanState::Executing;
                                        }
                                        AgentEvent::StepStarted(idx) => {
                                            if let Some(step) = plan.steps.get_mut(*idx) {
                                                step.state = StepState::Running;
                                            }
                                        }
                                        AgentEvent::StepCompleted {
                                            index,
                                            output: _,
                                            exit_code,
                                        } => {
                                            if let Some(step) = plan.steps.get_mut(*index) {
                                                step.state = if exit_code == &Some(0)
                                                    || exit_code.is_none()
                                                {
                                                    StepState::Completed
                                                } else {
                                                    StepState::Failed
                                                };
                                            }
                                        }
                                        AgentEvent::StepFailed { index, error: _ } => {
                                            if let Some(step) = plan.steps.get_mut(*index) {
                                                step.state = StepState::Failed;
                                            }
                                        }
                                        AgentEvent::AllDone => {
                                            let all_ok = plan
                                                .steps
                                                .iter()
                                                .all(|s| matches!(s.state, StepState::Completed));
                                            plan.state = if all_ok {
                                                AgentPlanState::Completed
                                            } else {
                                                AgentPlanState::Failed
                                            };
                                        }
                                        AgentEvent::Error(e) => {
                                            plan.state = AgentPlanState::Failed;
                                            plan.steps.push(AgentStep {
                                                index: plan.steps.len(),
                                                description: e.clone(),
                                                state: StepState::Failed,
                                                child_block_id: None,
                                                duration: None,
                                                is_dangerous: false,
                                            });
                                        }
                                    }
                                }
                            }

                            if is_done {
                                tab.ai_streaming = false;
                            }
                            tab.scroll_handle.scroll_to_bottom();
                            this.save_session();
                            cx.notify();
                        });
                    }
                });
                if is_done {
                    break;
                }
            }
        })
        .detach();
    }

    // ─── AI execution with real API call ───

    fn execute_ai(&mut self, query: &str, cx: &mut Context<Self>) {
        let tab = self.tab_mut();

        // User message block
        let user_meta = tab.new_meta();
        tab.blocks.push(Block::AI(AIBlock {
            meta: user_meta,
            model: String::new(),
            role: Role::User,
            content: query.to_string(),
            streaming: false,
            token_count: None,
            context_refs: vec![],
            executable_snippets: vec![],
        }));

        // Assistant placeholder block (streaming)
        let assistant_meta = tab.new_meta();
        let assistant_block_id = assistant_meta.id;
        tab.blocks.push(Block::AI(AIBlock {
            meta: assistant_meta,
            model: String::new(),
            role: Role::Assistant,
            content: String::new(),
            streaming: true,
            token_count: None,
            context_refs: vec![],
            executable_snippets: vec![],
        }));

        tab.ai_streaming = true;
        let cancel = Arc::new(AtomicBool::new(false));
        tab.ai_cancel = cancel.clone();
        tab.scroll_handle.scroll_to_bottom();
        cx.notify();

        // Resolve model from settings or environment
        let model_str = resolve_ai_model();

        let (tx, rx) = smol::channel::unbounded::<AiStreamEvent>();
        let query_owned = query.to_string();

        // Background thread: runs tokio runtime for async HTTP
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send_blocking(AiStreamEvent::Error(format!(
                        "Failed to create runtime: {}",
                        e
                    )));
                    return;
                }
            };

            rt.block_on(async move {
                let client = match aineer_api::ProviderClient::from_model(&model_str) {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = tx
                            .send(AiStreamEvent::Error(format!(
                                "Provider error for model \"{}\": {}\n\nSet ANTHROPIC_API_KEY, OPENAI_API_KEY, or XAI_API_KEY in your environment.",
                                model_str, e
                            )))
                            .await;
                        return;
                    }
                };

                let request = aineer_api::MessageRequest {
                    model: model_str.clone(),
                    max_tokens: 4096,
                    messages: vec![aineer_api::InputMessage::user_text(&query_owned)],
                    system: None,
                    tools: None,
                    tool_choice: None,
                    stream: true,
                    thinking: None,
                    gemini_cached_content: None,
                };

                match client.stream_message(&request).await {
                    Ok(mut stream) => {
                        loop {
                            if cancel.load(Ordering::Relaxed) {
                                break;
                            }
                            match stream.next_event().await {
                                Ok(Some(event)) => {
                                    if let Some(text) = extract_stream_text(&event) {
                                        if tx.send(AiStreamEvent::Delta(text)).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    let _ = tx
                                        .send(AiStreamEvent::Error(format!("Stream error: {}", e)))
                                        .await;
                                    break;
                                }
                            }
                        }
                        let _ = tx
                            .send(AiStreamEvent::Done {
                                model: model_str,
                                tokens: None,
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(AiStreamEvent::Error(format!(
                                "API error: {}\n\nCheck your API key and model configuration.",
                                e
                            )))
                            .await;
                    }
                }
            });
        });

        // Receive stream events and update UI
        cx.spawn(async move |entity, cx| {
            while let Ok(event) = rx.recv().await {
                let should_break =
                    matches!(event, AiStreamEvent::Done { .. } | AiStreamEvent::Error(_));
                let _ = cx.update(|cx| {
                    if let Some(entity) = entity.upgrade() {
                        entity.update(cx, |this, cx| {
                            let tab = this.tab_mut();
                            if let Some(block) = tab
                                .blocks
                                .iter_mut()
                                .rev()
                                .find(|b| b.id() == assistant_block_id)
                            {
                                if let Block::AI(ai) = block {
                                    match &event {
                                        AiStreamEvent::Delta(text) => {
                                            ai.content.push_str(text);
                                        }
                                        AiStreamEvent::Done { model, tokens } => {
                                            ai.streaming = false;
                                            ai.model = model.clone();
                                            ai.token_count = *tokens;
                                            tab.ai_streaming = false;
                                        }
                                        AiStreamEvent::Error(e) => {
                                            ai.streaming = false;
                                            ai.content = e.clone();
                                            ai.model = "error".into();
                                            tab.ai_streaming = false;
                                        }
                                    }
                                }
                            }
                            tab.scroll_handle.scroll_to_bottom();
                            if !tab.ai_streaming {
                                this.save_session();
                            }
                            cx.notify();
                        });
                    }
                });
                if should_break {
                    break;
                }
            }
        })
        .detach();
    }

    // ─── Shell execution with streaming output ───

    fn execute_shell(&mut self, command: &str, cx: &mut Context<Self>) {
        let cwd = self.tab().cwd.clone();

        if command == "clear" {
            self.tab_mut().blocks.clear();
            cx.notify();
            return;
        }
        if let Some(dir) =
            command
                .strip_prefix("cd ")
                .or_else(|| if command == "cd" { Some("") } else { None })
        {
            self.handle_cd(dir.trim(), &cwd, cx);
            return;
        }

        let tab = self.tab_mut();
        let meta = tab.new_meta();
        let block_id = meta.id;
        tab.blocks.push(Block::Command(CommandBlock {
            meta,
            command: command.to_string(),
            cwd: cwd.clone(),
            output_text: String::new(),
            exit_code: None,
            duration: None,
            ai_diagnosis: None,
        }));
        tab.scroll_handle.scroll_to_bottom();
        cx.notify();

        let (tx, rx) = smol::channel::unbounded::<ShellEvent>();
        let cmd = command.to_string();

        // Background thread: spawn process and stream output line-by-line
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let child = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("{} 2>&1", cmd))
                .current_dir(&cwd)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn();

            match child {
                Ok(mut proc) => {
                    if let Some(stdout) = proc.stdout.take() {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines().map_while(Result::ok) {
                            if tx.send_blocking(ShellEvent::Line(line)).is_err() {
                                break;
                            }
                        }
                    }
                    let status = proc.wait().ok().and_then(|s| s.code());
                    let _ = tx.send_blocking(ShellEvent::Exit(status, start.elapsed()));
                }
                Err(e) => {
                    let _ = tx.send_blocking(ShellEvent::Error(e.to_string()));
                }
            }
        });

        // Receive lines and update UI progressively
        cx.spawn(async move |entity, cx| {
            while let Ok(event) = rx.recv().await {
                let is_final = !matches!(event, ShellEvent::Line(_));
                let _ = cx.update(|cx| {
                    if let Some(entity) = entity.upgrade() {
                        entity.update(cx, |this, cx| {
                            let tab = this.tab_mut();
                            if let Some(block) =
                                tab.blocks.iter_mut().rev().find(|b| b.id() == block_id)
                            {
                                if let Block::Command(cmd_block) = block {
                                    match &event {
                                        ShellEvent::Line(line) => {
                                            if !cmd_block.output_text.is_empty() {
                                                cmd_block.output_text.push('\n');
                                            }
                                            cmd_block.output_text.push_str(line);
                                        }
                                        ShellEvent::Exit(code, elapsed) => {
                                            cmd_block.exit_code = *code;
                                            cmd_block.duration = Some(*elapsed);
                                        }
                                        ShellEvent::Error(e) => {
                                            cmd_block.output_text = format!("exec error: {}", e);
                                            cmd_block.exit_code = Some(-1);
                                        }
                                    }
                                }
                            }
                            tab.scroll_handle.scroll_to_bottom();
                            if is_final {
                                this.save_session();
                                // C9: Auto-diagnosis on failure
                                this.maybe_auto_diagnose(block_id, cx);
                            }
                            cx.notify();
                        });
                    }
                });
                if is_final {
                    break;
                }
            }
        })
        .detach();
    }

    // ─── C9: Auto-diagnose failed commands ───

    fn maybe_auto_diagnose(&mut self, block_id: BlockId, cx: &mut Context<Self>) {
        let tab = self.tab();
        let (exit_code, output, command) = {
            let Some(block) = tab.blocks.iter().rev().find(|b| b.id() == block_id) else {
                return;
            };
            let Block::Command(cmd) = block else {
                return;
            };
            (cmd.exit_code, cmd.output_text.clone(), cmd.command.clone())
        };

        if exit_code == Some(0) || exit_code.is_none() {
            return;
        }

        let model_str = resolve_ai_model();
        if model_str.is_empty() {
            return;
        }

        // Create a hint block for the diagnosis
        let tab = self.tab_mut();
        let meta = tab.new_meta();
        let diag_block_id = meta.id;
        tab.blocks.push(Block::AI(AIBlock {
            meta,
            model: model_str.clone(),
            role: Role::Assistant,
            content: String::new(),
            streaming: true,
            token_count: None,
            context_refs: vec![ContextRef::Block(block_id)],
            executable_snippets: vec![],
        }));
        tab.scroll_handle.scroll_to_bottom();
        cx.notify();

        let last_lines: String = output
            .lines()
            .rev()
            .take(30)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");

        let (tx, rx) = smol::channel::unbounded::<AiStreamEvent>();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(_) => return,
            };
            rt.block_on(async move {
                let client = match aineer_api::ProviderClient::from_model(&model_str) {
                    Ok(c) => c,
                    Err(_) => return,
                };
                let prompt = format!(
                    "The command `{}` failed with exit code {}. Here is the output:\n```\n{}\n```\nBriefly explain the error and suggest a fix in 2-3 sentences.",
                    command,
                    exit_code.unwrap_or(-1),
                    last_lines
                );
                let request = aineer_api::MessageRequest {
                    model: model_str.clone(),
                    max_tokens: 512,
                    messages: vec![aineer_api::InputMessage::user_text(&prompt)],
                    system: None,
                    tools: None,
                    tool_choice: None,
                    stream: true,
                    thinking: None,
                    gemini_cached_content: None,
                };
                match client.stream_message(&request).await {
                    Ok(mut stream) => {
                        loop {
                            match stream.next_event().await {
                                Ok(Some(event)) => {
                                    if let Some(text) = extract_stream_text(&event) {
                                        if tx.send(AiStreamEvent::Delta(text)).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                                Ok(None) => break,
                                Err(_) => break,
                            }
                        }
                        let _ = tx
                            .send(AiStreamEvent::Done {
                                model: model_str,
                                tokens: None,
                            })
                            .await;
                    }
                    Err(_) => {}
                }
            });
        });

        cx.spawn(async move |entity, cx| {
            while let Ok(event) = rx.recv().await {
                let done = matches!(event, AiStreamEvent::Done { .. } | AiStreamEvent::Error(_));
                let _ = cx.update(|cx| {
                    if let Some(entity) = entity.upgrade() {
                        entity.update(cx, |this, cx| {
                            let tab = this.tab_mut();
                            if let Some(block) = tab
                                .blocks
                                .iter_mut()
                                .rev()
                                .find(|b| b.id() == diag_block_id)
                            {
                                if let Block::AI(ai) = block {
                                    match &event {
                                        AiStreamEvent::Delta(text) => ai.content.push_str(text),
                                        AiStreamEvent::Done { model, tokens } => {
                                            ai.streaming = false;
                                            ai.model = model.clone();
                                            ai.token_count = *tokens;
                                        }
                                        AiStreamEvent::Error(_) => {
                                            ai.streaming = false;
                                        }
                                    }
                                }
                            }
                            tab.scroll_handle.scroll_to_bottom();
                            cx.notify();
                        });
                    }
                });
                if done {
                    break;
                }
            }
        })
        .detach();
    }

    fn handle_cd(&mut self, dir: &str, cwd: &PathBuf, cx: &mut Context<Self>) {
        let target = if dir.is_empty() || dir == "~" {
            dirs_or_home()
        } else if dir.starts_with('/') {
            PathBuf::from(dir)
        } else if dir.starts_with("~/") {
            dirs_or_home().join(&dir[2..])
        } else {
            cwd.join(dir)
        };

        match std::env::set_current_dir(&target) {
            Ok(()) => {
                let tab = self.tab_mut();
                tab.cwd = target.clone();
                tab.file_tree = load_directory(&target, 0);
                tab.cached_git_branch = read_git_branch(&target);
                let meta = tab.new_meta();
                tab.blocks.push(Block::System(SystemBlock {
                    meta,
                    kind: SystemKind::DirChange,
                    message: format!("→ {}", target.display()),
                }));
            }
            Err(e) => {
                let tab = self.tab_mut();
                let meta = tab.new_meta();
                tab.blocks.push(Block::System(SystemBlock {
                    meta,
                    kind: SystemKind::Error,
                    message: format!("cd: {}: {}", dir, e),
                }));
            }
        }
        self.tab_mut().scroll_handle.scroll_to_bottom();
        self.save_session();
        cx.notify();
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Render — Root
// ═══════════════════════════════════════════════════════════════════

impl Render for AineerWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.needs_initial_focus {
            self.focus_handle.focus(window);
            self.needs_initial_focus = false;
        }

        let entity = cx.entity().clone();
        let focus = self.focus_handle.clone();

        div()
            .id("workspace-root")
            .key_context("Workspace")
            .size_full()
            .bg(rgb(Clr::BG))
            .text_color(rgb(Clr::TEXT))
            .font_family("Berkeley Mono, JetBrains Mono, Menlo, monospace")
            .flex()
            .flex_row()
            .on_action(cx.listener(Self::on_toggle_sidebar))
            .on_action(cx.listener(Self::on_new_tab))
            .on_action(cx.listener(Self::on_close_tab))
            .on_action(cx.listener(Self::on_shell_mode))
            .on_action(cx.listener(Self::on_ai_mode))
            .on_action(cx.listener(Self::on_agent_mode))
            .on_action(cx.listener(Self::on_focus_input))
            .on_action(cx.listener(Self::on_clear_blocks))
            .child(self.render_activity_bar(cx))
            .child(self.render_main(window, cx))
            .when(self.sidebar_visible, |el| el.child(self.render_sidebar(cx)))
            .when(self.tab().command_palette_open, |el| {
                el.child(self.render_command_palette(cx))
            })
            .when(!self.tab().toasts.is_empty(), |el| {
                let toasts = self.tab().toasts.clone();
                el.child(
                    div()
                        .absolute()
                        .top(px(8.0))
                        .right(px(8.0))
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .children(toasts.iter().map(|toast| {
                            let (bg, border) = match toast.kind {
                                ToastKind::Success => (0x1a3d1a_u32, Clr::SUCCESS),
                                ToastKind::Error => (0x3d1a1a_u32, Clr::ERROR),
                                ToastKind::Info => (0x1a2a3d_u32, Clr::ACCENT),
                            };
                            let toast_id = toast.id;
                            div()
                                .id(SharedString::from(format!("toast-{}", toast_id)))
                                .px(px(12.0))
                                .py(px(8.0))
                                .min_w(px(200.0))
                                .max_w(px(360.0))
                                .bg(rgb(bg))
                                .rounded(px(6.0))
                                .border_l_2()
                                .border_color(rgb(border))
                                .shadow_lg()
                                .text_size(px(12.0))
                                .text_color(rgb(Clr::TEXT))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.tab_mut().toasts.retain(|t| t.id != toast_id);
                                    cx.notify();
                                }))
                                .child(toast.message.clone())
                        })),
                )
            })
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Render — Activity Bar
// ═══════════════════════════════════════════════════════════════════

impl AineerWorkspace {
    fn render_activity_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("bar-activity")
            .w(px(44.0))
            .h_full()
            .bg(rgb(Clr::BAR))
            .border_r_1()
            .border_color(rgb(Clr::BORDER))
            .flex()
            .flex_col()
            .items_center()
            .pt(px(8.0))
            .gap(px(2.0))
            .children(ALL_PANELS.iter().map(|&panel| {
                let active = panel == self.active_panel && self.sidebar_visible;
                div()
                    .id(SharedString::from(format!("p-{}", panel.icon())))
                    .w(px(36.0))
                    .h(px(32.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(14.0))
                    .text_color(if active {
                        rgb(Clr::TEXT)
                    } else {
                        rgb(Clr::MUTED)
                    })
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(Clr::HOVER)).text_color(rgb(Clr::TEXT)))
                    .when(active, |el| {
                        el.bg(rgb(Clr::HOVER))
                            .border_l_2()
                            .border_color(rgb(Clr::ACCENT))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.active_panel == panel && this.sidebar_visible {
                            this.sidebar_visible = false;
                        } else {
                            this.active_panel = panel;
                            this.sidebar_visible = true;
                        }
                        cx.notify();
                    }))
                    .child(panel.icon())
            }))
            .child(div().flex_1())
            .child(
                div()
                    .id("btn-settings")
                    .w(px(36.0))
                    .h(px(32.0))
                    .mb(px(8.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(14.0))
                    .text_color(
                        if self.active_panel == SidebarPanel::Settings && self.sidebar_visible {
                            rgb(Clr::TEXT)
                        } else {
                            rgb(Clr::MUTED)
                        },
                    )
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(Clr::HOVER)).text_color(rgb(Clr::TEXT)))
                    .when(
                        self.active_panel == SidebarPanel::Settings && self.sidebar_visible,
                        |el| {
                            el.bg(rgb(Clr::HOVER))
                                .border_l_2()
                                .border_color(rgb(Clr::ACCENT))
                        },
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.active_panel == SidebarPanel::Settings && this.sidebar_visible {
                            this.sidebar_visible = false;
                        } else {
                            this.active_panel = SidebarPanel::Settings;
                            this.sidebar_visible = true;
                        }
                        cx.notify();
                    }))
                    .child("⚙"),
            )
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Render — Main Area
// ═══════════════════════════════════════════════════════════════════

impl AineerWorkspace {
    fn render_main(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("main-area")
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .child(self.render_tab_bar(cx))
            .child(self.render_content(cx))
            .child(self.render_input_bar(window, cx))
            .child(self.render_status_bar())
    }

    // ─── Tab Bar ───
    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("tab-bar")
            .w_full()
            .h(px(34.0))
            .bg(rgb(Clr::BG))
            .border_b_1()
            .border_color(rgb(Clr::BORDER))
            .flex()
            .flex_row()
            .items_center()
            .px(px(4.0))
            .children(self.tabs.iter().enumerate().map(|(i, tab)| {
                let active = i == self.active_tab;
                let tab_i = i;
                let tab_id = tab.id;
                div()
                    .id(SharedString::from(format!("tab-{}", tab_id)))
                    .px(px(12.0))
                    .py(px(6.0))
                    .text_size(px(12.0))
                    .text_color(if active {
                        rgb(Clr::TEXT)
                    } else {
                        rgb(Clr::TEXT2)
                    })
                    .rounded_t(px(3.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(Clr::ELEVATED)))
                    .when(active, |el| {
                        el.bg(rgb(Clr::SURFACE))
                            .border_b_2()
                            .border_color(rgb(Clr::ACCENT))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.active_tab = tab_i;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(tab.title.clone())
                            .when(active && self.tabs.len() > 1, |el| {
                                el.child(
                                    div()
                                        .id(SharedString::from(format!("x-{}", tab_id)))
                                        .text_size(px(10.0))
                                        .text_color(rgb(Clr::MUTED))
                                        .w(px(14.0))
                                        .h(px(14.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(2.0))
                                        .cursor_pointer()
                                        .hover(|s| {
                                            s.bg(rgb(Clr::BORDER)).text_color(rgb(Clr::TEXT))
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if this.tabs.len() > 1 {
                                                this.tabs.remove(tab_i);
                                                if this.active_tab >= this.tabs.len() {
                                                    this.active_tab = this.tabs.len() - 1;
                                                }
                                                cx.notify();
                                            }
                                        }))
                                        .child("×"),
                                )
                            }),
                    )
            }))
            .child(
                div()
                    .id("btn-new-tab")
                    .px(px(8.0))
                    .py(px(6.0))
                    .text_size(px(14.0))
                    .text_color(rgb(Clr::MUTED))
                    .rounded(px(3.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(Clr::ELEVATED)).text_color(rgb(Clr::TEXT)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let id = this.next_tab_id;
                        this.next_tab_id += 1;
                        let cwd = this.tab().cwd.clone();
                        this.tabs
                            .push(TabState::new(id, format!("Terminal {}", id), cwd));
                        this.active_tab = this.tabs.len() - 1;
                        cx.notify();
                    }))
                    .child("+"),
            )
    }

    // ─── Content Area ───
    fn render_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.tab();

        // File preview mode
        if let Some(ref path) = tab.previewing_file {
            return self
                .render_file_preview(path.clone(), cx)
                .into_any_element();
        }

        if tab.blocks.is_empty() {
            return self.render_welcome(cx).into_any_element();
        }

        // Block stream with optional search bar
        div()
            .flex_1()
            .w_full()
            .flex()
            .flex_col()
            .when(tab.search_active, |el| {
                let query = &tab.search_query;
                let match_count = if query.len() >= 2 {
                    tab.blocks
                        .iter()
                        .filter(|b| block_text(b).to_lowercase().contains(&query.to_lowercase()))
                        .count()
                } else {
                    0
                };
                el.child(
                    div()
                        .w_full()
                        .h(px(32.0))
                        .bg(rgb(Clr::SURFACE))
                        .border_b_1()
                        .border_color(rgb(Clr::BORDER))
                        .flex()
                        .flex_row()
                        .items_center()
                        .px(px(10.0))
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(Clr::MUTED))
                                .child("🔍"),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(12.0))
                                .text_color(if query.is_empty() {
                                    rgb(Clr::MUTED)
                                } else {
                                    rgb(Clr::TEXT)
                                })
                                .child(if query.is_empty() {
                                    "Search blocks...".to_string()
                                } else {
                                    query.clone()
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(Clr::MUTED))
                                .child(format!("{} matches", match_count)),
                        ),
                )
            })
            .child(self.render_block_stream(cx))
            .into_any_element()
    }

    fn render_file_preview(&self, path: PathBuf, cx: &mut Context<Self>) -> impl IntoElement {
        let display_path = short_path(&path);
        let content = self
            .tab()
            .file_preview_content
            .as_deref()
            .unwrap_or("Loading...");

        div()
            .id("file-preview")
            .flex_1()
            .w_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .h(px(32.0))
                    .bg(rgb(Clr::SURFACE))
                    .border_b_1()
                    .border_color(rgb(Clr::BORDER))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(10.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(Clr::TEXT2))
                            .child(display_path),
                    )
                    .child(
                        div()
                            .id("btn-close-preview")
                            .px(px(6.0))
                            .py(px(2.0))
                            .text_size(px(11.0))
                            .text_color(rgb(Clr::MUTED))
                            .rounded(px(3.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(Clr::HOVER)).text_color(rgb(Clr::TEXT)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.tab_mut().previewing_file = None;
                                this.tab_mut().file_preview_content = None;
                                cx.notify();
                            }))
                            .child("✕ Close"),
                    ),
            )
            .child(
                div()
                    .id("file-content")
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .bg(rgb(Clr::BG))
                    .p(px(12.0))
                    .flex()
                    .flex_col()
                    .children(content.lines().enumerate().map(|(i, line)| {
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .w(px(36.0))
                                    .text_size(px(11.0))
                                    .text_color(rgb(Clr::MUTED))
                                    .child(format!("{}", i + 1)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.0))
                                    .text_color(rgb(Clr::TEXT2))
                                    .child(line.to_string()),
                            )
                    })),
            )
    }

    fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.tab();
        let git_info = tab
            .cached_git_branch
            .as_ref()
            .map(|b| format!("on branch: {}", b))
            .unwrap_or_default();

        div()
            .id("content-welcome")
            .flex_1()
            .w_full()
            .bg(rgb(Clr::BG))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.0))
                    .child({
                        static LOGO_PNG: &[u8] = include_bytes!("../../../assets/icon-256.png");
                        let image = Image::from_bytes(ImageFormat::Png, LOGO_PNG.to_vec());
                        img(ImageSource::Image(Arc::new(image)))
                            .w(px(64.0))
                            .h(px(64.0))
                    })
                    .child(
                        div()
                            .text_size(px(28.0))
                            .text_color(rgb(Clr::ACCENT))
                            .child("AINEER"),
                    ),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(Clr::TEXT2))
                    .child("The Agentic Development Environment"),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(Clr::MUTED))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(2.0))
                    .child(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .when(!git_info.is_empty(), |el| {
                        el.child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(Clr::MUTED))
                                .child(git_info),
                        )
                    }),
            )
            .child(
                div()
                    .mt(px(20.0))
                    .flex()
                    .flex_row()
                    .gap(px(10.0))
                    .child(self.render_card("⌨", "Shell", "Type a command", cx))
                    .child(self.render_card("✦", "AI Chat", "Ask anything", cx))
                    .child(self.render_card("⚡", "Agent", "Automate a task", cx)),
            )
            .child(
                div()
                    .mt(px(16.0))
                    .flex()
                    .flex_row()
                    .gap(px(14.0))
                    .text_size(px(11.0))
                    .text_color(rgb(Clr::MUTED))
                    .child("↑↓ History")
                    .child("⌘B Sidebar")
                    .child("⌘T New Tab")
                    .child("Ctrl+L Clear")
                    .child("Ctrl+A/E Home/End"),
            )
    }

    fn render_card(
        &self,
        icon: &str,
        title: &str,
        sub: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mode = match title {
            "AI Chat" => Some(InputMode::AIChat),
            "Agent" => Some(InputMode::Agent),
            _ => Some(InputMode::Shell),
        };

        div()
            .id(SharedString::from(format!("card-{}", title)))
            .w(px(150.0))
            .px(px(14.0))
            .py(px(12.0))
            .bg(rgb(Clr::SURFACE))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(Clr::BORDER))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(Clr::ELEVATED)).border_color(rgb(Clr::ACCENT)))
            .flex()
            .flex_col()
            .gap(px(3.0))
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(m) = mode {
                    this.tab_mut().input_mode = m;
                }
                cx.notify();
            }))
            .child(
                div()
                    .text_size(px(18.0))
                    .text_color(rgb(Clr::ACCENT))
                    .child(icon.to_string()),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(Clr::TEXT))
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(Clr::MUTED))
                    .child(sub.to_string()),
            )
    }

    fn render_block_stream(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.tab();
        let search_active = tab.search_active;
        let search_query = tab.search_query.clone();
        div()
            .id("content-blocks")
            .flex_1()
            .w_full()
            .bg(rgb(Clr::BG))
            .overflow_y_scroll()
            .track_scroll(&tab.scroll_handle)
            .flex()
            .flex_col()
            .pb(px(8.0))
            .children(tab.blocks.iter().enumerate().map(|(i, block)| {
                let block_content = block_text(block);
                let is_match = search_active
                    && search_query.len() >= 2
                    && block_content
                        .to_lowercase()
                        .contains(&search_query.to_lowercase());

                let block_el = render_block(i, block);
                let is_pending_tool =
                    matches!(block, Block::Tool(t) if matches!(t.state, ToolState::Pending));

                div()
                    .id(SharedString::from(format!("blk-wrap-{}", i)))
                    .relative()
                    .group("block-hover")
                    .when(is_match, |el| {
                        el.bg(rgba(0x4488ff18_u32))
                            .border_l_2()
                            .border_color(rgb(Clr::ACCENT))
                    })
                    .child(block_el)
                    .child(
                        div()
                            .absolute()
                            .top(px(2.0))
                            .right(px(10.0))
                            .flex()
                            .flex_row()
                            .gap(px(2.0))
                            .invisible()
                            .group_hover("block-hover", |s| s.visible())
                            .child(self.render_block_chrome_btn("📋", "Copy", i, cx))
                            .child(self.render_block_chrome_btn(
                                if block.meta().collapsed { "▸" } else { "▾" },
                                "Toggle",
                                i,
                                cx,
                            )),
                    )
                    .when(is_pending_tool, |el| {
                        el.child(self.render_tool_approval_buttons(i, cx))
                    })
            }))
    }

    fn render_tool_approval_buttons(
        &self,
        block_idx: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .px(px(16.0))
            .py(px(4.0))
            .flex()
            .flex_row()
            .gap(px(6.0))
            .child(
                div()
                    .id(SharedString::from(format!("tool-allow-{}", block_idx)))
                    .px(px(12.0))
                    .py(px(3.0))
                    .bg(rgb(Clr::SUCCESS))
                    .text_color(rgb(Clr::BG))
                    .text_size(px(11.0))
                    .rounded(px(3.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x40a040)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(Block::Tool(tool)) = this.tab_mut().blocks.get_mut(block_idx) {
                            tool.state = ToolState::Running;
                        }
                        cx.notify();
                    }))
                    .child("✓ Allow"),
            )
            .child(
                div()
                    .id(SharedString::from(format!("tool-deny-{}", block_idx)))
                    .px(px(12.0))
                    .py(px(3.0))
                    .bg(rgb(Clr::ERROR))
                    .text_color(rgb(0xffffff_u32))
                    .text_size(px(11.0))
                    .rounded(px(3.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0xd03030)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(Block::Tool(tool)) = this.tab_mut().blocks.get_mut(block_idx) {
                            tool.state = ToolState::Denied;
                        }
                        cx.notify();
                    }))
                    .child("✗ Deny"),
            )
    }

    fn render_block_chrome_btn(
        &self,
        icon: &str,
        action: &str,
        block_idx: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let action_name = action.to_string();
        let icon_owned = icon.to_string();
        div()
            .id(SharedString::from(format!(
                "chrome-{}-{}",
                action, block_idx
            )))
            .px(px(4.0))
            .py(px(1.0))
            .text_size(px(10.0))
            .text_color(rgb(Clr::MUTED))
            .bg(rgb(Clr::SURFACE))
            .rounded(px(3.0))
            .border_1()
            .border_color(rgb(Clr::BORDER))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(Clr::HOVER)).text_color(rgb(Clr::TEXT)))
            .on_click(cx.listener(move |this, _, _, cx| {
                match action_name.as_str() {
                    "Copy" => {
                        if let Some(block) = this.tab().blocks.get(block_idx) {
                            let text = block_text(block);
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                            let id = this.tab().next_block_id;
                            this.tab_mut().toasts.push(Toast {
                                id,
                                kind: ToastKind::Info,
                                message: "Copied to clipboard".into(),
                            });
                        }
                    }
                    "Toggle" => {
                        if let Some(block) = this.tab_mut().blocks.get_mut(block_idx) {
                            let collapsed = !block.meta().collapsed;
                            match block {
                                Block::Command(b) => b.meta.collapsed = collapsed,
                                Block::AI(b) => b.meta.collapsed = collapsed,
                                Block::Tool(b) => b.meta.collapsed = collapsed,
                                Block::System(b) => b.meta.collapsed = collapsed,
                                Block::Diff(b) => b.meta.collapsed = collapsed,
                                Block::AgentPlan(b) => b.meta.collapsed = collapsed,
                            }
                        }
                    }
                    _ => {}
                }
                cx.notify();
            }))
            .child(icon_owned)
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Render — Blocks
// ═══════════════════════════════════════════════════════════════════

fn render_block(idx: usize, block: &Block) -> impl IntoElement {
    let el_id = SharedString::from(format!("blk-{}", idx));
    let meta = block.meta();
    let timestamp = meta.created_at.format("%H:%M:%S").to_string();

    match block {
        Block::Command(cmd) => {
            let cwd_display = short_path(&cmd.cwd);
            let is_running = cmd.exit_code.is_none() && cmd.output_text.is_empty();
            let exit_color = match cmd.exit_code {
                Some(0) | None => Clr::TEXT2,
                _ => Clr::ERROR,
            };
            let exit_badge = match cmd.exit_code {
                Some(0) => None,
                Some(code) => Some(format!("exit {}", code)),
                None if !is_running => Some("exit ?".into()),
                _ => None,
            };
            let duration_str = cmd.duration.map(|d| {
                if d.as_millis() < 1000 {
                    format!("{}ms", d.as_millis())
                } else {
                    format!("{:.1}s", d.as_secs_f64())
                }
            });

            div()
                .id(el_id)
                .w_full()
                .px(px(16.0))
                .pt(px(10.0))
                .pb(px(6.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                // Command line with cwd + timestamp
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_baseline()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(Clr::MUTED))
                                .child(cwd_display),
                        )
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(rgb(Clr::ACCENT))
                                .child(format!("❯ {}", cmd.command)),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(Clr::MUTED))
                                .child(timestamp),
                        ),
                )
                // Output
                .when(is_running, |el| {
                    el.child(
                        div()
                            .px(px(12.0))
                            .py(px(4.0))
                            .text_size(px(11.0))
                            .text_color(rgb(Clr::MUTED))
                            .child("⏳ Running..."),
                    )
                })
                .when(!cmd.output_text.is_empty(), |el| {
                    let collapsed = meta.collapsed && cmd.output_text.lines().count() > 5;
                    let display_text = if collapsed {
                        let first_lines: String = cmd
                            .output_text
                            .lines()
                            .take(5)
                            .collect::<Vec<_>>()
                            .join("\n");
                        let total = cmd.output_text.lines().count();
                        format!("{}\n... ({} more lines)", first_lines, total - 5)
                    } else {
                        cmd.output_text.clone()
                    };
                    el.child(
                        div()
                            .w_full()
                            .px(px(12.0))
                            .py(px(6.0))
                            .bg(rgb(Clr::SURFACE))
                            .rounded(px(4.0))
                            .text_size(px(12.0))
                            .text_color(rgb(exit_color))
                            .child(display_text),
                    )
                })
                // Exit badge + duration
                .when(exit_badge.is_some() || duration_str.is_some(), |el| {
                    el.child(
                        div()
                            .px(px(12.0))
                            .pt(px(2.0))
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .text_size(px(10.0))
                            .when(exit_badge.is_some(), |el| {
                                el.child(
                                    div()
                                        .text_color(rgb(Clr::ERROR))
                                        .child(exit_badge.unwrap_or_default()),
                                )
                            })
                            .when(duration_str.is_some(), |el| {
                                el.child(
                                    div()
                                        .text_color(rgb(Clr::MUTED))
                                        .child(duration_str.unwrap_or_default()),
                                )
                            }),
                    )
                })
        }

        Block::AI(ai) => {
            let is_user = matches!(ai.role, Role::User);
            if is_user {
                div()
                    .id(el_id)
                    .w_full()
                    .px(px(16.0))
                    .pt(px(10.0))
                    .pb(px(2.0))
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(Clr::AI))
                            .child("AI"),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(rgb(Clr::ACCENT))
                            .child(format!("❯ {}", ai.content)),
                    )
            } else {
                div()
                    .id(el_id)
                    .w_full()
                    .px(px(16.0))
                    .py(px(6.0))
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(20.0))
                            .h(px(20.0))
                            .rounded(px(10.0))
                            .bg(rgb(Clr::AI))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(10.0))
                            .text_color(rgb(0xffffff))
                            .child("AI"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(rgb(Clr::AI))
                                            .child(ai.model.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(rgb(Clr::MUTED))
                                            .child(timestamp),
                                    ),
                            )
                            .child(
                                div()
                                    .px(px(10.0))
                                    .py(px(6.0))
                                    .bg(rgb(Clr::SURFACE))
                                    .rounded(px(4.0))
                                    .text_size(px(13.0))
                                    .text_color(rgb(Clr::TEXT))
                                    .child(ai.content.clone()),
                            ),
                    )
            }
        }

        Block::Tool(tool) => {
            let state_label = match &tool.state {
                ToolState::Pending => "⏳ Pending approval",
                ToolState::Running => "⚙ Running...",
                ToolState::Completed { is_error, .. } => {
                    if *is_error {
                        "✗ Failed"
                    } else {
                        "✓ Completed"
                    }
                }
                ToolState::Denied => "⛔ Denied",
            };
            let is_pending = matches!(tool.state, ToolState::Pending);
            div()
                .id(el_id.clone())
                .w_full()
                .px(px(16.0))
                .py(px(4.0))
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(6.0))
                        .bg(rgb(Clr::SURFACE))
                        .rounded(px(4.0))
                        .border_l_2()
                        .border_color(rgb(if is_pending { Clr::AGENT } else { Clr::SURFACE }))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(rgb(Clr::AGENT))
                                        .child(format!("🔧 {}", tool.name)),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(rgb(Clr::MUTED))
                                        .child(state_label),
                                ),
                        )
                        .child(
                            div()
                                .px(px(8.0))
                                .py(px(4.0))
                                .bg(rgb(Clr::ELEVATED))
                                .rounded(px(3.0))
                                .text_size(px(11.0))
                                .text_color(rgb(Clr::TEXT2))
                                .child(tool.input.clone()),
                        )
                        .when(is_pending, |el| {
                            el.child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("{}-allow", el_id)))
                                            .px(px(12.0))
                                            .py(px(3.0))
                                            .bg(rgb(Clr::SUCCESS))
                                            .text_color(rgb(Clr::BG))
                                            .text_size(px(11.0))
                                            .rounded(px(3.0))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgb(0x40a040)))
                                            .child("✓ Allow"),
                                    )
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("{}-deny", el_id)))
                                            .px(px(12.0))
                                            .py(px(3.0))
                                            .bg(rgb(Clr::ERROR))
                                            .text_color(rgb(Clr::BG))
                                            .text_size(px(11.0))
                                            .rounded(px(3.0))
                                            .cursor_pointer()
                                            .hover(|s| s.bg(rgb(0xd03030)))
                                            .child("✗ Deny"),
                                    ),
                            )
                        })
                        .when(matches!(&tool.state, ToolState::Completed { .. }), |el| {
                            if let ToolState::Completed { output, .. } = &tool.state {
                                el.child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(rgb(Clr::TEXT2))
                                        .child(output.clone()),
                                )
                            } else {
                                el
                            }
                        }),
                )
        }

        Block::System(sys) => {
            let (icon, color) = match sys.kind {
                SystemKind::DirChange => ("→", Clr::MUTED),
                SystemKind::Error => ("✗", Clr::ERROR),
                SystemKind::Info => ("ℹ", Clr::MUTED),
                SystemKind::Welcome => ("◆", Clr::ACCENT),
                SystemKind::ProactiveHint => ("💡", Clr::AI),
            };
            let is_error = matches!(sys.kind, SystemKind::Error);

            div()
                .id(el_id)
                .w_full()
                .px(px(16.0))
                .py(px(4.0))
                .when(!is_error, |el| {
                    el.child(
                        div().flex().justify_center().child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(color))
                                .child(format!("— {} {} —", icon, sys.message)),
                        ),
                    )
                })
                .when(is_error, |el| {
                    el.child(
                        div()
                            .px(px(10.0))
                            .py(px(4.0))
                            .bg(rgb(0x3d1a1a))
                            .rounded(px(4.0))
                            .text_size(px(12.0))
                            .text_color(rgb(Clr::ERROR))
                            .child(format!("✗ {}", sys.message)),
                    )
                })
        }

        Block::Diff(diff) => div().id(el_id).w_full().px(px(16.0)).py(px(6.0)).child(
            div()
                .w_full()
                .bg(rgb(Clr::SURFACE))
                .rounded(px(4.0))
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(4.0))
                        .border_b_1()
                        .border_color(rgb(Clr::BORDER))
                        .flex()
                        .flex_row()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(rgb(Clr::TEXT))
                                .child(diff.file_path.clone()),
                        )
                        .child(div().text_size(px(10.0)).text_color(rgb(Clr::MUTED)).child(
                            format!("+{} -{}", diff.stats.additions, diff.stats.deletions),
                        )),
                )
                .children(diff.hunks.iter().map(|hunk| {
                    let header = format!(
                        "@@ -{},{} +{},{} @@",
                        hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
                    );
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .px(px(10.0))
                                .py(px(2.0))
                                .bg(rgb(Clr::ELEVATED))
                                .text_size(px(10.0))
                                .text_color(rgb(Clr::MUTED))
                                .child(header),
                        )
                        .children(hunk.lines.iter().map(render_diff_line))
                })),
        ),

        Block::AgentPlan(plan) => {
            let state_label = match plan.state {
                AgentPlanState::Planning => "📋 Planning...",
                AgentPlanState::Executing => "⚙ Executing...",
                AgentPlanState::AwaitApproval => "⏳ Awaiting approval",
                AgentPlanState::Completed => "✓ Completed",
                AgentPlanState::Failed => "✗ Failed",
                AgentPlanState::Cancelled => "⛔ Cancelled",
            };
            div().id(el_id).w_full().px(px(16.0)).py(px(6.0)).child(
                div()
                    .px(px(10.0))
                    .py(px(8.0))
                    .bg(rgb(Clr::SURFACE))
                    .rounded(px(4.0))
                    .border_l_2()
                    .border_color(rgb(Clr::AGENT))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgb(Clr::AGENT))
                                    .child(format!("Agent: {}", plan.goal)),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(Clr::MUTED))
                                    .child(state_label),
                            ),
                    )
                    .children(plan.steps.iter().map(|step| {
                        let (icon, clr) = match step.state {
                            StepState::Pending => ("○", Clr::MUTED),
                            StepState::Running => ("◉", Clr::ACCENT),
                            StepState::Completed => ("✓", Clr::SUCCESS),
                            StepState::Failed => ("✗", Clr::ERROR),
                            StepState::NeedsApproval => ("⏳", Clr::AGENT),
                        };
                        div()
                            .pl(px(8.0))
                            .flex()
                            .flex_row()
                            .gap(px(6.0))
                            .child(div().text_size(px(12.0)).text_color(rgb(clr)).child(icon))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(Clr::TEXT2))
                                    .child(step.description.clone()),
                            )
                    })),
            )
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Render — Input Bar
// ═══════════════════════════════════════════════════════════════════

impl AineerWorkspace {
    fn render_input_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.tab();
        let focused = self.focus_handle.is_focused(window);
        let mode_color = match tab.input_mode {
            InputMode::Shell => rgb(Clr::TEXT2),
            InputMode::AIChat => rgb(Clr::AI),
            InputMode::Agent => rgb(Clr::AGENT),
        };
        let border_clr = if focused {
            mode_color
        } else {
            rgb(Clr::BORDER)
        };

        let (before_cursor, after_cursor) = tab
            .input_text
            .split_at(tab.cursor_pos.min(tab.input_text.len()));
        let before = before_cursor.to_string();
        let after = after_cursor.to_string();
        let is_empty = tab.input_text.is_empty();
        let input_mode = tab.input_mode;

        let line_count = tab.input_text.chars().filter(|&c| c == '\n').count() + 1;
        let input_height = (line_count.min(8).max(1) as f32) * 20.0 + 12.0;

        let ime_entity = cx.entity().clone();
        let ime_focus = self.focus_handle.clone();

        div()
            .id("input-bar")
            .key_context("InputBar")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .w_full()
            .min_h(px(input_height.max(42.0)))
            .bg(rgb(Clr::SURFACE))
            .border_t_1()
            .border_color(border_clr)
            .flex()
            .flex_row()
            .items_start()
            .px(px(10.0))
            .py(px(6.0))
            .gap(px(8.0))
            .cursor_text()
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, cx| {
                        window.handle_input(
                            &ime_focus,
                            ElementInputHandler::new(bounds, ime_entity),
                            cx,
                        );
                    },
                )
                .w(px(0.0))
                .h(px(0.0)),
            )
            .child(
                div()
                    .id("input-mode")
                    .px(px(7.0))
                    .py(px(2.0))
                    .text_size(px(11.0))
                    .text_color(mode_color)
                    .bg(rgb(Clr::ELEVATED))
                    .rounded(px(3.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(Clr::BORDER)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let next = this.tab().input_mode.next();
                        this.tab_mut().input_mode = next;
                        cx.notify();
                    }))
                    .child(format!("[{}]", input_mode.label())),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .overflow_x_hidden()
                    .when(is_empty, |el| {
                        let ph = match input_mode {
                            InputMode::Shell => "❯ Type a command...",
                            InputMode::AIChat => "Ask AI anything...",
                            InputMode::Agent => "Describe a task for Agent...",
                        };
                        el.child(
                            div()
                                .text_size(px(13.0))
                                .text_color(rgb(Clr::MUTED))
                                .child(ph),
                        )
                    })
                    .when(!is_empty || focused, |el| {
                        el.when(!is_empty, |el| {
                            el.child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgb(Clr::TEXT))
                                    .child(before),
                            )
                        })
                        .when(focused, |el| {
                            el.child(div().w(px(1.5)).h(px(16.0)).bg(mode_color))
                        })
                        .when(!after.is_empty(), |el| {
                            el.child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgb(Clr::TEXT))
                                    .child(after),
                            )
                        })
                    }),
            )
            .when(self.tab().ai_streaming, |el| {
                el.child(
                    div()
                        .id("btn-stop")
                        .px(px(8.0))
                        .py(px(2.0))
                        .bg(rgb(Clr::ERROR))
                        .text_color(rgb(0xffffff))
                        .text_size(px(10.0))
                        .rounded(px(3.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0xd03030)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.stop_ai_streaming(cx);
                        }))
                        .child("■ Stop"),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .text_size(px(10.0))
                    .text_color(rgb(Clr::MUTED))
                    .child(
                        div()
                            .id("hint-mode")
                            .px(px(5.0))
                            .py(px(1.0))
                            .rounded(px(2.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(Clr::ELEVATED)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let next = this.tab().input_mode.next();
                                this.tab_mut().input_mode = next;
                                cx.notify();
                            }))
                            .child(match input_mode {
                                InputMode::Shell => "⌘⏎ AI".to_string(),
                                InputMode::AIChat => "⌘⇧⏎ Agent".to_string(),
                                InputMode::Agent => "Esc Shell".to_string(),
                            }),
                    ),
            )
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Render — Status Bar
// ═══════════════════════════════════════════════════════════════════

impl AineerWorkspace {
    fn render_status_bar(&self) -> impl IntoElement {
        let tab = self.tab();
        let cwd_str = short_path(&tab.cwd);

        let (gw_color, gw_label) = if self.gateway_connected {
            (Clr::SUCCESS, "Gateway")
        } else {
            (Clr::MUTED, "Gateway Off")
        };

        let git_dirty = !read_git_status(&tab.cwd).is_empty();
        let branch_display: String = {
            let b = tab.cached_git_branch.as_deref().unwrap_or("—");
            if git_dirty {
                format!("{}*", b)
            } else {
                b.to_string()
            }
        };

        div()
            .id("status-bar")
            .w_full()
            .h(px(24.0))
            .bg(rgb(Clr::BAR))
            .border_t_1()
            .border_color(rgb(Clr::BORDER))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px(px(10.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(Clr::TEXT2))
                    .flex()
                    .flex_row()
                    .gap(px(10.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(3.0))
                            .child(
                                div()
                                    .w(px(6.0))
                                    .h(px(6.0))
                                    .rounded(px(3.0))
                                    .bg(rgb(gw_color)),
                            )
                            .child(gw_label),
                    )
                    .child(branch_display)
                    .child(cwd_str),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(Clr::TEXT2))
                    .flex()
                    .flex_row()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_color(match tab.input_mode {
                                InputMode::Shell => rgb(Clr::TEXT2),
                                InputMode::AIChat => rgb(Clr::AI),
                                InputMode::Agent => rgb(Clr::AGENT),
                            })
                            .child(tab.input_mode.label()),
                    )
                    .child(format!("{} blocks", tab.blocks.len())),
            )
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Render — Sidebar
// ═══════════════════════════════════════════════════════════════════

impl AineerWorkspace {
    fn execute_palette_command(&mut self, cx: &mut Context<Self>) {
        let query = self.tab().command_palette_query.to_lowercase();
        let commands = Self::palette_commands();
        let target = if query.is_empty() {
            commands.first().map(|(name, _)| *name)
        } else {
            commands
                .iter()
                .find(|(name, _)| name.to_lowercase().contains(&query))
                .map(|(name, _)| *name)
        };
        self.tab_mut().command_palette_open = false;
        self.tab_mut().command_palette_query.clear();

        if let Some(cmd) = target {
            match cmd {
                "Toggle Sidebar" => {
                    self.sidebar_visible = !self.sidebar_visible;
                }
                "Shell Mode" => {
                    self.tab_mut().input_mode = InputMode::Shell;
                }
                "AI Chat Mode" => {
                    self.tab_mut().input_mode = InputMode::AIChat;
                }
                "Agent Mode" => {
                    self.tab_mut().input_mode = InputMode::Agent;
                }
                "Clear Blocks" => {
                    self.tab_mut().blocks.clear();
                }
                "Search in Blocks" => {
                    self.tab_mut().search_active = true;
                }
                "Open Settings" => {
                    self.active_panel = SidebarPanel::Settings;
                    self.sidebar_visible = true;
                }
                "Open Explorer" => {
                    self.active_panel = SidebarPanel::Explorer;
                    self.sidebar_visible = true;
                }
                "Open Git Panel" => {
                    self.active_panel = SidebarPanel::Git;
                    self.sidebar_visible = true;
                }
                "Open Search Panel" => {
                    self.active_panel = SidebarPanel::Search;
                    self.sidebar_visible = true;
                }
                "Open Memory Panel" => {
                    self.active_panel = SidebarPanel::Memory;
                    self.sidebar_visible = true;
                }
                "Close File Preview" => {
                    self.tab_mut().previewing_file = None;
                }
                _ => {}
            }
        }
        cx.notify();
    }

    fn palette_commands() -> Vec<(&'static str, &'static str)> {
        vec![
            ("Toggle Sidebar", "⌘B"),
            ("New Tab", "⌘T"),
            ("Close Tab", "⌘W"),
            ("Shell Mode", ""),
            ("AI Chat Mode", "⌘⏎"),
            ("Agent Mode", "⌘⇧⏎"),
            ("Clear Blocks", "⌃L"),
            ("Search in Blocks", "⌘F"),
            ("Open Settings", ""),
            ("Open Explorer", ""),
            ("Open Git Panel", ""),
            ("Open Search Panel", ""),
            ("Open Memory Panel", ""),
            ("Close File Preview", ""),
        ]
    }

    fn render_command_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let commands = Self::palette_commands();
        let query = &self.tab().command_palette_query;
        let filtered: Vec<_> = if query.is_empty() {
            commands.clone()
        } else {
            let q = query.to_lowercase();
            commands
                .iter()
                .filter(|(name, _)| name.to_lowercase().contains(&q))
                .copied()
                .collect()
        };

        div()
            .id("command-palette-overlay")
            .absolute()
            .inset_0()
            .bg(rgba(0x00000088))
            .flex()
            .justify_center()
            .pt(px(80.0))
            .on_click(cx.listener(|this, _, _, cx| {
                this.tab_mut().command_palette_open = false;
                this.tab_mut().command_palette_query.clear();
                cx.notify();
            }))
            .child(
                div()
                    .w(px(400.0))
                    .max_h(px(320.0))
                    .bg(rgb(Clr::ELEVATED))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(rgb(Clr::ACCENT))
                    .shadow_lg()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(8.0))
                            .border_b_1()
                            .border_color(rgb(Clr::BORDER))
                            .text_size(px(13.0))
                            .text_color(if query.is_empty() {
                                rgb(Clr::MUTED)
                            } else {
                                rgb(Clr::TEXT)
                            })
                            .child(if query.is_empty() {
                                "Type a command...".to_string()
                            } else {
                                format!("> {}", query)
                            }),
                    )
                    .children(filtered.iter().enumerate().map(|(i, (name, shortcut))| {
                        let cmd_name: &'static str = name;
                        div()
                            .id(SharedString::from(format!("cmd-{}", i)))
                            .px(px(12.0))
                            .py(px(6.0))
                            .flex()
                            .flex_row()
                            .justify_between()
                            .items_center()
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(Clr::HOVER)))
                            .when(i == 0, |el| el.bg(rgb(Clr::HOVER)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.tab_mut().command_palette_query = cmd_name.to_string();
                                this.execute_palette_command(cx);
                            }))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(rgb(Clr::TEXT))
                                    .child(name.to_string()),
                            )
                            .when(!shortcut.is_empty(), |el| {
                                el.child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(rgb(Clr::MUTED))
                                        .child(shortcut.to_string()),
                                )
                            })
                    })),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("sidebar")
            .w(px(self.sidebar_width))
            .h_full()
            .bg(rgb(Clr::BG))
            .border_l_1()
            .border_color(rgb(Clr::BORDER))
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .px(px(10.0))
                    .py(px(6.0))
                    .text_size(px(11.0))
                    .text_color(rgb(Clr::TEXT2))
                    .border_b_1()
                    .border_color(rgb(Clr::BORDER))
                    .child(self.active_panel.label()),
            )
            .child(match self.active_panel {
                SidebarPanel::Explorer => self.render_explorer(cx).into_any_element(),
                SidebarPanel::Git => self.render_git_panel().into_any_element(),
                SidebarPanel::Search => self.render_search_panel(cx).into_any_element(),
                SidebarPanel::Context => self.render_context_panel(cx).into_any_element(),
                SidebarPanel::Memory => self.render_memory_panel().into_any_element(),
                SidebarPanel::Settings => self.render_settings_panel(cx).into_any_element(),
            })
    }

    fn render_explorer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.tab();
        let cwd_display = short_path(&tab.cwd);
        div()
            .id("explorer-tree")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .px(px(8.0))
                    .py(px(4.0))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(Clr::MUTED))
                            .child(cwd_display),
                    )
                    .child(
                        div()
                            .id("btn-refresh-tree")
                            .text_size(px(10.0))
                            .text_color(rgb(Clr::TEXT2))
                            .px(px(4.0))
                            .py(px(1.0))
                            .rounded(px(2.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(Clr::HOVER)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let cwd = this.tab().cwd.clone();
                                let tab = this.tab_mut();
                                tab.file_tree = load_directory(&cwd, 0);
                                cx.notify();
                            }))
                            .child("↻"),
                    ),
            )
            .children(
                tab.file_tree
                    .iter()
                    .enumerate()
                    .map(|(i, node)| self.render_file_node(i, node, cx)),
            )
    }

    fn render_file_node(
        &self,
        idx: usize,
        node: &FileNode,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let indent = px(10.0 + node.depth as f32 * 14.0);
        let icon = if node.is_dir {
            if node.expanded {
                "▾ 📂"
            } else {
                "▸ 📁"
            }
        } else {
            file_icon(&node.name)
        };

        let git_color = git_file_color(&self.tab().cwd, &node.path);

        let path = node.path.clone();
        let is_dir = node.is_dir;

        div()
            .id(SharedString::from(format!("fn-{}", idx)))
            .w_full()
            .h(px(24.0))
            .pl(indent)
            .pr(px(8.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .text_size(px(12.0))
            .text_color(if let Some(gc) = git_color {
                rgb(gc)
            } else if node.is_dir {
                rgb(Clr::TEXT)
            } else {
                rgb(Clr::TEXT2)
            })
            .cursor_pointer()
            .hover(|s| s.bg(rgb(Clr::HOVER)))
            .on_click(cx.listener(move |this, _, _, cx| {
                if is_dir {
                    this.toggle_dir(&path, cx);
                } else {
                    this.open_file_preview(&path);
                }
                cx.notify();
            }))
            .child(div().text_size(px(11.0)).child(icon))
            .child(node.name.clone())
    }

    fn open_file_preview(&mut self, path: &PathBuf) {
        let content = std::fs::read_to_string(path).unwrap_or_else(|e| {
            if let Ok(bytes) = std::fs::read(path) {
                if bytes.len() > 1_000_000 {
                    return "File too large to preview (>1MB)".to_string();
                }
                return format!("Binary file ({} bytes)", bytes.len());
            }
            format!("Error reading file: {}", e)
        });
        let content = if content.len() > 1_000_000 {
            format!("{}\n\n... (truncated, file too large)", &content[..500_000])
        } else {
            content
        };
        let tab = self.tab_mut();
        tab.previewing_file = Some(path.clone());
        tab.file_preview_content = Some(content);
    }

    fn toggle_dir(&mut self, path: &PathBuf, _cx: &mut Context<Self>) {
        let tab = self.tab_mut();
        if let Some(node) = tab
            .file_tree
            .iter_mut()
            .find(|n| n.path == *path && n.is_dir)
        {
            if node.expanded {
                node.expanded = false;
                let depth = node.depth;
                let path = node.path.clone();
                tab.file_tree
                    .retain(|n| !(n.depth > depth && n.path.starts_with(&path) && n.path != path));
            } else {
                node.expanded = true;
                if !node.children_loaded {
                    node.children_loaded = true;
                }
                let children = load_directory(path, node.depth + 1);
                let pos = tab
                    .file_tree
                    .iter()
                    .position(|n| n.path == *path)
                    .unwrap_or(0);
                for (j, child) in children.into_iter().enumerate() {
                    tab.file_tree.insert(pos + 1 + j, child);
                }
            }
        }
    }

    // ─── D3: Git Panel — shows git status ───
    fn render_git_panel(&self) -> impl IntoElement {
        let tab = self.tab();
        let branch = tab.cached_git_branch.as_deref().unwrap_or("not a git repo");
        let git_files = read_git_status(&tab.cwd);

        div()
            .id("git-panel")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(10.0))
                    .py(px(6.0))
                    .flex()
                    .flex_row()
                    .gap(px(6.0))
                    .items_center()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(Clr::TEXT))
                            .child(format!("⎇ {}", branch)),
                    ),
            )
            .when(git_files.is_empty(), |el| {
                el.child(
                    div()
                        .p(px(12.0))
                        .text_size(px(12.0))
                        .text_color(rgb(Clr::MUTED))
                        .child("No changes"),
                )
            })
            .children(git_files.iter().map(|(status, path)| {
                let (icon, color) = match status.as_str() {
                    "M" | "MM" => ("M", Clr::AGENT),
                    "A" | "AM" => ("+", Clr::SUCCESS),
                    "D" => ("D", Clr::ERROR),
                    "R" => ("R", Clr::AI),
                    "??" => ("?", Clr::SUCCESS),
                    _ => ("·", Clr::MUTED),
                };
                div()
                    .px(px(10.0))
                    .py(px(2.0))
                    .flex()
                    .flex_row()
                    .gap(px(6.0))
                    .items_center()
                    .text_size(px(12.0))
                    .child(
                        div()
                            .w(px(16.0))
                            .text_color(rgb(color))
                            .child(icon.to_string()),
                    )
                    .child(div().text_color(rgb(Clr::TEXT2)).child(path.clone()))
            }))
    }

    // ─── D6: Search Panel — basic file name search ───
    fn render_search_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.tab();
        let query = &tab.sidebar_search_query;
        let results = &tab.sidebar_search_results;

        // File name search results
        let file_results = if query.len() >= 2 {
            search_files_by_name(&tab.cwd, query, 20)
        } else {
            vec![]
        };

        div()
            .id("search-panel")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            // Search input
            .child(
                div().w_full().px(px(8.0)).py(px(6.0)).child(
                    div()
                        .w_full()
                        .px(px(8.0))
                        .py(px(4.0))
                        .bg(rgb(Clr::ELEVATED))
                        .rounded(px(3.0))
                        .border_1()
                        .border_color(rgb(Clr::BORDER))
                        .text_size(px(12.0))
                        .text_color(if query.is_empty() {
                            rgb(Clr::MUTED)
                        } else {
                            rgb(Clr::TEXT)
                        })
                        .child(if query.is_empty() {
                            "Search files and content...".to_string()
                        } else {
                            query.clone()
                        }),
                ),
            )
            // File name matches
            .when(!file_results.is_empty(), |el| {
                el.child(
                    div()
                        .px(px(10.0))
                        .py(px(3.0))
                        .text_size(px(10.0))
                        .text_color(rgb(Clr::ACCENT))
                        .child("Files"),
                )
                .children(file_results.iter().enumerate().map(|(i, path)| {
                    let display = path
                        .strip_prefix(&tab.cwd)
                        .unwrap_or(path)
                        .display()
                        .to_string();
                    let p = path.clone();
                    div()
                        .id(SharedString::from(format!("sr-{}", i)))
                        .px(px(10.0))
                        .py(px(3.0))
                        .text_size(px(11.0))
                        .text_color(rgb(Clr::TEXT2))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(Clr::HOVER)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_file_preview(&p);
                            cx.notify();
                        }))
                        .child(display)
                }))
            })
            // Content search results
            .when(!results.is_empty(), |el| {
                el.child(
                    div()
                        .px(px(10.0))
                        .py(px(3.0))
                        .text_size(px(10.0))
                        .text_color(rgb(Clr::ACCENT))
                        .child(format!("Content matches ({})", results.len())),
                )
                .children(results.iter().enumerate().map(|(i, result)| {
                    let p = result.file_path.clone();
                    let display = result
                        .file_path
                        .strip_prefix(&tab.cwd)
                        .unwrap_or(&result.file_path)
                        .display()
                        .to_string();
                    div()
                        .id(SharedString::from(format!("cr-{}", i)))
                        .px(px(10.0))
                        .py(px(3.0))
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(Clr::HOVER)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_file_preview(&p);
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(rgb(Clr::TEXT2))
                                .child(format!("{}:{}", display, result.line_number)),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(Clr::MUTED))
                                .child(result.line_content.chars().take(80).collect::<String>()),
                        )
                }))
            })
            .when(
                file_results.is_empty() && results.is_empty() && query.len() >= 2,
                |el| {
                    el.child(
                        div()
                            .px(px(10.0))
                            .py(px(6.0))
                            .text_size(px(12.0))
                            .text_color(rgb(Clr::MUTED))
                            .child("No results found"),
                    )
                },
            )
            // Search button for content search
            .when(query.len() >= 2, |el| {
                el.child(
                    div().px(px(10.0)).py(px(6.0)).child(
                        div()
                            .id("btn-content-search")
                            .px(px(10.0))
                            .py(px(4.0))
                            .bg(rgb(Clr::ACCENT))
                            .text_color(rgb(Clr::BG))
                            .text_size(px(11.0))
                            .rounded(px(3.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(Clr::TEXT)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run_content_search();
                                cx.notify();
                            }))
                            .child("🔍 Search file contents"),
                    ),
                )
            })
    }

    fn run_content_search(&mut self) {
        let query = self.tab().sidebar_search_query.clone();
        let cwd = self.tab().cwd.clone();
        if query.len() < 2 {
            return;
        }

        let output = std::process::Command::new("grep")
            .args([
                "-rn",
                "--include=*.rs",
                "--include=*.toml",
                "--include=*.md",
                "--include=*.json",
                "--include=*.yaml",
                "--include=*.yml",
                "--include=*.py",
                "--include=*.js",
                "--include=*.ts",
                "-m",
                "5",
                &query,
            ])
            .current_dir(&cwd)
            .output();

        let results = match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                stdout
                    .lines()
                    .take(50)
                    .filter_map(|line| {
                        let parts: Vec<&str> = line.splitn(3, ':').collect();
                        if parts.len() >= 3 {
                            Some(SearchResult {
                                file_path: cwd.join(parts[0]),
                                line_number: parts[1].parse().unwrap_or(0),
                                line_content: parts[2].trim().to_string(),
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            Err(_) => vec![],
        };

        self.tab_mut().sidebar_search_results = results;
    }

    // ─── M23: Memory Panel ───
    fn render_context_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = self.tab();
        let ai_blocks: Vec<_> = tab
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::AI(ai) => Some(ai),
                _ => None,
            })
            .collect();

        let total_chars: usize = ai_blocks.iter().map(|a| a.content.len()).sum();
        let estimated_tokens = total_chars / 4;
        let max_tokens: u32 = self
            .settings_store
            .as_ref()
            .and_then(|s| s.merged().max_context_tokens)
            .unwrap_or(128_000);
        let usage_pct = (estimated_tokens as f64 / max_tokens as f64 * 100.0).min(100.0);

        let context_items: Vec<(String, String)> = tab
            .blocks
            .iter()
            .filter_map(|b| match b {
                Block::AI(ai) if ai.role == Role::User => {
                    let preview = ai.content.chars().take(60).collect::<String>();
                    Some(("User message".to_string(), preview))
                }
                Block::AI(ai) if ai.role == Role::Assistant => {
                    let preview = ai.content.chars().take(60).collect::<String>();
                    Some((format!("AI ({})", ai.model), preview))
                }
                Block::Command(cmd) => Some(("Command output".to_string(), cmd.command.clone())),
                _ => None,
            })
            .collect();

        div()
            .id("context-panel")
            .flex_1()
            .overflow_y_scroll()
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(12.0))
            // Token budget bar
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgb(Clr::TEXT2))
                                    .child("Token Budget"),
                            )
                            .child(div().text_size(px(10.0)).text_color(rgb(Clr::MUTED)).child(
                                format!(
                                    "~{} / {} ({:.0}%)",
                                    estimated_tokens, max_tokens, usage_pct
                                ),
                            )),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(6.0))
                            .bg(rgb(Clr::ELEVATED))
                            .rounded(px(3.0))
                            .child(
                                div()
                                    .h(px(6.0))
                                    .w(relative(usage_pct as f32 / 100.0))
                                    .bg(rgb(if usage_pct > 90.0 {
                                        Clr::ERROR
                                    } else if usage_pct > 70.0 {
                                        Clr::AGENT
                                    } else {
                                        Clr::ACCENT
                                    }))
                                    .rounded(px(3.0)),
                            ),
                    ),
            )
            // Context items
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(Clr::TEXT2))
                    .child(format!("Context items ({})", context_items.len())),
            )
            .children(
                context_items
                    .iter()
                    .enumerate()
                    .map(|(i, (kind, preview))| {
                        div()
                            .id(SharedString::from(format!("ctx-{}", i)))
                            .px(px(8.0))
                            .py(px(4.0))
                            .bg(rgb(Clr::SURFACE))
                            .rounded(px(3.0))
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(rgb(Clr::ACCENT))
                                            .child(kind.clone()),
                                    )
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("rm-ctx-{}", i)))
                                            .text_size(px(9.0))
                                            .text_color(rgb(Clr::MUTED))
                                            .px(px(4.0))
                                            .rounded(px(2.0))
                                            .cursor_pointer()
                                            .hover(|s| {
                                                s.bg(rgb(Clr::HOVER)).text_color(rgb(Clr::ERROR))
                                            })
                                            .child("✕"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(Clr::MUTED))
                                    .child(format!("{}…", preview)),
                            )
                    }),
            )
    }

    fn render_memory_panel(&self) -> impl IntoElement {
        let memory_entries = load_memory_entries();

        div()
            .id("memory-panel")
            .flex_1()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(10.0))
                    .py(px(6.0))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(Clr::MUTED))
                            .child(format!("{} memories", memory_entries.len())),
                    ),
            )
            .when(memory_entries.is_empty(), |el| {
                el.child(
                    div()
                        .p(px(12.0))
                        .text_size(px(12.0))
                        .text_color(rgb(Clr::MUTED))
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child("No memories yet.")
                        .child("Memories are auto-captured from AI conversations."),
                )
            })
            .children(memory_entries.iter().enumerate().map(|(i, entry)| {
                div()
                    .id(SharedString::from(format!("mem-{}", i)))
                    .px(px(10.0))
                    .py(px(4.0))
                    .border_b_1()
                    .border_color(rgb(Clr::BORDER))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(Clr::TEXT2))
                            .child(entry.content.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(9.0))
                            .text_color(rgb(Clr::MUTED))
                            .child(entry.timestamp.clone()),
                    )
            }))
    }

    fn render_settings_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_model = self
            .settings_store
            .as_ref()
            .and_then(|s| s.merged().model.clone())
            .unwrap_or_else(resolve_ai_model);
        let model_display = if current_model.is_empty() {
            "No model configured".to_string()
        } else {
            current_model.clone()
        };

        let providers: Vec<(&str, &str, bool)> = vec![
            (
                "Anthropic",
                "ANTHROPIC_API_KEY",
                std::env::var("ANTHROPIC_API_KEY").is_ok(),
            ),
            (
                "OpenAI",
                "OPENAI_API_KEY",
                std::env::var("OPENAI_API_KEY").is_ok(),
            ),
            ("xAI", "XAI_API_KEY", std::env::var("XAI_API_KEY").is_ok()),
        ];

        let model_choices = [
            "anthropic/claude-sonnet-4-20250514",
            "anthropic/claude-3-5-sonnet-20241022",
            "openai/gpt-4o",
            "openai/gpt-4o-mini",
            "xai/grok-3",
        ];

        let editing_provider = self.settings_editing_provider.clone();
        let api_key_input = self.settings_api_key_input.clone();
        let test_result = self.settings_test_result.clone();

        div()
            .id("settings-panel")
            .flex_1()
            .overflow_y_scroll()
            .p(px(16.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(rgb(Clr::TEXT))
                    .child("Settings"),
            )
            // Model Selection
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(Clr::ACCENT))
                            .child("Active Model"),
                    )
                    .child(
                        div()
                            .px(px(10.0))
                            .py(px(8.0))
                            .bg(rgb(Clr::SURFACE))
                            .rounded(px(4.0))
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(if current_model.is_empty() {
                                        rgb(Clr::ERROR)
                                    } else {
                                        rgb(Clr::SUCCESS)
                                    })
                                    .child(model_display),
                            )
                            .children(model_choices.iter().map(|&model| {
                                let is_selected = current_model == model;
                                let model_owned = model.to_string();
                                div()
                                    .id(SharedString::from(format!("model-{}", model)))
                                    .px(px(8.0))
                                    .py(px(3.0))
                                    .rounded(px(3.0))
                                    .cursor_pointer()
                                    .text_size(px(11.0))
                                    .text_color(if is_selected {
                                        rgb(Clr::ACCENT)
                                    } else {
                                        rgb(Clr::TEXT2)
                                    })
                                    .hover(|s| s.bg(rgb(Clr::HOVER)))
                                    .when(is_selected, |el| el.bg(rgb(Clr::ELEVATED)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(ref store) = this.settings_store {
                                            let update = serde_json::json!({
                                                "model": model_owned.clone()
                                            });
                                            let _ = store.save_user(&update);
                                        }
                                        let user_path =
                                            dirs_or_home().join(".aineer").join("settings.json");
                                        this.settings_store =
                                            aineer_settings::SettingsStore::load(user_path, None)
                                                .ok();
                                        cx.notify();
                                    }))
                                    .child(format!(
                                        "{} {}",
                                        if is_selected { "●" } else { "○" },
                                        model
                                    ))
                            })),
                    ),
            )
            // Provider API Keys
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(Clr::ACCENT))
                            .child("Provider API Keys"),
                    )
                    .children(providers.iter().map(|(name, env_var, active)| {
                        let name_s = name.to_string();
                        let env_var_s = env_var.to_string();
                        let is_editing = editing_provider.as_deref() == Some(*name);

                        div()
                            .px(px(10.0))
                            .py(px(6.0))
                            .bg(rgb(Clr::SURFACE))
                            .rounded(px(4.0))
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(8.0))
                                    .items_center()
                                    .child(
                                        div().w(px(8.0)).h(px(8.0)).rounded(px(4.0)).bg(rgb(
                                            if *active { Clr::SUCCESS } else { Clr::MUTED },
                                        )),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .text_color(rgb(Clr::TEXT))
                                            .child(name.to_string()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(rgb(Clr::MUTED))
                                            .child(if *active {
                                                "✓ key set via env".to_string()
                                            } else {
                                                format!("needs {}", env_var)
                                            }),
                                    )
                                    .when(!is_editing, |el| {
                                        let name_c = name_s.clone();
                                        el.child(
                                            div()
                                                .id(SharedString::from(format!("edit-{}", name_c)))
                                                .px(px(6.0))
                                                .py(px(1.0))
                                                .text_size(px(10.0))
                                                .text_color(rgb(Clr::ACCENT))
                                                .rounded(px(3.0))
                                                .cursor_pointer()
                                                .hover(|s| s.bg(rgb(Clr::HOVER)))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.settings_editing_provider =
                                                        Some(name_c.clone());
                                                    this.settings_api_key_input.clear();
                                                    this.settings_test_result = None;
                                                    cx.notify();
                                                }))
                                                .child("Edit"),
                                        )
                                    }),
                            )
                            .when(is_editing, |el| {
                                let env_var_c = env_var_s.clone();
                                let name_c = name_s.clone();
                                let key_display = if api_key_input.is_empty() {
                                    "Enter API key...".to_string()
                                } else {
                                    format!(
                                        "{}{}",
                                        &api_key_input[..api_key_input.len().min(8)],
                                        "•".repeat(api_key_input.len().saturating_sub(8))
                                    )
                                };
                                el.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.0))
                                        .child(
                                            div()
                                                .w_full()
                                                .px(px(8.0))
                                                .py(px(4.0))
                                                .bg(rgb(Clr::ELEVATED))
                                                .rounded(px(3.0))
                                                .border_1()
                                                .border_color(rgb(Clr::BORDER))
                                                .text_size(px(11.0))
                                                .text_color(if api_key_input.is_empty() {
                                                    rgb(Clr::MUTED)
                                                } else {
                                                    rgb(Clr::TEXT)
                                                })
                                                .child(key_display),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .gap(px(6.0))
                                                .child(
                                                    div()
                                                        .id(SharedString::from(format!(
                                                            "save-{}",
                                                            name_c
                                                        )))
                                                        .px(px(8.0))
                                                        .py(px(2.0))
                                                        .bg(rgb(Clr::ACCENT))
                                                        .text_color(rgb(Clr::BG))
                                                        .text_size(px(10.0))
                                                        .rounded(px(3.0))
                                                        .cursor_pointer()
                                                        .hover(|s| s.bg(rgb(Clr::TEXT)))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                let key = this
                                                                    .settings_api_key_input
                                                                    .clone();
                                                                if !key.is_empty() {
                                                                    std::env::set_var(
                                                                        &env_var_c, &key,
                                                                    );
                                                                }
                                                                this.settings_editing_provider =
                                                                    None;
                                                                this.settings_api_key_input.clear();
                                                                cx.notify();
                                                            },
                                                        ))
                                                        .child("Save"),
                                                )
                                                .child(
                                                    div()
                                                        .id(SharedString::from(format!(
                                                            "cancel-{}",
                                                            name_c
                                                        )))
                                                        .px(px(8.0))
                                                        .py(px(2.0))
                                                        .text_size(px(10.0))
                                                        .text_color(rgb(Clr::MUTED))
                                                        .rounded(px(3.0))
                                                        .cursor_pointer()
                                                        .hover(|s| {
                                                            s.bg(rgb(Clr::HOVER))
                                                                .text_color(rgb(Clr::TEXT))
                                                        })
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.settings_editing_provider = None;
                                                            this.settings_api_key_input.clear();
                                                            cx.notify();
                                                        }))
                                                        .child("Cancel"),
                                                ),
                                        )
                                        .when(test_result.is_some(), |el| {
                                            let (msg, ok) = test_result.clone().unwrap();
                                            el.child(
                                                div()
                                                    .text_size(px(10.0))
                                                    .text_color(rgb(if ok {
                                                        Clr::SUCCESS
                                                    } else {
                                                        Clr::ERROR
                                                    }))
                                                    .child(msg),
                                            )
                                        }),
                                )
                            })
                    })),
            )
            // Configuration
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(Clr::ACCENT))
                            .child("Configuration"),
                    )
                    .child(
                        div()
                            .px(px(10.0))
                            .py(px(8.0))
                            .bg(rgb(Clr::SURFACE))
                            .rounded(px(4.0))
                            .text_size(px(11.0))
                            .text_color(rgb(Clr::TEXT2))
                            .flex()
                            .flex_col()
                            .gap(px(4.0))
                            .child("Config: ~/.aineer/settings.json")
                            .child("Sessions: ~/.aineer/sessions/")
                            .child(format!("Version: {}", env!("CARGO_PKG_VERSION"))),
                    ),
            )
            // Appearance
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(Clr::ACCENT))
                            .child("Appearance"),
                    )
                    .child(
                        div()
                            .px(px(10.0))
                            .py(px(8.0))
                            .bg(rgb(Clr::SURFACE))
                            .rounded(px(4.0))
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .child({
                                let is_dark = matches!(
                                    self.theme.appearance,
                                    aineer_theme::ThemeAppearance::Dark
                                );
                                div()
                                    .id("theme-dark")
                                    .px(px(10.0))
                                    .py(px(4.0))
                                    .rounded(px(3.0))
                                    .cursor_pointer()
                                    .text_size(px(11.0))
                                    .bg(rgb(if is_dark { Clr::ACCENT } else { Clr::ELEVATED }))
                                    .text_color(rgb(if is_dark { Clr::BG } else { Clr::TEXT2 }))
                                    .hover(|s| s.bg(rgb(Clr::ACCENT)).text_color(rgb(Clr::BG)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.theme = aineer_theme::Theme::dark_default();
                                        cx.notify();
                                    }))
                                    .child("🌙 Dark")
                            })
                            .child({
                                let is_light = matches!(
                                    self.theme.appearance,
                                    aineer_theme::ThemeAppearance::Light
                                );
                                div()
                                    .id("theme-light")
                                    .px(px(10.0))
                                    .py(px(4.0))
                                    .rounded(px(3.0))
                                    .cursor_pointer()
                                    .text_size(px(11.0))
                                    .bg(rgb(if is_light { Clr::ACCENT } else { Clr::ELEVATED }))
                                    .text_color(rgb(if is_light { Clr::BG } else { Clr::TEXT2 }))
                                    .hover(|s| s.bg(rgb(Clr::ACCENT)).text_color(rgb(Clr::BG)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.theme = aineer_theme::Theme::light_default();
                                        cx.notify();
                                    }))
                                    .child("☀ Light")
                            })
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(Clr::MUTED))
                                    .child(format!("Current: {}", self.theme.name)),
                            ),
                    ),
            )
            // Keyboard shortcuts
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(Clr::ACCENT))
                            .child("Keyboard Shortcuts"),
                    )
                    .child(
                        div()
                            .px(px(10.0))
                            .py(px(8.0))
                            .bg(rgb(Clr::SURFACE))
                            .rounded(px(4.0))
                            .text_size(px(11.0))
                            .text_color(rgb(Clr::TEXT2))
                            .flex()
                            .flex_col()
                            .gap(px(3.0))
                            .child("Enter — Execute (Shell mode)")
                            .child("⌘Enter — Send to AI")
                            .child("⌘⇧Enter — Send to Agent")
                            .child("Escape — Cancel / Switch to Shell")
                            .child("⌘F — Search in blocks")
                            .child("⌘T — New tab")
                            .child("⌘W — Close tab")
                            .child("Ctrl+L — Clear blocks")
                            .child("Ctrl+A / E — Line start / end")
                            .child("Ctrl+U / K — Clear before / after cursor")
                            .child("↑↓ — Command history"),
                    ),
            )
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Session Persistence
// ═══════════════════════════════════════════════════════════════════

use crate::session::{SessionData, TabSession};

fn session_dir() -> PathBuf {
    dirs_or_home().join(".aineer").join("sessions")
}

fn last_session_path() -> PathBuf {
    session_dir().join("last_session.json")
}

fn restore_session(default_cwd: &PathBuf) -> (Vec<TabState>, usize, bool, f32, usize) {
    let path = last_session_path();
    if let Ok(data) = SessionData::load(&path) {
        let mut tabs: Vec<TabState> = data
            .tabs
            .into_iter()
            .enumerate()
            .map(|(i, ts)| {
                let cwd = if ts.working_dir.exists() {
                    ts.working_dir
                } else {
                    default_cwd.clone()
                };
                let mut state = TabState::new(i, ts.title, cwd);
                state.blocks = ts.blocks;
                state.next_block_id = state.blocks.iter().map(|b| b.id()).max().unwrap_or(0) + 1;
                state
            })
            .collect();

        if tabs.is_empty() {
            tabs.push(TabState::new(0, "Terminal".into(), default_cwd.clone()));
        }

        let active = data.active_tab_index.min(tabs.len() - 1);
        let max_id = tabs.iter().map(|t| t.id).max().unwrap_or(0);
        (
            tabs,
            active,
            data.sidebar_visible,
            data.sidebar_width,
            max_id,
        )
    } else {
        (
            vec![TabState::new(0, "Terminal".into(), default_cwd.clone())],
            0,
            true,
            260.0,
            0,
        )
    }
}

impl AineerWorkspace {
    pub fn save_session(&self) {
        let data = SessionData {
            version: 1,
            tabs: self
                .tabs
                .iter()
                .map(|tab| TabSession {
                    id: tab.id as u64,
                    title: tab.title.clone(),
                    working_dir: tab.cwd.clone(),
                    blocks: tab.blocks.clone(),
                    scroll_position: 0.0,
                })
                .collect(),
            active_tab_index: self.active_tab,
            sidebar_visible: self.sidebar_visible,
            sidebar_width: self.sidebar_width,
        };

        if let Err(e) = data.save(&last_session_path()) {
            tracing::warn!("Failed to save session: {}", e);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  File System Helpers
// ═══════════════════════════════════════════════════════════════════

fn load_directory(path: &PathBuf, depth: usize) -> Vec<FileNode> {
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(path) else {
        return entries;
    };
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    for entry in read_dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if matches!(name.as_str(), "target" | "node_modules" | "__pycache__") {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let node = FileNode {
            name,
            path: entry.path(),
            is_dir,
            depth,
            expanded: false,
            children_loaded: false,
        };
        if is_dir {
            dirs.push(node);
        } else {
            files.push(node);
        }
    }
    dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries.extend(dirs);
    entries.extend(files);
    entries
}

fn file_icon(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "  🦀",
        "toml" => "  ⚙",
        "md" => "  📝",
        "json" => "  {}",
        "yaml" | "yml" => "  📋",
        "sh" | "bash" => "  🐚",
        "py" => "  🐍",
        "js" | "ts" | "tsx" | "jsx" => "  ⬡",
        "svg" | "png" | "jpg" | "ico" | "icns" => "  🖼",
        "lock" => "  🔒",
        "txt" => "  📄",
        "gitignore" => "  🙈",
        _ => "  📄",
    }
}

fn short_path(path: &PathBuf) -> String {
    let home = dirs_or_home();
    if let Ok(stripped) = path.strip_prefix(&home) {
        format!("~/{}", stripped.display())
    } else {
        path.display().to_string()
    }
}

fn dirs_or_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/"))
}

struct MemoryEntry {
    content: String,
    timestamp: String,
}

fn load_memory_entries() -> Vec<MemoryEntry> {
    let memory_file = dirs_or_home().join(".aineer").join("memory.json");
    let Ok(data) = std::fs::read_to_string(&memory_file) else {
        return vec![];
    };
    let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(&data) else {
        return vec![];
    };
    entries
        .iter()
        .filter_map(|e| {
            Some(MemoryEntry {
                content: e.get("content")?.as_str()?.to_string(),
                timestamp: e
                    .get("timestamp")
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
            })
        })
        .collect()
}

fn render_diff_line(line: &DiffLine) -> Div {
    let (prefix, text, bg_color, fg_color) = match line {
        DiffLine::Context(t) => (" ", t.as_str(), Clr::SURFACE, Clr::TEXT2),
        DiffLine::Addition(t) => ("+", t.as_str(), 0x1a3d1a, Clr::SUCCESS),
        DiffLine::Deletion(t) => ("-", t.as_str(), 0x3d1a1a, Clr::ERROR),
    };
    div()
        .w_full()
        .px(px(10.0))
        .bg(rgb(bg_color))
        .flex()
        .flex_row()
        .gap(px(6.0))
        .child(
            div()
                .w(px(12.0))
                .text_size(px(11.0))
                .text_color(rgb(fg_color))
                .child(prefix),
        )
        .child(
            div()
                .flex_1()
                .text_size(px(11.0))
                .text_color(rgb(fg_color))
                .child(text.to_string()),
        )
}

fn utf16_to_utf8_offset_text(text: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0;
    for (byte_offset, ch) in text.char_indices() {
        if utf16_count >= utf16_offset {
            return byte_offset;
        }
        utf16_count += ch.len_utf16();
    }
    text.len()
}

fn block_text(block: &Block) -> String {
    match block {
        Block::Command(c) => format!("{} {}", c.command, c.output_text),
        Block::AI(a) => a.content.clone(),
        Block::Tool(t) => format!("{} {}", t.name, t.input),
        Block::System(s) => s.message.clone(),
        Block::Diff(d) => d.file_path.clone(),
        Block::AgentPlan(p) => format!(
            "{} {}",
            p.goal,
            p.steps
                .iter()
                .map(|s| s.description.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        ),
    }
}

fn git_file_color(repo_root: &PathBuf, file_path: &PathBuf) -> Option<u32> {
    let rel = file_path
        .strip_prefix(repo_root)
        .ok()?
        .display()
        .to_string();
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain", "--", &rel])
        .current_dir(repo_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let line = String::from_utf8_lossy(&output.stdout);
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    match &line[..2] {
        "M " | " M" | "MM" => Some(Clr::AGENT),
        "A " | "AM" => Some(Clr::SUCCESS),
        "D " | " D" => Some(Clr::ERROR),
        "??" => Some(Clr::SUCCESS),
        _ => Some(Clr::MUTED),
    }
}

fn read_git_status(dir: &PathBuf) -> Vec<(String, String)> {
    std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|line| {
                    if line.len() < 4 {
                        return None;
                    }
                    let status = line[..2].trim().to_string();
                    let path = line[3..].to_string();
                    Some((status, path))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn search_files_by_name(dir: &PathBuf, query: &str, limit: usize) -> Vec<PathBuf> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();
    search_files_recursive(dir, &query_lower, &mut results, limit, 0, 5);
    results
}

fn search_files_recursive(
    dir: &PathBuf,
    query: &str,
    results: &mut Vec<PathBuf>,
    limit: usize,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth || results.len() >= limit {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if results.len() >= limit {
            return;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.')
            || matches!(name.as_str(), "target" | "node_modules" | "__pycache__")
        {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if name.to_lowercase().contains(query) {
            results.push(entry.path());
        }
        if is_dir {
            search_files_recursive(&entry.path(), query, results, limit, depth + 1, max_depth);
        }
    }
}

fn resolve_ai_model() -> String {
    for var in ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "XAI_API_KEY"] {
        if std::env::var(var).is_ok() {
            return match var {
                "ANTHROPIC_API_KEY" => "claude-sonnet-4-20250514".to_string(),
                "OPENAI_API_KEY" => "gpt-4o".to_string(),
                "XAI_API_KEY" => "grok-3-mini-fast-latest".to_string(),
                _ => String::new(),
            };
        }
    }
    String::new()
}

fn extract_stream_text(event: &aineer_api::StreamEvent) -> Option<String> {
    match event {
        aineer_api::StreamEvent::ContentBlockDelta(delta) => match &delta.delta {
            aineer_api::ContentBlockDelta::TextDelta { text } => Some(text.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn read_git_branch(dir: &PathBuf) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
}
