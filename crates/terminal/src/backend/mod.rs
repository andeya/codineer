pub mod settings;

use crate::types::Size;
use alacritty_terminal::event::{Event, EventListener, Notify, OnResize, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, Msg, Notifier};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{
    Selection, SelectionRange, SelectionType as AlacrittySelectionType,
};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::search::{Match, RegexIter, RegexSearch};
use alacritty_terminal::term::{
    self, cell::Cell, test::TermSize, viewport_to_point, Term, TermMode,
};
use alacritty_terminal::{tty, Grid};
use egui::Modifiers;
use settings::BackendSettings;
use std::borrow::Cow;
use std::cmp::min;
use std::io::Result;
use std::ops::{Index, RangeInclusive};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc};

pub type TerminalMode = TermMode;
pub type PtyEvent = Event;
pub type SelectionType = AlacrittySelectionType;

#[derive(Debug, Clone)]
pub enum BackendCommand {
    Write(Vec<u8>),
    Scroll(i32),
    Resize(Size, Size),
    SelectStart(SelectionType, f32, f32),
    SelectUpdate(f32, f32),
    ProcessLink(LinkAction, Point),
    MouseReport(MouseButton, Modifiers, Point, bool),
}

#[derive(Debug, Clone)]
pub enum MouseMode {
    Sgr,
    Normal(bool),
}

impl From<TermMode> for MouseMode {
    fn from(term_mode: TermMode) -> Self {
        if term_mode.contains(TermMode::SGR_MOUSE) {
            MouseMode::Sgr
        } else if term_mode.contains(TermMode::UTF8_MOUSE) {
            MouseMode::Normal(true)
        } else {
            MouseMode::Normal(false)
        }
    }
}

#[derive(Debug, Clone)]
pub enum MouseButton {
    LeftButton = 0,
    MiddleButton = 1,
    RightButton = 2,
    LeftMove = 32,
    MiddleMove = 33,
    RightMove = 34,
    NoneMove = 35,
    ScrollUp = 64,
    ScrollDown = 65,
    Other = 99,
}

#[derive(Debug, Clone)]
pub enum LinkAction {
    Clear,
    Hover,
    Open,
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalSize {
    pub cell_width: u16,
    pub cell_height: u16,
    num_cols: u16,
    num_lines: u16,
    layout_size: Size,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            cell_width: 1,
            cell_height: 1,
            num_cols: 80,
            num_lines: 50,
            layout_size: Size::default(),
        }
    }
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        self.num_lines as usize
    }

    fn columns(&self) -> usize {
        self.num_cols as usize
    }

    fn last_column(&self) -> Column {
        Column(self.num_cols as usize - 1)
    }

    fn bottommost_line(&self) -> Line {
        Line(self.num_lines as i32 - 1)
    }
}

impl From<TerminalSize> for WindowSize {
    fn from(size: TerminalSize) -> Self {
        Self {
            num_lines: size.num_lines,
            num_cols: size.num_cols,
            cell_width: size.cell_width,
            cell_height: size.cell_height,
        }
    }
}

pub struct TerminalBackend {
    id: u64,
    pty_id: u32,
    url_regex: RegexSearch,
    term: Arc<FairMutex<Term<EventProxy>>>,
    size: TerminalSize,
    notifier: Notifier,
    last_content: RenderableContent,
    write_generation: Arc<AtomicU64>,
    search_state: SearchState,
}

#[derive(Default)]
struct SearchState {
    regex: Option<RegexSearch>,
    query: String,
    current_match: Option<Match>,
}

