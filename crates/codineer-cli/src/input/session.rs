use std::borrow::Cow;
use std::io::Write;

use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType};

use super::text::{
    is_vim_toggle, line_end, line_start, move_vertical, next_boundary, previous_boundary,
    previous_command_boundary, remove_previous_char, render_selected_text, selection_bounds, to_u16,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOutcome {
    Submit(String),
    Cancel,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EditorMode {
    Plain,
    Insert,
    Normal,
    Visual,
    Command,
}

impl EditorMode {
    pub(super) fn indicator(self, vim_enabled: bool) -> Option<&'static str> {
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
pub(super) struct YankBuffer {
    pub(super) text: String,
    pub(super) linewise: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditSession {
    pub(super) text: String,
    pub(super) cursor: usize,
    pub(super) mode: EditorMode,
    pub(super) pending_operator: Option<char>,
    pub(super) visual_anchor: Option<usize>,
    pub(super) command_buffer: String,
    pub(super) command_cursor: usize,
    pub(super) history_index: Option<usize>,
    pub(super) history_backup: Option<String>,
    rendered_cursor_row: usize,
    rendered_lines: usize,
}

impl EditSession {
    pub(super) fn new(vim_enabled: bool) -> Self {
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

    pub(super) fn active_text(&self) -> &str {
        if self.mode == EditorMode::Command {
            &self.command_buffer
        } else {
            &self.text
        }
    }

    pub(super) fn current_len(&self) -> usize {
        self.active_text().len()
    }

    pub(super) fn has_input(&self) -> bool {
        !self.active_text().is_empty()
    }

    pub(super) fn current_line(&self) -> String {
        self.active_text().to_string()
    }

    pub(super) fn set_text_from_history(&mut self, entry: String) {
        self.text = entry;
        self.cursor = self.text.len();
        self.pending_operator = None;
        self.visual_anchor = None;
        if self.mode != EditorMode::Plain && self.mode != EditorMode::Insert {
            self.mode = EditorMode::Normal;
        }
    }

    pub(super) fn enter_insert_mode(&mut self) {
        self.mode = EditorMode::Insert;
        self.pending_operator = None;
        self.visual_anchor = None;
    }

    pub(super) fn enter_normal_mode(&mut self) {
        self.mode = EditorMode::Normal;
        self.pending_operator = None;
        self.visual_anchor = None;
    }

    pub(super) fn enter_visual_mode(&mut self) {
        self.mode = EditorMode::Visual;
        self.pending_operator = None;
        self.visual_anchor = Some(self.cursor);
    }

    pub(super) fn enter_command_mode(&mut self) {
        self.mode = EditorMode::Command;
        self.pending_operator = None;
        self.visual_anchor = None;
        self.command_buffer.clear();
        self.command_buffer.push(':');
        self.command_cursor = self.command_buffer.len();
    }

    pub(super) fn exit_command_mode(&mut self) {
        self.command_buffer.clear();
        self.command_cursor = 0;
        self.enter_normal_mode();
    }

    pub(super) fn visible_buffer(&self) -> Cow<'_, str> {
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

    pub(super) fn prompt<'a>(&self, base_prompt: &'a str, vim_enabled: bool) -> Cow<'a, str> {
        match self.mode.indicator(vim_enabled) {
            Some(mode) => Cow::Owned(format!("[{mode}] {base_prompt}")),
            None => Cow::Borrowed(base_prompt),
        }
    }

    pub(super) fn clear_render(&self, out: &mut impl Write) -> std::io::Result<()> {
        if self.rendered_cursor_row > 0 {
            queue!(out, MoveUp(to_u16(self.rendered_cursor_row)?))?;
        }
        queue!(out, MoveToColumn(0), Clear(ClearType::FromCursorDown))?;
        out.flush()
    }

    pub(super) fn render(
        &mut self,
        out: &mut impl Write,
        base_prompt: &str,
        vim_enabled: bool,
    ) -> std::io::Result<()> {
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

    pub(super) fn finalize_render(
        &self,
        out: &mut impl Write,
        base_prompt: &str,
        vim_enabled: bool,
    ) -> std::io::Result<()> {
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

    pub(super) fn handle_escape(&mut self) -> KeyAction {
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

    pub(super) fn handle_backspace(&mut self) {
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

    pub(super) fn submit_or_toggle(&self) -> KeyAction {
        let line = self.current_line();
        if is_vim_toggle(&line) {
            KeyAction::ToggleVim
        } else {
            KeyAction::Submit(line)
        }
    }

    pub(super) fn insert_char(&mut self, ch: char) {
        let mut buffer = [0; 4];
        self.insert_text(ch.encode_utf8(&mut buffer));
    }

    pub(super) fn insert_text(&mut self, text: &str) {
        if self.mode == EditorMode::Command {
            self.command_buffer.insert_str(self.command_cursor, text);
            self.command_cursor += text.len();
        } else {
            self.text.insert_str(self.cursor, text);
            self.cursor += text.len();
        }
    }

    pub(super) fn move_left(&mut self) {
        if self.mode == EditorMode::Command {
            self.command_cursor =
                previous_command_boundary(&self.command_buffer, self.command_cursor);
        } else {
            self.cursor = previous_boundary(&self.text, self.cursor);
        }
    }

    pub(super) fn move_right(&mut self) {
        if self.mode == EditorMode::Command {
            self.command_cursor = next_boundary(&self.command_buffer, self.command_cursor);
        } else {
            self.cursor = next_boundary(&self.text, self.cursor);
        }
    }

    pub(super) fn move_line_start(&mut self) {
        if self.mode == EditorMode::Command {
            self.command_cursor = 1;
        } else {
            self.cursor = line_start(&self.text, self.cursor);
        }
    }

    pub(super) fn move_line_end(&mut self) {
        if self.mode == EditorMode::Command {
            self.command_cursor = self.command_buffer.len();
        } else {
            self.cursor = line_end(&self.text, self.cursor);
        }
    }

    pub(super) fn move_up(&mut self) {
        if self.mode == EditorMode::Command {
            return;
        }
        self.cursor = move_vertical(&self.text, self.cursor, -1);
    }

    pub(super) fn move_down(&mut self) {
        if self.mode == EditorMode::Command {
            return;
        }
        self.cursor = move_vertical(&self.text, self.cursor, 1);
    }

    pub(super) fn delete_char_under_cursor(&mut self) {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum KeyAction {
    Continue,
    Submit(String),
    Cancel,
    Exit,
    ToggleVim,
}
