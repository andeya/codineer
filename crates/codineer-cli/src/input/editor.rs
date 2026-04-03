use std::io::{self, IsTerminal, Write};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal;

use super::session::{
    EditSession, EditorMode, KeyAction, ReadOutcome, YankBuffer,
};
use super::text::{
    current_line_delete_range, is_vim_toggle, line_end, next_boundary, slash_command_prefix,
};

pub struct LineEditor {
    prompt: String,
    completions: Vec<String>,
    pub(super) history: Vec<String>,
    yank_buffer: YankBuffer,
    pub(super) vim_enabled: bool,
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
        session.render(&mut stdout, &self.prompt, self.vim_enabled)?;

        loop {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                continue;
            }

            match self.handle_key_event(&mut session, key) {
                KeyAction::Continue => {
                    session.render(&mut stdout, &self.prompt, self.vim_enabled)?;
                }
                KeyAction::Submit(line) => {
                    session.finalize_render(&mut stdout, &self.prompt, self.vim_enabled)?;
                    return Ok(ReadOutcome::Submit(line));
                }
                KeyAction::Cancel => {
                    session.clear_render(&mut stdout)?;
                    writeln!(stdout)?;
                    return Ok(ReadOutcome::Cancel);
                }
                KeyAction::Exit => {
                    session.clear_render(&mut stdout)?;
                    writeln!(stdout)?;
                    return Ok(ReadOutcome::Exit);
                }
                KeyAction::ToggleVim => {
                    session.clear_render(&mut stdout)?;
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
                    session = EditSession::new(self.vim_enabled);
                    session.render(&mut stdout, &self.prompt, self.vim_enabled)?;
                }
            }
        }
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

    pub(super) fn handle_key_event(&mut self, session: &mut EditSession, key: KeyEvent) -> KeyAction {
        if key.code != KeyCode::Tab {
            self.completion_state = None;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c' | 'C') => {
                    return if session.has_input() {
                        KeyAction::Cancel
                    } else {
                        KeyAction::Exit
                    };
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
            KeyCode::Enter => session.submit_or_toggle(),
            KeyCode::Esc => session.handle_escape(),
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
            KeyCode::Tab => {
                self.complete_slash_command(session);
                KeyAction::Continue
            }
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
        self.yank_buffer.linewise = true;
        session.text.drain(delete_start_idx..line_end_idx);
        session.cursor = delete_start_idx.min(session.text.len());
    }

    fn yank_current_line(&mut self, session: &mut EditSession) {
        let (line_start_idx, line_end_idx, _) =
            current_line_delete_range(&session.text, session.cursor);
        self.yank_buffer.text = session.text[line_start_idx..line_end_idx].to_string();
        self.yank_buffer.linewise = true;
    }

    fn paste_after(&mut self, session: &mut EditSession) {
        if self.yank_buffer.text.is_empty() {
            return;
        }

        if self.yank_buffer.linewise {
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

    pub(super) fn complete_slash_command(&mut self, session: &mut EditSession) {
        if session.mode == EditorMode::Command {
            self.completion_state = None;
            return;
        }
        if let Some(state) = self
            .completion_state
            .as_mut()
            .filter(|_| session.cursor == session.text.len())
            .filter(|state| {
                state
                    .matches
                    .iter()
                    .any(|candidate| candidate == &session.text)
            })
        {
            let candidate = state.matches[state.next_index % state.matches.len()].clone();
            state.next_index += 1;
            session.text.replace_range(..session.cursor, &candidate);
            session.cursor = candidate.len();
            return;
        }
        let Some(prefix) = slash_command_prefix(&session.text, session.cursor) else {
            self.completion_state = None;
            return;
        };
        let matches = self
            .completions
            .iter()
            .filter(|candidate| candidate.starts_with(prefix) && candidate.as_str() != prefix)
            .cloned()
            .collect::<Vec<_>>();
        if matches.is_empty() {
            self.completion_state = None;
            return;
        }

        let candidate = if let Some(state) = self
            .completion_state
            .as_mut()
            .filter(|state| state.prefix == prefix && state.matches == matches)
        {
            let index = state.next_index % state.matches.len();
            state.next_index += 1;
            state.matches[index].clone()
        } else {
            let candidate = matches[0].clone();
            self.completion_state = Some(CompletionState {
                prefix: prefix.to_string(),
                matches,
                next_index: 1,
            });
            candidate
        };

        session.text.replace_range(..session.cursor, &candidate);
        session.cursor = candidate.len();
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

struct RawModeGuard;

impl RawModeGuard {
    fn new() -> io::Result<Self> {
        terminal::enable_raw_mode().map_err(io::Error::other)?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests;