impl TerminalBackend {
    pub fn new(
        id: u64,
        app_context: egui::Context,
        pty_event_proxy_sender: Sender<(u64, PtyEvent)>,
        settings: BackendSettings,
    ) -> Result<Self> {
        let pty_config = tty::Options {
            shell: Some(tty::Shell::new(settings.shell, settings.args)),
            working_directory: settings.working_directory,
            ..tty::Options::default()
        };
        let config = term::Config::default();
        let terminal_size = TerminalSize::default();
        let pty = tty::new(&pty_config, terminal_size.into(), id)?;
        #[cfg(not(windows))]
        let pty_id = pty.child().id();
        #[cfg(windows)]
        let pty_id = pty
            .child_watcher()
            .pid()
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Failed to get child process ID",
            ))?
            .into();
        let (event_sender, event_receiver) = mpsc::channel();
        let event_proxy = EventProxy(event_sender);
        let mut term = Term::new(config, &terminal_size, event_proxy.clone());
        let initial_content = RenderableContent {
            grid: term.grid().clone(),
            selectable_range: None,
            terminal_mode: *term.mode(),
            terminal_size,
            cursor: term.grid_mut().cursor_cell().clone(),
            hovered_hyperlink: None,
            generation: 0,
            search_match: None,
        };
        let term = Arc::new(FairMutex::new(term));
        let pty_event_loop = EventLoop::new(term.clone(), event_proxy, pty, false, false)?;
        let notifier = Notifier(pty_event_loop.channel());
        let pty_notifier = Notifier(pty_event_loop.channel());
        let write_generation = Arc::new(AtomicU64::new(0));
        let url_regex = RegexSearch::new(r#"(ipfs:|ipns:|magnet:|mailto:|gemini://|gopher://|https://|http://|news:|file://|git://|ssh:|ftp://)[^\u{0000}-\u{001F}\u{007F}-\u{009F}<>"\s{-}\^⟨⟩`]+"#).unwrap();
        let _pty_event_loop_thread = pty_event_loop.spawn();
        let gen_counter = write_generation.clone();
        let _pty_event_subscription = std::thread::Builder::new()
            .name(format!("pty_event_subscription_{}", id))
            .spawn(move || loop {
                if let Ok(event) = event_receiver.recv() {
                    gen_counter.fetch_add(1, Ordering::Relaxed);
                    if pty_event_proxy_sender.send((id, event.clone())).is_err() {
                        tracing::warn!("pty_event_subscription_{id}: event channel closed");
                        break;
                    }
                    // Rate-limit repaints: coalesce bursts of PTY output to
                    // at most ~60 fps so the UI stays responsive under heavy
                    // terminal activity (e.g. cargo build output).
                    app_context
                        .clone()
                        .request_repaint_after(std::time::Duration::from_millis(16));
                    match event {
                        Event::Exit => break,
                        Event::PtyWrite(pty) => pty_notifier.notify(pty.into_bytes()),
                        _ => {}
                    }
                }
            })?;

        Ok(Self {
            id,
            pty_id,
            url_regex,
            term: term.clone(),
            size: terminal_size,
            notifier,
            last_content: initial_content,
            write_generation,
            search_state: SearchState::default(),
        })
    }

    pub fn process_command(&mut self, cmd: BackendCommand) {
        let term = self.term.clone();
        let mut term = term.lock();
        match cmd {
            BackendCommand::Write(input) => {
                self.write(input);
                term.scroll_display(Scroll::Bottom);
            }
            BackendCommand::Scroll(delta) => {
                self.scroll(&mut term, delta);
            }
            BackendCommand::Resize(layout_size, font_size) => {
                self.resize(&mut term, layout_size, font_size);
            }
            BackendCommand::SelectStart(selection_type, x, y) => {
                self.start_selection(&mut term, selection_type, x, y);
            }
            BackendCommand::SelectUpdate(x, y) => {
                self.update_selection(&mut term, x, y);
            }
            BackendCommand::ProcessLink(link_action, point) => {
                self.process_link_action(&term, link_action, point);
            }
            BackendCommand::MouseReport(button, modifiers, point, pressed) => {
                self.process_mouse_report(button, modifiers, point, pressed);
            }
        };
    }

    pub fn selection_point(
        x: f32,
        y: f32,
        terminal_size: &TerminalSize,
        display_offset: usize,
    ) -> Point {
        let col = (x as usize) / (terminal_size.cell_width as usize);
        let col = min(Column(col), Column(terminal_size.num_cols as usize - 1));

        let line = (y as usize) / (terminal_size.cell_height as usize);
        let line = min(line, terminal_size.num_lines as usize - 1);

        viewport_to_point(display_offset, Point::new(line, col))
    }

    pub fn selectable_content(&self) -> String {
        let content = self.last_content();
        let mut result = String::new();
        if let Some(range) = content.selectable_range {
            for indexed in content.grid.display_iter() {
                if range.contains(indexed.point) {
                    result.push(indexed.c);
                }
            }
        }
        result
    }

    pub fn sync(&mut self) -> &RenderableContent {
        let term = self.term.clone();
        let mut terminal = term.lock();
        let selectable_range = match &terminal.selection {
            Some(s) => s.to_range(&terminal),
            None => None,
        };

        let cursor = terminal.grid_mut().cursor_cell().clone();
        self.last_content.grid = terminal.grid().clone();
        self.last_content.selectable_range = selectable_range;
        self.last_content.cursor = cursor.clone();
        self.last_content.terminal_mode = *terminal.mode();
        self.last_content.terminal_size = self.size;
        self.last_content.generation = self.write_generation.load(Ordering::Relaxed);
        self.last_content.search_match = self.search_state.current_match.clone();
        self.last_content()
    }

    pub fn last_content(&self) -> &RenderableContent {
        &self.last_content
    }

    /// Current write generation counter — incremented on each PTY event.
    pub fn write_generation(&self) -> u64 {
        self.write_generation.load(Ordering::Relaxed)
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn pty_id(&self) -> u32 {
        self.pty_id
    }

    /// Extract visible text content from the terminal grid as a Vec of lines.
    pub fn visible_text(&mut self) -> Vec<String> {
        self.sync();
        let content = &self.last_content;
        let cols = content.terminal_size.columns();
        let lines = content.terminal_size.screen_lines();
        let mut result = Vec::with_capacity(lines);

        for line_idx in 0..lines {
            let line = Line(line_idx as i32);
            let mut row = String::with_capacity(cols);
            for col_idx in 0..cols {
                let point = Point::new(line, Column(col_idx));
                let cell = &content.grid[point];
                // Skip wide-char spacers (they produce double-width glyph artifacts).
                if cell.flags.contains(term::cell::Flags::WIDE_CHAR_SPACER) {
                    continue;
                }
                // Replace NUL / C0/C1 control codes with a space so that plain
                // text labels don't render garbage glyphs.
                let ch = cell.c;
                row.push(if ch == '\0' || (ch < ' ' && ch != '\t') {
                    ' '
                } else {
                    ch
                });
            }
            result.push(row.trim_end().to_string());
        }

        while result.last().is_some_and(|l| l.is_empty()) {
            result.pop();
        }
        result
    }

    /// Extract visible text with color info from the terminal grid.
    pub fn visible_styled_lines(&mut self, theme: &crate::theme::TerminalTheme) -> Vec<StyledLine> {
        self.sync();
        let content = &self.last_content;
        let cols = content.terminal_size.columns();
        let lines = content.terminal_size.screen_lines();
        let mut result = Vec::with_capacity(lines);

        for line_idx in 0..lines {
            let line = Line(line_idx as i32);
            let mut segments: Vec<StyledSegment> = Vec::new();
            let mut cur_text = String::new();
            let mut cur_fg = egui::Color32::GRAY;
            let mut cur_bold = false;

            for col_idx in 0..cols {
                let point = Point::new(line, Column(col_idx));
                let cell = &content.grid[point];

                if cell.flags.contains(term::cell::Flags::WIDE_CHAR_SPACER) {
                    continue;
                }

                let mut fg = theme.get_color(cell.fg);
                if cell
                    .flags
                    .intersects(term::cell::Flags::DIM | term::cell::Flags::DIM_BOLD)
                {
                    fg = fg.linear_multiply(0.7);
                }
                if cell.flags.contains(term::cell::Flags::INVERSE) {
                    fg = theme.get_color(cell.bg);
                }
                let bold = cell.flags.contains(term::cell::Flags::BOLD);

                if !cur_text.is_empty() && (fg != cur_fg || bold != cur_bold) {
                    segments.push(StyledSegment {
                        text: std::mem::take(&mut cur_text),
                        fg: cur_fg,
                        bold: cur_bold,
                    });
                }
                cur_fg = fg;
                cur_bold = bold;
                cur_text.push(cell.c);
            }

            if !cur_text.is_empty() {
                segments.push(StyledSegment {
                    text: cur_text,
                    fg: cur_fg,
                    bold: cur_bold,
                });
            }

            // Trim trailing whitespace from last segment
            if let Some(last) = segments.last_mut() {
                let trimmed = last.text.trim_end().to_string();
                if trimmed.is_empty() {
                    segments.pop();
                } else {
                    last.text = trimmed;
                }
            }

            let is_empty =
                segments.is_empty() || (segments.len() == 1 && segments[0].text.trim().is_empty());
            result.push(StyledLine {
                segments: if is_empty { vec![] } else { segments },
            });
        }

        while result.last().is_some_and(|l| l.segments.is_empty()) {
            result.pop();
        }
        result
    }

    /// Get CWD by querying the child process.
    pub fn current_cwd(&self) -> Option<std::path::PathBuf> {
        current_cwd_for_pid(self.pty_id)
    }

    /// Set the search query. Returns true if the regex compiled successfully.
    pub fn search_set_query(&mut self, query: &str) -> bool {
        if query.is_empty() {
            self.search_state = SearchState::default();
            self.last_content.search_match = None;
            return true;
        }
        if query == self.search_state.query {
            return self.search_state.regex.is_some();
        }
        match RegexSearch::new(query) {
            Ok(regex) => {
                self.search_state.query = query.to_string();
                self.search_state.regex = Some(regex);
                self.search_state.current_match = None;
                self.search_find_next();
                true
            }
            Err(_) => {
                self.search_state.query = query.to_string();
                self.search_state.regex = None;
                self.search_state.current_match = None;
                self.last_content.search_match = None;
                false
            }
        }
    }

    /// Find the next match from the current position.
    pub fn search_find_next(&mut self) {
        let Some(regex) = &mut self.search_state.regex else {
            return;
        };
        let term = self.term.clone();
        let mut term = term.lock();
        let origin = self
            .search_state
            .current_match
            .as_ref()
            .map(|m| *m.end())
            .unwrap_or_else(|| Point::new(Line(0), Column(0)));
        if let Some(m) = term.search_next(regex, origin, Direction::Right, Side::Left, None) {
            self.scroll_to_match(&mut term, &m);
            self.search_state.current_match = Some(m.clone());
            self.last_content.search_match = Some(m);
        }
    }

    /// Find the previous match.
    pub fn search_find_prev(&mut self) {
        let Some(regex) = &mut self.search_state.regex else {
            return;
        };
        let term = self.term.clone();
        let mut term = term.lock();
        let origin = self
            .search_state
            .current_match
            .as_ref()
            .map(|m| *m.start())
            .unwrap_or_else(|| {
                Point::new(
                    term.bottommost_line(),
                    Column(term.columns().saturating_sub(1)),
                )
            });
        if let Some(m) = term.search_next(regex, origin, Direction::Left, Side::Right, None) {
            self.scroll_to_match(&mut term, &m);
            self.search_state.current_match = Some(m.clone());
            self.last_content.search_match = Some(m);
        }
    }

    fn scroll_to_match(&self, term: &mut Term<EventProxy>, m: &Match) {
        let match_line = m.start().line.0;
        let display_offset = term.grid().display_offset() as i32;
        let screen_lines = self.size.num_lines as i32;
        let visible_top = -(display_offset);
        let visible_bottom = visible_top + screen_lines - 1;
        if match_line < visible_top {
            let delta = visible_top - match_line;
            term.grid_mut().scroll_display(Scroll::Delta(delta));
        } else if match_line > visible_bottom {
            let delta = visible_bottom - match_line;
            term.grid_mut().scroll_display(Scroll::Delta(delta));
        }
    }

    /// Clear search state.
    pub fn search_clear(&mut self) {
        self.search_state = SearchState::default();
        self.last_content.search_match = None;
    }

    /// Returns true if search is active.
    pub fn has_search_match(&self) -> bool {
        self.search_state.current_match.is_some()
    }

    /// Returns true if the terminal is in alternate screen mode (vim, htop, less, etc.)
    pub fn is_alternate_screen(&self) -> bool {
        self.last_content
            .terminal_mode
            .contains(TermMode::ALT_SCREEN)
    }

    /// Returns the current terminal mode flags.
    pub fn terminal_mode(&self) -> TermMode {
        self.last_content.terminal_mode
    }

    fn process_link_action(
        &mut self,
        terminal: &Term<EventProxy>,
        link_action: LinkAction,
        point: Point,
    ) {
        match link_action {
            LinkAction::Hover => {
                self.last_content.hovered_hyperlink =
                    self.regex_match_at(terminal, point, &mut self.url_regex.clone());
            }
            LinkAction::Clear => {
                self.last_content.hovered_hyperlink = None;
            }
            LinkAction::Open => {
                self.open_link();
            }
        };
    }

    fn open_link(&self) {
        if let Some(range) = &self.last_content.hovered_hyperlink {
            let start = range.start();
            let end = range.end();

            let mut url = String::from(self.last_content.grid.index(*start).c);
            for indexed in self.last_content.grid.iter_from(*start) {
                url.push(indexed.c);
                if indexed.point == *end {
                    break;
                }
            }

            if let Err(e) = open::that(url) {
                tracing::warn!("Failed to open link: {e}");
            }
        }
    }

    fn process_mouse_report(
        &self,
        button: MouseButton,
        modifiers: Modifiers,
        point: Point,
        pressed: bool,
    ) {
        let mut mods = 0;
        if modifiers.contains(Modifiers::SHIFT) {
            mods += 4;
        }
        if modifiers.contains(Modifiers::ALT) {
            mods += 8;
        }
        if modifiers.contains(Modifiers::COMMAND) {
            mods += 16;
        }

        match MouseMode::from(self.last_content().terminal_mode) {
            MouseMode::Sgr => self.sgr_mouse_report(point, button as u8 + mods, pressed),
            MouseMode::Normal(is_utf8) => {
                if pressed {
                    self.normal_mouse_report(point, button as u8 + mods, is_utf8)
                } else {
                    self.normal_mouse_report(point, 3 + mods, is_utf8)
                }
            }
        }
    }

    fn sgr_mouse_report(&self, point: Point, button: u8, pressed: bool) {
        let c = if pressed { 'M' } else { 'm' };

        let msg = format!(
            "\x1b[<{};{};{}{}",
            button,
            point.column + 1,
            point.line + 1,
            c
        );

        self.notifier.notify(msg.as_bytes().to_vec());
    }

    fn normal_mouse_report(&self, point: Point, button: u8, is_utf8: bool) {
        let Point { line, column } = point;
        let max_point = if is_utf8 { 2015 } else { 223 };

        if line >= max_point || column >= max_point {
            return;
        }

        let mut msg = vec![b'\x1b', b'[', b'M', 32 + button];

        let mouse_pos_encode = |pos: usize| -> Vec<u8> {
            let pos = 32 + 1 + pos;
            let first = 0xC0 + pos / 64;
            let second = 0x80 + (pos & 63);
            vec![first as u8, second as u8]
        };

        if is_utf8 && column >= Column(95) {
            msg.append(&mut mouse_pos_encode(column.0));
        } else {
            msg.push(32 + 1 + column.0 as u8);
        }

        if is_utf8 && line >= 95 {
            msg.append(&mut mouse_pos_encode(line.0 as usize));
        } else {
            msg.push(32 + 1 + line.0 as u8);
        }

        self.notifier.notify(msg);
    }

    fn start_selection(
        &mut self,
        terminal: &mut Term<EventProxy>,
        selection_type: SelectionType,
        x: f32,
        y: f32,
    ) {
        let location = Self::selection_point(x, y, &self.size, terminal.grid().display_offset());
        terminal.selection = Some(Selection::new(
            selection_type,
            location,
            self.selection_side(x),
        ));
    }

    fn update_selection(&mut self, terminal: &mut Term<EventProxy>, x: f32, y: f32) {
        let display_offset = terminal.grid().display_offset();
        if let Some(ref mut selection) = terminal.selection {
            let location = Self::selection_point(x, y, &self.size, display_offset);
            selection.update(location, self.selection_side(x));
        }
    }

    fn selection_side(&self, x: f32) -> Side {
        let cell_x = x as usize % self.size.cell_width as usize;
        let half_cell_width = (self.size.cell_width as f32 / 2.0) as usize;

        if cell_x > half_cell_width {
            Side::Right
        } else {
            Side::Left
        }
    }

    fn resize(&mut self, terminal: &mut Term<EventProxy>, layout_size: Size, font_size: Size) {
        if layout_size == self.size.layout_size
            && font_size.width as u16 == self.size.cell_width
            && font_size.height as u16 == self.size.cell_height
        {
            return;
        }

        let lines = (layout_size.height / font_size.height.floor()) as u16;
        let cols = (layout_size.width / font_size.width.floor()) as u16;
        if lines > 0 && cols > 0 {
            self.size = TerminalSize {
                layout_size,
                cell_height: font_size.height as u16,
                cell_width: font_size.width as u16,
                num_lines: lines,
                num_cols: cols,
            };

            self.notifier.on_resize(self.size.into());
            terminal.resize(TermSize::new(
                self.size.num_cols as usize,
                self.size.num_lines as usize,
            ));
        }
    }

    fn write<I: Into<Cow<'static, [u8]>>>(&self, input: I) {
        self.notifier.notify(input);
    }

    fn scroll(&mut self, terminal: &mut Term<EventProxy>, delta_value: i32) {
        if delta_value != 0 {
            let scroll = Scroll::Delta(delta_value);
            if terminal
                .mode()
                .contains(TermMode::ALTERNATE_SCROLL | TermMode::ALT_SCREEN)
            {
                let line_cmd = if delta_value > 0 { b'A' } else { b'B' };
                let mut content = vec![];

                for _ in 0..delta_value.abs() {
                    content.push(0x1b);
                    content.push(b'O');
                    content.push(line_cmd);
                }

                self.notifier.notify(content);
            } else {
                terminal.grid_mut().scroll_display(scroll);
            }
        }
    }

    /// Based on alacritty/src/display/hint.rs > regex_match_at
    /// Retrieve the match, if the specified point is inside the content matching the regex.
    fn regex_match_at(
        &self,
        terminal: &Term<EventProxy>,
        point: Point,
        regex: &mut RegexSearch,
    ) -> Option<Match> {
        let x = visible_regex_match_iter(terminal, regex).find(|rm| rm.contains(&point));
        x
    }
}

