pub mod mappings;
pub mod pty;

use std::path::PathBuf;

/// Framework-agnostic terminal content snapshot.
/// Extracted from the alacritty_terminal grid for UI rendering.
#[derive(Debug, Clone)]
pub struct TerminalContent {
    pub cells: Vec<TerminalCell>,
    pub display_offset: usize,
    pub cursor: CursorPosition,
    pub columns: usize,
    pub lines: usize,
    pub title: String,
    pub mode: TerminalMode,
}

#[derive(Debug, Clone)]
pub struct TerminalCell {
    pub c: char,
    pub fg: TerminalColor,
    pub bg: TerminalColor,
    pub flags: CellFlags,
    pub column: usize,
    pub line: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct CursorPosition {
    pub line: usize,
    pub column: usize,
    pub shape: CursorShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Bar,
    Underline,
}

#[derive(Debug, Clone, Copy)]
pub enum TerminalColor {
    Named(NamedColor),
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy)]
pub enum NamedColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Foreground,
    Background,
    Cursor,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CellFlags {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub inverse: bool,
    pub dim: bool,
    pub hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMode {
    Normal,
    AlternateScreen,
}

#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("Failed to create PTY: {0}")]
    PtyCreation(String),
    #[error("PTY I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Terminal configuration (framework-agnostic)
#[derive(Debug, Clone)]
pub struct TerminalConfig {
    pub shell_path: Option<String>,
    pub shell_args: Vec<String>,
    pub working_dir: PathBuf,
    pub env: std::collections::HashMap<String, String>,
    pub columns: u16,
    pub lines: u16,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            shell_path: None,
            shell_args: Vec::new(),
            working_dir: PathBuf::from("."),
            env: std::collections::HashMap::new(),
            columns: 80,
            lines: 24,
        }
    }
}
