use std::borrow::Cow;
use std::io::Write;

use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::queue;
use crossterm::terminal::{self, Clear, ClearType};

use super::suggestions::SuggestionState;

use super::text::{
    is_vim_toggle, line_end, line_start, move_vertical, next_boundary, previous_boundary,
    previous_command_boundary, remove_previous_char, render_selected_text, selection_bounds,
    to_u16,
};
use crate::style::Palette;
use crate::terminal_width::strip_ansi;
use unicode_width::UnicodeWidthStr;

/// Write a full-width dim separator line (`─`) without trailing newline.
pub(super) fn write_dim_separator(out: &mut impl Write, cols: usize) -> std::io::Result<()> {
    let p = Palette::for_stdout();
    if p.dim.is_empty() {
        write!(out, "{}", "─".repeat(cols))
    } else {
        write!(out, "{}{}{}", p.dim, "─".repeat(cols), p.r)
    }
}

/// Write `content` on a `prompt_bg` background padded to `cols` width.
fn write_bg_padded(
    out: &mut impl Write,
    p: &Palette,
    content: &str,
    cols: usize,
) -> std::io::Result<()> {
    let pad = cols.saturating_sub(content.width());
    write!(out, "{}{content}{}{}", p.prompt_bg, " ".repeat(pad), p.r)
}

