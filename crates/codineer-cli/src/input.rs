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