/// Copied from alacritty/src/display/hint.rs:
/// Iterate over all visible regex matches.
fn visible_regex_match_iter<'a>(
    term: &'a Term<EventProxy>,
    regex: &'a mut RegexSearch,
) -> impl Iterator<Item = Match> + 'a {
    let viewport_start = Line(-(term.grid().display_offset() as i32));
    let viewport_end = viewport_start + term.bottommost_line();
    let mut start = term.line_search_left(Point::new(viewport_start, Column(0)));
    let mut end = term.line_search_right(Point::new(viewport_end, Column(0)));
    start.line = start.line.max(viewport_start - 100);
    end.line = end.line.min(viewport_end + 100);

    RegexIter::new(start, end, Direction::Right, term, regex)
        .skip_while(move |rm| rm.end().line < viewport_start)
        .take_while(move |rm| rm.start().line <= viewport_end)
}

pub struct RenderableContent {
    pub grid: Grid<Cell>,
    pub hovered_hyperlink: Option<RangeInclusive<Point>>,
    pub selectable_range: Option<SelectionRange>,
    pub cursor: Cell,
    pub terminal_mode: TermMode,
    pub terminal_size: TerminalSize,
    /// Monotonically increasing counter; incremented on each PTY write event.
    pub generation: u64,
    /// Active search match (highlighted distinctly).
    pub search_match: Option<RangeInclusive<Point>>,
}

