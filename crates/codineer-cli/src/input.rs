use std::borrow::Cow;
use std::io::{self, IsTerminal, Write};

use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::queue;
use crossterm::terminal::{self, Clear, ClearType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOutcome {
    Submit(String),
    Cancel,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorMode {
    Plain,
    Insert,
    Normal,
    Visual,
    Command,
}

impl EditorMode {
    fn indicator(self, vim_enabled: bool) -> Option<&'static str> {
        if !vim_enabled {
            return None;
        }

        Some(match self {
            Self::Plain => "PLAIN",
            Self::Insert => "INSERT",
            Self::Normal => "NORMAL",
            Self::Visual => "VISUAL",
            Self::Command => "COMMAND",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct YankBuffer {
    text: String,
    linewise: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditSession {
    text: String,
    cursor: usize,
    mode: EditorMode,
    pending_operator: Option<char>,
    visual_anchor: Option<usize>,
    command_buffer: String,
    command_cursor: usize,
    history_index: Option<usize>,
    history_backup: Option<String>,
    rendered_cursor_row: usize,
    rendered_lines: usize,
}

impl EditSession {
    fn new(vim_enabled: bool) -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            mode: if vim_enabled {
                EditorMode::Insert
            } else {
                EditorMode::Plain
            },
            pending_operator: None,
            visual_anchor: None,
            command_buffer: String::new(),
            command_cursor: 0,
            history_index: None,
            history_backup: None,
            rendered_cursor_row: 0,
            rendered_lines: 1,
        }
    }

    fn active_text(&self) -> &str {
        if self.mode == EditorMode::Command {
            &self.command_buffer
        } else {
            &self.text
        }
    }

    fn current_len(&self) -> usize {
        self.active_text().len()
    }

    fn has_input(&self) -> bool {
        !self.active_text().is_empty()
    }

    fn current_line(&self) -> String {
        self.active_text().to_string()
    }

    fn set_text_from_history(&mut self, entry: String) {
        self.text = entry;
        self.cursor = self.text.len();
        self.pending_operator = None;
        self.visual_anchor = None;
        if self.mode != EditorMode::Plain && self.mode != EditorMode::Insert {
            self.mode = EditorMode::Normal;
        }
    }

    fn enter_insert_mode(&mut self) {
        self.mode = EditorMode::Insert;
        self.pending_operator = None;
        self.visual_anchor = None;
    }

    fn enter_normal_mode(&mut self) {
        self.mode = EditorMode::Normal;
        self.pending_operator = None;
        self.visual_anchor = None;
    }

    fn enter_visual_mode(&mut self) {
        self.mode = EditorMode::Visual;
        self.pending_operator = None;
        self.visual_anchor = Some(self.cursor);
    }

    fn enter_command_mode(&mut self) {
        self.mode = EditorMode::Command;
        self.pending_operator = None;
        self.visual_anchor = None;
        self.command_buffer.clear();
        self.command_buffer.push(':');
        self.command_cursor = self.command_buffer.len();
    }

    fn exit_command_mode(&mut self) {
        self.command_buffer.clear();
        self.command_cursor = 0;
        self.enter_normal_mode();
    }

    fn visible_buffer(&self) -> Cow<'_, str> {
        if self.mode != EditorMode::Visual {
            return Cow::Borrowed(self.active_text());
        }

        let Some(anchor) = self.visual_anchor else {
            return Cow::Borrowed(self.active_text());
        };
        let Some((start, end)) = selection_bounds(&self.text, anchor, self.cursor) else {
            return Cow::Borrowed(self.active_text());
        };

        Cow::Owned(render_selected_text(&self.text, start, end))
    }

    fn prompt<'a>(&self, base_prompt: &'a str, vim_enabled: bool) -> Cow<'a, str> {
        match self.mode.indicator(vim_enabled) {
            Some(mode) => Cow::Owned(format!("[{mode}] {base_prompt}")),
            None => Cow::Borrowed(base_prompt),
        }
    }

    fn clear_render(&self, out: &mut impl Write) -> io::Result<()> {
        if self.rendered_cursor_row > 0 {
            queue!(out, MoveUp(to_u16(self.rendered_cursor_row)?))?;
        }
        queue!(out, MoveToColumn(0), Clear(ClearType::FromCursorDown))?;
        out.flush()
    }

    fn render(
        &mut self,
        out: &mut impl Write,
        base_prompt: &str,
        vim_enabled: bool,
    ) -> io::Result<()> {
        self.clear_render(out)?;

        let prompt = self.prompt(base_prompt, vim_enabled);
        let buffer = self.visible_buffer();
        write!(out, "{prompt}{buffer}")?;

        let (cursor_row, cursor_col, total_lines) = self.cursor_layout(prompt.as_ref());
        let rows_to_move_up = total_lines.saturating_sub(cursor_row + 1);
        if rows_to_move_up > 0 {
            queue!(out, MoveUp(to_u16(rows_to_move_up)?))?;
        }
        queue!(out, MoveToColumn(to_u16(cursor_col)?))?;
        out.flush()?;

        self.rendered_cursor_row = cursor_row;
        self.rendered_lines = total_lines;
        Ok(())
    }

    fn finalize_render(
        &self,
        out: &mut impl Write,
        base_prompt: &str,
        vim_enabled: bool,
    ) -> io::Result<()> {
        self.clear_render(out)?;
        let prompt = self.prompt(base_prompt, vim_enabled);
        let buffer = self.visible_buffer();
        write!(out, "{prompt}{buffer}")?;
        writeln!(out)
    }

    fn cursor_layout(&self, prompt: &str) -> (usize, usize, usize) {
        let active_text = self.active_text();
        let cursor = if self.mode == EditorMode::Command {
            self.command_cursor
        } else {
            self.cursor
        };

        let cursor_prefix = &active_text[..cursor];
        let cursor_row = cursor_prefix.bytes().filter(|byte| *byte == b'\n').count();
        let cursor_col = match cursor_prefix.rsplit_once('\n') {
            Some((_, suffix)) => suffix.chars().count(),
            None => prompt.chars().count() + cursor_prefix.chars().count(),
        };
        let total_lines = active_text.bytes().filter(|byte| *byte == b'\n').count() + 1;
        (cursor_row, cursor_col, total_lines)
    }

    fn handle_escape(&mut self) -> KeyAction {
        match self.mode {
            EditorMode::Plain | EditorMode::Normal => KeyAction::Continue,
            EditorMode::Insert => {
                if self.cursor > 0 {
                    self.cursor = previous_boundary(&self.text, self.cursor);
                }
                self.enter_normal_mode();
                KeyAction::Continue
            }
            EditorMode::Visual => {
                self.enter_normal_mode();
                KeyAction::Continue
            }
            EditorMode::Command => {
                self.exit_command_mode();
                KeyAction::Continue
            }
        }
    }

    fn handle_backspace(&mut self) {
        match self.mode {
            EditorMode::Normal | EditorMode::Visual => self.move_left(),
            EditorMode::Command => {
                if self.command_cursor <= 1 {
                    self.exit_command_mode();
                } else {
                    remove_previous_char(&mut self.command_buffer, &mut self.command_cursor);
                }
            }
            EditorMode::Plain | EditorMode::Insert => {
                remove_previous_char(&mut self.text, &mut self.cursor);
            }
        }
    }

    fn submit_or_toggle(&self) -> KeyAction {
        let line = self.current_line();
        if is_vim_toggle(&line) {
            KeyAction::ToggleVim
        } else {
            KeyAction::Submit(line)
        }
    }

    fn insert_char(&mut self, ch: char) {
        let mut buffer = [0; 4];
        self.insert_text(ch.encode_utf8(&mut buffer));
    }

    fn insert_text(&mut self, text: &str) {
        if self.mode == EditorMode::Command {
            self.command_buffer.insert_str(self.command_cursor, text);
            self.command_cursor += text.len();
        } else {
            self.text.insert_str(self.cursor, text);
            self.cursor += text.len();
        }
    }

    fn move_left(&mut self) {
        if self.mode == EditorMode::Command {
            self.command_cursor =
                previous_command_boundary(&self.command_buffer, self.command_cursor);
        } else {
            self.cursor = previous_boundary(&self.text, self.cursor);
        }
    }

    fn move_right(&mut self) {
        if self.mode == EditorMode::Command {
            self.command_cursor = next_boundary(&self.command_buffer, self.command_cursor);
        } else {
            self.cursor = next_boundary(&self.text, self.cursor);
        }
    }

    fn move_line_start(&mut self) {
        if self.mode == EditorMode::Command {
            self.command_cursor = 1;
        } else {
            self.cursor = line_start(&self.text, self.cursor);
        }
    }

    fn move_line_end(&mut self) {
        if self.mode == EditorMode::Command {
            self.command_cursor = self.command_buffer.len();
        } else {
            self.cursor = line_end(&self.text, self.cursor);
        }
    }

    fn move_up(&mut self) {
        if self.mode == EditorMode::Command {
            return;
        }
        self.cursor = move_vertical(&self.text, self.cursor, -1);
    }

    fn move_down(&mut self) {
        if self.mode == EditorMode::Command {
            return;
        }
        self.cursor = move_vertical(&self.text, self.cursor, 1);
    }

    fn delete_char_under_cursor(&mut self) {
        match self.mode {
            EditorMode::Command => {
                if self.command_cursor < self.command_buffer.len() {
                    let end = next_boundary(&self.command_buffer, self.command_cursor);
                    self.command_buffer.drain(self.command_cursor..end);
                }
            }
            _ => {
                if self.cursor < self.text.len() {
                    let end = next_boundary(&self.text, self.cursor);
                    self.text.drain(self.cursor..end);
                }
            }
        }
    }
}

enum KeyAction {
    Continue,
    Submit(String),
    Cancel,
    Exit,
    ToggleVim,
}

pub struct LineEditor {
    prompt: String,
    completions: Vec<String>,
    history: Vec<String>,
    yank_buffer: YankBuffer,
    vim_enabled: bool,
    completion_state: Option<CompletionState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompletionState {
    prefix: String,
    matches: Vec<String>,
    next_index: usize,
}

impl LineEditor {
    #[must_use]
    pub fn new(prompt: impl Into<String>, completions: Vec<String>) -> Self {
        Self {
            prompt: prompt.into(),
            completions,
            history: Vec::new(),
            yank_buffer: YankBuffer::default(),
            vim_enabled: false,
            completion_state: None,
        }
    }

    pub fn push_history(&mut self, entry: impl Into<String>) {
        let entry = entry.into();
        if entry.trim().is_empty() {
            return;
        }

        self.history.push(entry);
    }

    pub fn read_line(&mut self) -> io::Result<ReadOutcome> {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
            return self.read_line_fallback();
        }

        let _raw_mode = RawModeGuard::new()?;
        let mut stdout = io::stdout();
        let mut session = EditSession::new(self.vim_enabled);
