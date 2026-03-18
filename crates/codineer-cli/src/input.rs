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

