use std::io::{self, IsTerminal, Write};
use std::time::Instant;

use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::{execute, terminal};

use super::session::{EditSession, EditorMode, KeyAction, ReadOutcome, YankBuffer, YankShape};
use super::suggestions::{self, CommandEntry, SuggestionState, SuggestionTrigger};
use super::text::{current_line_delete_range, is_vim_toggle, line_end, next_boundary};

/// Maximum interval (ms) between two Ctrl+C or Esc presses to be considered a
/// "double-tap".
const DOUBLE_TAP_MS: u128 = 1500;

pub struct LineEditor {
    prompt: String,
    command_specs: Vec<CommandEntry>,
    pub(super) history: Vec<String>,
    yank_buffer: YankBuffer,
    pub(super) vim_enabled: bool,
    suggestion_state: Option<SuggestionState>,
    dismissed_for_input: Option<String>,
    show_separator: bool,
    /// Optional closure that generates prefix text (banner + help) above the
    /// separator. Called on each render so the output adapts to terminal width.
    /// Consumed after the first `read_line` call.
    prefix_fn: Option<Box<dyn Fn() -> String>>,
    /// Timestamp of the last Ctrl+C on an empty prompt — used for "press
    /// Ctrl-C again to exit" (Claude Code style).
    last_ctrlc: Option<Instant>,
    /// Timestamp of the last Esc press in Plain mode — used for double-tap Esc
    /// to clear input.
    last_esc: Option<Instant>,
}

impl LineEditor {
    #[must_use]
    pub fn new(prompt: impl Into<String>, command_specs: Vec<CommandEntry>) -> Self {
        Self {
            prompt: prompt.into(),
            command_specs,
            history: Vec::new(),
            yank_buffer: YankBuffer::default(),
            vim_enabled: false,
            suggestion_state: None,
            dismissed_for_input: None,
            show_separator: false,
            prefix_fn: None,
            last_ctrlc: None,
            last_esc: None,
        }
    }

    /// Enable a full-width separator line above the prompt that
    /// auto-adjusts on terminal resize.
    #[must_use]
    pub fn with_separator(mut self) -> Self {
        self.show_separator = true;
        self
    }