/// Raw image bytes captured from clipboard or drag-and-drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageData {
    pub bytes: Vec<u8>,
    pub media_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitPayload {
    pub text: String,
    pub images: Vec<ImageData>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOutcome {
    Submit(SubmitPayload),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum YankShape {
    #[default]
    Charwise,
    Linewise,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct YankBuffer {
    pub(super) text: String,
    pub(super) shape: YankShape,
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
    /// When true, render a "Press Ctrl-C again to exit" hint below the prompt.
    /// Cleared on the next re-render that doesn't set it.
    pub(super) show_interrupt_hint: bool,
    /// One-shot status message rendered in the info area (e.g. "no image in
    /// clipboard").  Cleared at the start of the next key-event handler so it
    /// disappears after the user presses any key.
    pub(super) transient_status: Option<String>,
    /// When true, render a bottom separator line below the input text and above
    /// the info/panel area.  Set from `LineEditor::show_separator`.
    pub(super) show_bottom_sep: bool,
    /// Persistent one-line hint shown in the info area below the bottom
    /// separator whenever no other panel (?, interrupt, suggestions) is active.
    /// Set from `LineEditor::hint_line` when the session is created.
    pub(super) static_hint: Option<String>,
    pub(super) history_index: Option<usize>,
    pub(super) history_backup: Option<String>,
    rendered_cursor_row: usize,
    rendered_lines: usize,
    /// Visible character width of each prefix line rendered above the prompt
    /// (banner + help text + separator). Used to compute reflow-adjusted line
    /// counts when the terminal is resized.
    pub(super) prefix_line_widths: Vec<usize>,
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
            show_interrupt_hint: false,
            transient_status: None,
            show_bottom_sep: false,
            static_hint: None,
            rendered_cursor_row: 0,
            rendered_lines: 1,
            prefix_line_widths: Vec::new(),
        }
    }

    pub(super) fn prefix_lines(&self) -> usize {
        self.prefix_line_widths.len()
    }

    /// How many terminal rows the prefix occupies at `new_cols` width,
    /// accounting for line wrapping (reflow) after a resize.
    pub(super) fn prefix_reflow_lines(&self, new_cols: usize) -> usize {
        let c = new_cols.max(1);
        self.prefix_line_widths
            .iter()
            .map(|&w| if w == 0 { 1 } else { w.div_ceil(c) })
            .sum()
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

    /// Erase the currently rendered area.
    ///
    /// `extra_lines_above`: number of lines above the prompt to also erase.
    /// Pass `0` for prompt-only, `prefix_lines` for normal prefix clear, or
    /// a larger value to account for terminal reflow after resize.
    pub(super) fn clear_render(
        &self,
        out: &mut impl Write,
        extra_lines_above: usize,
    ) -> std::io::Result<()> {
        let up = self.rendered_cursor_row + extra_lines_above;
        if up > 0 {
            queue!(out, MoveUp(to_u16(up)?))?;
        }
        queue!(out, MoveToColumn(0), Clear(ClearType::FromCursorDown))?;
        out.flush()
    }

    /// Render prompt + buffer + optional bottom separator + info panel.
    ///
    /// Layout:
    /// ```text
    /// ❯ <input text, potentially multi-line>
    /// ──────────────────────────────────────  ← bottom separator (when show_bottom_sep)
    /// <info area: ? panel / hint / suggestions>
    /// ```
    pub(super) fn render_content(
        &mut self,
        out: &mut impl Write,
        base_prompt: &str,
        vim_enabled: bool,
        suggestions: Option<&SuggestionState>,
    ) -> std::io::Result<()> {
        let prompt = self.prompt(base_prompt, vim_enabled);
        let buffer = self.visible_buffer();
        // In raw mode `\n` only moves the cursor down without resetting the
        // column.  Convert to `\r\n` AND indent continuation lines by the
        // visible prompt width so they are visually aligned with the first
        // line's content area (matching Claude Code's multi-line appearance).
        let prompt_display_width = strip_ansi(prompt.as_ref()).width();
        let indent = " ".repeat(prompt_display_width);
        let display_buffer = buffer.replace('\n', &format!("\r\n{indent}"));
        write!(out, "{prompt}{display_buffer}")?;

        let (cursor_row, cursor_col, total_lines) = self.cursor_layout(prompt.as_ref());

        let sep_lines = if self.show_bottom_sep {
            let (cols, _) = terminal::size().unwrap_or((80, 24));
            write!(out, "\r\n")?;
            write_dim_separator(out, cols as usize)?;
            1usize
        } else {
            0
        };

        // Info/panel area rendered below the separator.
        let panel_lines = if self.text == "?" {
            self.draw_shortcuts_panel(out)?
        } else if self.show_interrupt_hint {
            self.draw_interrupt_hint(out)?
        } else if let Some(state) = suggestions.filter(|s| !s.items.is_empty()) {
            self.draw_suggestions(out, state)?
        } else if let Some(ref status) = self.transient_status {
            let p = Palette::for_stdout();
            write!(out, "\r\n{}{status}{}", p.dim, p.r)?;
            1
        } else if self.show_bottom_sep {
            // Static hint line — shown whenever the info area is otherwise empty.
            if let Some(ref hint) = self.static_hint {
                write!(out, "\r\n{hint}")?;
                1
            } else {
                0
            }
        } else {
            0
        };

        let rows_below = total_lines.saturating_sub(cursor_row + 1) + sep_lines + panel_lines;
        if rows_below > 0 {
            queue!(out, MoveUp(to_u16(rows_below)?))?;
        }
        queue!(out, MoveToColumn(to_u16(cursor_col)?))?;
        out.flush()?;

        self.rendered_cursor_row = cursor_row;
        self.rendered_lines = total_lines;
        Ok(())
    }

    /// Clear prompt area only, then re-render prompt + buffer + suggestions.
    pub(super) fn render_with_suggestions(
        &mut self,
        out: &mut impl Write,
        base_prompt: &str,
        vim_enabled: bool,
        suggestions: Option<&SuggestionState>,
    ) -> std::io::Result<()> {
        self.clear_render(out, 0)?;
        self.render_content(out, base_prompt, vim_enabled, suggestions)
    }

    fn draw_suggestions(
        &self,
        out: &mut impl Write,
        state: &SuggestionState,
    ) -> std::io::Result<usize> {
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let cols = cols as usize;
        let max_visible = std::cmp::min(6, (rows as usize).saturating_sub(3)).max(1);

        let total = state.items.len();
        let start = if total <= max_visible {
            0
        } else {
            let center = max_visible / 2;
            state
                .selected
                .saturating_sub(center)
                .min(total - max_visible)
        };
        let end = (start + max_visible).min(total);

        let p = Palette::for_stdout();

        let name_col = (cols * 40 / 100).clamp(12, 30);

        for (i, item) in state.items[start..end].iter().enumerate() {
            let idx = start + i;
            let is_selected = idx == state.selected;
            let display_w = crate::terminal_width::display_width(&item.display);
            let pad = name_col.saturating_sub(display_w);

            if is_selected {
                write!(out, "\r\n  {}{}{}", p.violet, item.display, p.r)?;
            } else {
                write!(out, "\r\n  {}", item.display)?;
            }

            if !item.description.is_empty() {
                let desc_max = cols.saturating_sub(name_col + 4);
                let desc = if item.description.len() > desc_max {
                    &item.description[..desc_max]
                } else {
                    &item.description
                };
                let color = if is_selected { p.violet } else { p.dim };
                write!(out, "{}{color}{desc}{}", " ".repeat(pad), p.r)?;
            }
        }

        Ok(end - start)
    }

    /// Draw the keyboard shortcuts panel in the info area (shown when the
    /// user types `?` as the sole character).  Returns the number of terminal
    /// lines drawn so the cursor can be repositioned correctly.
    fn draw_shortcuts_panel(&self, out: &mut impl Write) -> std::io::Result<usize> {
        let (cols, _) = terminal::size().unwrap_or((80, 24));
        let p = Palette::for_stdout();

        const SHORTCUTS: &[(&str, &str)] = &[
            ("! for bash mode", "double tap esc to clear input"),
            ("/ for commands", "shift + enter for newline"),
            ("@ for file paths", "ctrl + j for newline"),
            ("\\ + enter for newline", "ctrl + c to clear input"),
            ("/vim to toggle vim mode", "ctrl + d to exit"),
            ("? for shortcuts", "/help for full command list"),
        ];

        let left_w = (cols as usize / 2).max(20);
        for &(left, right) in SHORTCUTS {
            write!(
                out,
                "\r\n{}  {:<w$}{right}{}",
                p.dim,
                left,
                p.r,
                w = left_w - 2
            )?;
        }
        Ok(SHORTCUTS.len())
    }

    fn draw_interrupt_hint(&self, out: &mut impl Write) -> std::io::Result<usize> {
        let p = Palette::for_stdout();
        write!(out, "\r\n{}Press Ctrl-C again to exit{}", p.dim, p.r,)?;
        Ok(1)
    }

    pub(super) fn finalize_render(
        &self,
        out: &mut impl Write,
        base_prompt: &str,
        vim_enabled: bool,
    ) -> std::io::Result<()> {
        self.clear_render(out, 0)?;
        let p = Palette::for_stdout();
        let prompt = self.prompt(base_prompt, vim_enabled);
        let prompt_plain = strip_ansi(&prompt);
        let indent = " ".repeat(prompt_plain.width());
        let buf = self.visible_buffer();
        let (cols, _) = terminal::size().unwrap_or((80, 24));
        let cols = cols as usize;
        // Each visual line gets a dark-gray background bar (Claude Code style).
        let mut lines = buf.lines();
        write_bg_padded(
            out,
            &p,
            &format!("{prompt_plain}{}", lines.next().unwrap_or("")),
            cols,
        )?;
        for line in lines {
            write!(out, "\r\n")?;
            write_bg_padded(out, &p, &format!("{indent}{line}"), cols)?;
        }
        write!(out, "\r\n")?;
        out.flush()
    }

    fn cursor_layout(&self, prompt: &str) -> (usize, usize, usize) {
        let active_text = self.active_text();
        let cursor = if self.mode == EditorMode::Command {
            self.command_cursor
        } else {
            self.cursor
        };

        let mut cursor = cursor.min(active_text.len());
        while !active_text.is_char_boundary(cursor) && cursor > 0 {
            cursor -= 1;
        }
        let cursor_prefix = &active_text[..cursor];
        let prompt_w = strip_ansi(prompt).width();
        let cursor_row = cursor_prefix.bytes().filter(|byte| *byte == b'\n').count();
        let cursor_col = match cursor_prefix.rsplit_once('\n') {
            // Continuation lines are indented by `prompt_w` spaces so the
            // cursor column must include that indent.
            Some((_, suffix)) => prompt_w + strip_ansi(suffix).width(),
            None => prompt_w + strip_ansi(cursor_prefix).width(),
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
    /// First Ctrl+C on an empty prompt — show "Press Ctrl-C again to exit".
    InterruptHint,
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with_prefix(widths: &[usize]) -> EditSession {
        let mut s = EditSession::new(false);
        s.prefix_line_widths = widths.to_vec();
        s
    }

    // ── prefix_lines ─────────────────────────────────────────────────────────

    #[test]
    fn prefix_lines_empty() {
        let s = EditSession::new(false);
        assert_eq!(s.prefix_lines(), 0);
    }

    #[test]
    fn prefix_lines_counts_elements() {
        let s = session_with_prefix(&[80, 80, 80]);
        assert_eq!(s.prefix_lines(), 3);
    }

    // ── prefix_reflow_lines ──────────────────────────────────────────────────

    #[test]
    fn reflow_empty_prefix_is_zero() {
        let s = EditSession::new(false);
        assert_eq!(s.prefix_reflow_lines(80), 0);
    }

    #[test]
    fn reflow_lines_fit_without_wrap() {
        // Three lines each 80 cols wide; terminal also 80 cols → no wrapping.
        let s = session_with_prefix(&[80, 80, 80]);
        assert_eq!(s.prefix_reflow_lines(80), 3);
    }

    #[test]
    fn reflow_single_wide_line_wraps_to_two() {
        // A 160-char separator at 80 cols → ceil(160/80) = 2 rows.
        let s = session_with_prefix(&[160]);
        assert_eq!(s.prefix_reflow_lines(80), 2);
    }

    #[test]
    fn reflow_line_exactly_fits_counts_as_one() {
        let s = session_with_prefix(&[80]);
        assert_eq!(s.prefix_reflow_lines(80), 1);
    }

    #[test]
    fn reflow_zero_width_line_counts_as_one() {
        // An empty (zero-width) line still occupies one terminal row.
        let s = session_with_prefix(&[0]);
        assert_eq!(s.prefix_reflow_lines(80), 1);
    }

    #[test]
    fn reflow_mixed_widths() {
        // widths=[80, 160, 0] at 80 cols → 1 + 2 + 1 = 4 rows.
        let s = session_with_prefix(&[80, 160, 0]);
        assert_eq!(s.prefix_reflow_lines(80), 4);
    }

    #[test]
    fn reflow_narrow_terminal_increases_rows() {
        // A 120-char line at 40 cols → ceil(120/40) = 3 rows.
        let s = session_with_prefix(&[120]);
        assert_eq!(s.prefix_reflow_lines(40), 3);
    }

    #[test]
    fn reflow_wide_terminal_decreases_rows() {
        // A line that was 160 chars at old width; new terminal is 200 cols → 1 row.
        let s = session_with_prefix(&[160]);
        assert_eq!(s.prefix_reflow_lines(200), 1);
    }

    #[test]
    fn reflow_cols_zero_treated_as_one() {
        // Guard against division by zero — cols=0 is clamped to 1.
        let s = session_with_prefix(&[80]);
        // 80 / 1 = 80 rows
        assert_eq!(s.prefix_reflow_lines(0), 80);
    }

    // ── shortcuts panel ─────────────────────────────────────────────────────

    #[test]
    fn shortcuts_panel_draws_expected_lines() {
        let s = EditSession::new(false);
        let mut buf = Vec::new();
        let lines = s.draw_shortcuts_panel(&mut buf).unwrap();
        // 6 shortcut rows (separator is now drawn by render_content, not the panel)
        assert_eq!(lines, 6);
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("! for bash mode"),
            "panel should contain bash mode shortcut"
        );
        assert!(
            output.contains("? for shortcuts"),
            "panel should contain ? shortcut"
        );
    }

    // ── KeyAction variants ──────────────────────────────────────────────────

    #[test]
    fn key_action_interrupt_hint_is_distinct() {
        let hint = KeyAction::InterruptHint;
        assert_ne!(hint, KeyAction::Exit);
        assert_ne!(hint, KeyAction::Cancel);
        assert_ne!(hint, KeyAction::Continue);
    }

    // ── submit_or_toggle ────────────────────────────────────────────────────

    #[test]
    fn submit_or_toggle_detects_vim() {
        let mut s = EditSession::new(false);
        s.text = "/vim".to_string();
        s.cursor = s.text.len();
        assert_eq!(s.submit_or_toggle(), KeyAction::ToggleVim);
    }

    #[test]
    fn submit_or_toggle_submits_normal_text() {
        let mut s = EditSession::new(false);
        s.text = "hello".to_string();
        s.cursor = s.text.len();
        assert_eq!(s.submit_or_toggle(), KeyAction::Submit("hello".to_string()));
    }
}