impl Default for RenderableContent {
    fn default() -> Self {
        Self {
            grid: Grid::new(0, 0, 0),
            hovered_hyperlink: None,
            selectable_range: None,
            cursor: Cell::default(),
            terminal_mode: TermMode::empty(),
            terminal_size: TerminalSize::default(),
            generation: 0,
            search_match: None,
        }
    }
}

impl Drop for TerminalBackend {
    fn drop(&mut self) {
        let _ = self.notifier.0.send(Msg::Shutdown);
    }
}

#[derive(Clone)]
pub struct EventProxy(mpsc::Sender<Event>);

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        let _ = self.0.send(event.clone());
    }
}

// ---------------------------------------------------------------------------
// Styled output types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct StyledSegment {
    pub text: String,
    pub fg: egui::Color32,
    pub bold: bool,
}

#[derive(Debug, Clone)]
pub struct StyledLine {
    pub segments: Vec<StyledSegment>,
}

impl StyledLine {
    pub fn plain_text(&self) -> String {
        self.segments.iter().map(|s| s.text.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// PID-based CWD resolution
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn current_cwd_for_pid(pid: u32) -> Option<std::path::PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
}

#[cfg(target_os = "macos")]
fn current_cwd_for_pid(pid: u32) -> Option<std::path::PathBuf> {
    let output = std::process::Command::new("lsof")
        .args(["-a", "-d", "cwd", "-p", &pid.to_string(), "-Fn"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Some(path) = line.strip_prefix('n') {
            let p = std::path::PathBuf::from(path);
            if p.is_absolute() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn current_cwd_for_pid(_pid: u32) -> Option<std::path::PathBuf> {
    None
}