    /// Set a prefix renderer (e.g. banner + help text) shown above the
    /// separator on the first prompt. Re-generated on resize so the layout
    /// adapts to the current terminal width.
    #[must_use]
    pub fn with_prefix(mut self, f: impl Fn() -> String + 'static) -> Self {
        self.prefix_fn = Some(Box::new(f));
        self
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
        session.show_bottom_sep = self.show_separator;
        self.render_prefix(&mut session, &mut stdout, None)?;

        loop {
            let event = event::read()?;
            if let Event::Resize(w, _) = event {
                crate::terminal_width::update_terminal_cols(w as usize);
                self.render_prefix(&mut session, &mut stdout, self.suggestion_state.as_ref())?;
                continue;
            }
            // Bracketed paste: insert the whole text at once so newlines in the
            // pasted content don't accidentally trigger a submission.
            if let Event::Paste(ref text) = event {
                let text = text.clone();
                session.insert_text(&text);
                self.update_suggestions(&session);
                session.render_with_suggestions(
                    &mut stdout,
                    &self.prompt,
                    self.vim_enabled,
                    self.suggestion_state.as_ref(),
                )?;
                continue;
            }
            let Event::Key(key) = event else {
                continue;
            };
            if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                continue;
            }

            match self.handle_key_event(&mut session, key) {
                KeyAction::Continue => {
                    self.update_suggestions(&session);
                    session.render_with_suggestions(
                        &mut stdout,
                        &self.prompt,
                        self.vim_enabled,
                        self.suggestion_state.as_ref(),
                    )?;
                }
                KeyAction::Submit(line) => {
                    session.finalize_render(&mut stdout, &self.prompt, self.vim_enabled)?;
                    self.prefix_fn = None;
                    return Ok(ReadOutcome::Submit(line));
                }
                KeyAction::Cancel => {
                    session.clear_render(&mut stdout, session.prefix_lines())?;
                    // In raw mode `\n` only moves the cursor down without
                    // resetting the column; `\r\n` is required on all platforms
                    // (including Windows) to land at column 0.
                    write!(stdout, "\r\n")?;
                    stdout.flush()?;
                    self.prefix_fn = None;
                    return Ok(ReadOutcome::Cancel);
                }
                KeyAction::Exit => {
                    session.clear_render(&mut stdout, session.prefix_lines())?;
                    write!(stdout, "\r\n")?;
                    stdout.flush()?;
                    return Ok(ReadOutcome::Exit);
                }
                KeyAction::InterruptHint => {
                    session.show_interrupt_hint = true;
                    session.render_with_suggestions(
                        &mut stdout,
                        &self.prompt,
                        self.vim_enabled,
                        None,
                    )?;
                }
                KeyAction::ToggleVim => {
                    session.clear_render(&mut stdout, session.prefix_lines())?;
                    self.vim_enabled = !self.vim_enabled;
                    // The banner is only shown once (at startup).  Consuming
                    // prefix_fn here prevents it from reappearing every time
                    // the user toggles Vim mode.
                    self.prefix_fn = None;
                    write!(
                        stdout,
                        "Vim mode {}.\r\n",
                        if self.vim_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    )?;
                    stdout.flush()?;
                    session = EditSession::new(self.vim_enabled);
                    session.show_bottom_sep = self.show_separator;
                    self.render_prefix(&mut session, &mut stdout, None)?;
                }
            }
        }
    }

    /// Clear the entire prefix area (accounting for reflow), regenerate
    /// prefix content (banner + help + separator), then render prompt.
    fn render_prefix(
        &self,
        session: &mut EditSession,
        out: &mut impl Write,
        suggestions: Option<&SuggestionState>,
    ) -> io::Result<()> {
        let new_cols = crate::terminal_width::terminal_cols().max(1);
        let reflow = session.prefix_reflow_lines(new_cols);
        session.clear_render(out, reflow)?;

        session.prefix_line_widths.clear();

        if let Some(ref f) = self.prefix_fn {
            let text = f();
            for line in text.split('\n') {
                write!(out, "{line}\r\n")?;
                session
                    .prefix_line_widths
                    .push(crate::terminal_width::display_width(line));
            }
        }

        if self.show_separator {
            let p = crate::style::Palette::for_stdout();
            if p.violet.is_empty() {
                write!(out, "{}\r\n", "-".repeat(new_cols))?;
            } else {
                write!(out, "{}{}{}\r\n", p.violet, "─".repeat(new_cols), p.r)?;
            }
            session.prefix_line_widths.push(new_cols);
        }

        session.render_content(out, &self.prompt, self.vim_enabled, suggestions)
    }

    fn read_line_fallback(&mut self) -> io::Result<ReadOutcome> {
        loop {
            let mut stdout = io::stdout();
            write!(stdout, "{}", self.prompt)?;
            stdout.flush()?;

            let mut buffer = String::new();
            let bytes_read = io::stdin().read_line(&mut buffer)?;
            if bytes_read == 0 {
                return Ok(ReadOutcome::Exit);
            }

            while matches!(buffer.chars().last(), Some('\n' | '\r')) {
                buffer.pop();
            }

            if is_vim_toggle(&buffer) {
                self.vim_enabled = !self.vim_enabled;
                writeln!(
                    stdout,
                    "Vim mode {}.",
                    if self.vim_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                )?;
                continue;
            }

            return Ok(ReadOutcome::Submit(buffer));
        }
    }

    // --- Suggestion handling (Claude Code aligned) ---

    fn update_suggestions(&mut self, session: &EditSession) {
        if self.dismissed_for_input.as_deref() == Some(&session.text) {
            return;
        }
        if self.dismissed_for_input.is_some() {
            self.dismissed_for_input = None;
        }

        let prev = self
            .suggestion_state
            .as_ref()
            .map(|s| (s.trigger, s.items.len(), s.selected));

        if let Some(mut state) =
            suggestions::slash_suggestions(&session.text, session.cursor, &self.command_specs)
        {
            Self::restore_selection(&mut state, prev);
            self.suggestion_state = Some(state);
            return;
        }

        if session.mode != EditorMode::Command {
            if let Some(mut state) = suggestions::file_suggestions(&session.text, session.cursor) {
                Self::restore_selection(&mut state, prev);
                self.suggestion_state = Some(state);
                return;
            }
        }

        self.suggestion_state = None;
    }

    fn restore_selection(
        state: &mut SuggestionState,
        prev: Option<(SuggestionTrigger, usize, usize)>,
    ) {
        if let Some((trigger, count, idx)) = prev {
            if trigger == state.trigger && count == state.items.len() && idx < state.items.len() {
                state.selected = idx;
            }
        }
    }

    fn accept_suggestion(&mut self, session: &mut EditSession) {
        let Some(state) = self.suggestion_state.take() else {
            return;
        };
        let item = &state.items[state.selected];
        match state.trigger {
            SuggestionTrigger::Slash => {
                session.text = item.completion.clone();
                session.cursor = session.text.len();
            }
            SuggestionTrigger::At {
                token_start,
                token_len,
            } => {
                session
                    .text
                    .replace_range(token_start..token_start + token_len, &item.completion);
                session.cursor = token_start + item.completion.len();
            }
        }
    }

    fn accept_suggestion_and_maybe_submit(&mut self, session: &mut EditSession) -> KeyAction {
        let Some(state) = self.suggestion_state.take() else {
            return session.submit_or_toggle();
        };
        let item = state.items[state.selected].clone();

        match state.trigger {
            SuggestionTrigger::Slash => {
                session.text = item.completion.clone();
                session.cursor = session.text.len();
                if item.execute_on_enter {
                    KeyAction::Submit(session.text.trim().to_string())
                } else {
                    KeyAction::Continue
                }
            }
            SuggestionTrigger::At {
                token_start,
                token_len,
            } => {
                // On Enter, always append a space so the accepted token is
                // cleanly separated from any following text the user types.
                // Tab keeps the raw completion (allowing directory drill-down).
                let completion = ensure_trailing_space(&item.completion);
                session
                    .text
                    .replace_range(token_start..token_start + token_len, &completion);
                session.cursor = token_start + completion.len();
                KeyAction::Continue
            }
        }
    }

    // --- Key event dispatch ---

    pub(super) fn handle_key_event(
        &mut self,
        session: &mut EditSession,
        key: KeyEvent,
    ) -> KeyAction {
        // Clear the "Press Ctrl-C again" hint on every keystroke so it
        // disappears as soon as the user does anything else.
        session.show_interrupt_hint = false;

        // When suggestions are visible, intercept navigation keys
        if self.suggestion_state.is_some() {
            match key.code {
                KeyCode::Up if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(state) = &mut self.suggestion_state {
                        state.selected = if state.selected == 0 {
                            state.items.len() - 1
                        } else {
                            state.selected - 1
                        };
                    }
                    return KeyAction::Continue;
                }
                KeyCode::Down if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(state) = &mut self.suggestion_state {
                        state.selected = (state.selected + 1) % state.items.len();
                    }
                    return KeyAction::Continue;
                }
                KeyCode::Tab => {
                    self.accept_suggestion(session);
                    return KeyAction::Continue;
                }
                KeyCode::Enter if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return self.accept_suggestion_and_maybe_submit(session);
                }
                KeyCode::Esc => {
                    self.dismissed_for_input = Some(session.text.clone());
                    self.suggestion_state = None;
                    return KeyAction::Continue;
                }
                _ => {
                    // Fall through to normal handling; suggestions will be
                    // re-evaluated after the key is processed via update_suggestions.
                }
            }
        }

        // Any key other than the double-tap target resets that target's timer.
        let is_ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'C'));
        if !is_ctrl_c {
            self.last_ctrlc = None;
        }
        if key.code != KeyCode::Esc {
            self.last_esc = None;
        }

        // Normal key handling
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c' | 'C') => {
                    if session.has_input() {
                        return KeyAction::Cancel;
                    }
                    let now = Instant::now();
                    if self
                        .last_ctrlc
                        .is_some_and(|t| now.duration_since(t).as_millis() < DOUBLE_TAP_MS)
                    {
                        self.last_ctrlc = None;
                        return KeyAction::Exit;
                    }
                    self.last_ctrlc = Some(now);
                    return KeyAction::InterruptHint;
                }
                KeyCode::Char('j' | 'J') => {
                    if session.mode != EditorMode::Normal && session.mode != EditorMode::Visual {
                        session.insert_text("\n");
                    }
                    return KeyAction::Continue;
                }
                KeyCode::Char('d' | 'D') => {
                    if session.current_len() == 0 {
                        return KeyAction::Exit;
                    }
                    session.delete_char_under_cursor();
                    return KeyAction::Continue;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                if session.mode != EditorMode::Normal && session.mode != EditorMode::Visual {
                    session.insert_text("\n");
                }
                KeyAction::Continue
            }
            KeyCode::Enter => {
                if session.mode != EditorMode::Normal
                    && session.mode != EditorMode::Visual
                    && session.text.ends_with('\\')
                {
                    session.text.pop();
                    session.cursor = session.cursor.min(session.text.len());
                    session.insert_text("\n");
                    KeyAction::Continue
                } else {
                    session.submit_or_toggle()
                }
            }
            KeyCode::Esc => {
                if session.mode == EditorMode::Plain && session.has_input() {
                    let now = Instant::now();
                    if self
                        .last_esc
                        .is_some_and(|t| now.duration_since(t).as_millis() < DOUBLE_TAP_MS)
                    {
                        self.last_esc = None;
                        session.text.clear();
                        session.cursor = 0;
                        return KeyAction::Cancel;
                    }
                    self.last_esc = Some(now);
                    KeyAction::Continue
                } else {
                    self.last_esc = None;
                    session.handle_escape()
                }
            }
            KeyCode::Backspace => {
                session.handle_backspace();
                KeyAction::Continue
            }
            KeyCode::Delete => {
                session.delete_char_under_cursor();
                KeyAction::Continue
            }
            KeyCode::Left => {
                session.move_left();
                KeyAction::Continue
            }
            KeyCode::Right => {
                session.move_right();
                KeyAction::Continue
            }
            KeyCode::Up => {
                self.history_up(session);
                KeyAction::Continue
            }
            KeyCode::Down => {
                self.history_down(session);
                KeyAction::Continue
            }
            KeyCode::Home => {
                session.move_line_start();
                KeyAction::Continue
            }
            KeyCode::End => {
                session.move_line_end();
                KeyAction::Continue
            }
            KeyCode::Tab => KeyAction::Continue,
            KeyCode::Char(ch) => {
                self.handle_char(session, ch);
                KeyAction::Continue
            }
            _ => KeyAction::Continue,
        }
    }

    pub(super) fn handle_char(&mut self, session: &mut EditSession, ch: char) {
        match session.mode {
            EditorMode::Plain | EditorMode::Insert | EditorMode::Command => {
                session.insert_char(ch);
            }
            EditorMode::Normal => self.handle_normal_char(session, ch),
            EditorMode::Visual => Self::handle_visual_char(session, ch),
        }
    }

    fn handle_normal_char(&mut self, session: &mut EditSession, ch: char) {
        if let Some(operator) = session.pending_operator.take() {
            match (operator, ch) {
                ('d', 'd') => {
                    self.delete_current_line(session);
                    return;
                }
                ('y', 'y') => {
                    self.yank_current_line(session);
                    return;
                }
                _ => {}
            }
        }

        match ch {
            'h' => session.move_left(),
            'j' => session.move_down(),
            'k' => session.move_up(),
            'l' => session.move_right(),
            'd' | 'y' => session.pending_operator = Some(ch),
            'p' => self.paste_after(session),
            'i' => session.enter_insert_mode(),
            'v' => session.enter_visual_mode(),
            ':' => session.enter_command_mode(),
            _ => {}
        }
    }

    fn handle_visual_char(session: &mut EditSession, ch: char) {
        match ch {
            'h' => session.move_left(),
            'j' => session.move_down(),
            'k' => session.move_up(),
            'l' => session.move_right(),
            'v' => session.enter_normal_mode(),
            _ => {}
        }
    }

    fn delete_current_line(&mut self, session: &mut EditSession) {
        let (line_start_idx, line_end_idx, delete_start_idx) =
            current_line_delete_range(&session.text, session.cursor);
        self.yank_buffer.text = session.text[line_start_idx..line_end_idx].to_string();
        self.yank_buffer.shape = YankShape::Linewise;
        session.text.drain(delete_start_idx..line_end_idx);
        session.cursor = delete_start_idx.min(session.text.len());
    }

    fn yank_current_line(&mut self, session: &mut EditSession) {
        let (line_start_idx, line_end_idx, _) =
            current_line_delete_range(&session.text, session.cursor);
        self.yank_buffer.text = session.text[line_start_idx..line_end_idx].to_string();
        self.yank_buffer.shape = YankShape::Linewise;
    }

    fn paste_after(&mut self, session: &mut EditSession) {
        if self.yank_buffer.text.is_empty() {
            return;
        }

        if self.yank_buffer.shape == YankShape::Linewise {
            let line_end_idx = line_end(&session.text, session.cursor);
            let insert_at = if line_end_idx < session.text.len() {
                line_end_idx + 1
            } else {
                session.text.len()
            };
            let mut insertion = self.yank_buffer.text.clone();
            if insert_at == session.text.len()
                && !session.text.is_empty()
                && !session.text.ends_with('\n')
            {
                insertion.insert(0, '\n');
            }
            if insert_at < session.text.len() && !insertion.ends_with('\n') {
                insertion.push('\n');
            }
            session.text.insert_str(insert_at, &insertion);
            session.cursor = if insertion.starts_with('\n') {
                insert_at + 1
            } else {
                insert_at
            };
            return;
        }

        let insert_at = next_boundary(&session.text, session.cursor);
        session.text.insert_str(insert_at, &self.yank_buffer.text);
        session.cursor = insert_at + self.yank_buffer.text.len();
    }

    fn history_up(&self, session: &mut EditSession) {
        if session.mode == EditorMode::Command || self.history.is_empty() {
            return;
        }

        let next_index = if let Some(index) = session.history_index {
            index.saturating_sub(1)
        } else {
            session.history_backup = Some(session.text.clone());
            self.history.len() - 1
        };

        session.history_index = Some(next_index);
        session.set_text_from_history(self.history[next_index].clone());
    }

    fn history_down(&self, session: &mut EditSession) {
        if session.mode == EditorMode::Command {
            return;
        }

        let Some(index) = session.history_index else {
            return;
        };

        if index + 1 < self.history.len() {
            let next_index = index + 1;
            session.history_index = Some(next_index);
            session.set_text_from_history(self.history[next_index].clone());
            return;
        }

        session.history_index = None;
        let restored = session.history_backup.take().unwrap_or_default();
        session.set_text_from_history(restored);
        if self.vim_enabled {
            session.enter_insert_mode();
        } else {
            session.mode = EditorMode::Plain;
        }
    }
}

/// Return `s` with exactly one trailing space, adding one only if absent.
fn ensure_trailing_space(s: &str) -> String {
    if s.ends_with(' ') {
        s.to_string()
    } else {
        format!("{s} ")
    }
}

struct RawModeGuard;

impl RawModeGuard {
    fn new() -> io::Result<Self> {
        terminal::enable_raw_mode().map_err(io::Error::other)?;
        // Enable bracketed paste so that multi-line pastes arrive as a single
        // Event::Paste rather than individual character/Enter events, preventing
        // newlines in pasted text from accidentally submitting the prompt.
        let _ = execute!(io::stdout(), EnableBracketedPaste);
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste);
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests;
