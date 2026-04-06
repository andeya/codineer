use std::collections::HashMap;
use std::io::{self, IsTerminal, Write};
use std::time::Instant;

use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::{execute, terminal};

use super::session::{
    EditSession, EditorMode, ImageData, KeyAction, ReadOutcome, SubmitPayload, YankBuffer,
    YankShape,
};
use super::suggestions::{self, CommandEntry, SuggestionState, SuggestionTrigger};
use super::text::{current_line_delete_range, is_vim_toggle, line_end, next_boundary};

/// Maximum interval (ms) between two Ctrl+C or Esc presses to be considered a
/// "double-tap".
const DOUBLE_TAP_MS: u128 = 1500;

/// Pasted text longer than this many characters is stored as a reference and
/// shown as `[Pasted text #N +M lines]` in the input area (Claude Code style).
const PASTE_CHAR_THRESHOLD: usize = 800;
/// Pasted text with more than this many newlines is also stored as a reference.
const PASTE_LINE_THRESHOLD: usize = 2;

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
    /// Optional one-line hint rendered in the info area (below the bottom
    /// separator) whenever no other panel is active.
    hint_line: Option<String>,
    /// Storage for pasted text references: maps numeric ID → original content.
    /// References are shown as `[Pasted text #N +M lines]` in the input and
    /// expanded back to their full content when the message is submitted.
    paste_store: HashMap<usize, String>,
    /// Auto-incrementing counter for paste reference IDs.
    next_paste_id: usize,
    /// Storage for clipboard-pasted images: keyed by monotonic ID so
    /// `drain_image_store` can return them in insertion order cheaply.
    image_store: std::collections::BTreeMap<usize, ImageData>,
    next_image_id: usize,
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
            hint_line: None,
            paste_store: HashMap::new(),
            next_paste_id: 1,
            image_store: std::collections::BTreeMap::new(),
            next_image_id: 1,
        }
    }

    /// Enable a full-width separator line above the prompt that
    /// auto-adjusts on terminal resize.
    #[must_use]
    pub fn with_separator(mut self) -> Self {
        self.show_separator = true;
        self
    }

    /// Set a prefix renderer (e.g. banner) shown above the top separator on
    /// the first prompt. Re-generated on resize so the layout adapts to the
    /// current terminal width.
    #[must_use]
    pub fn with_prefix(mut self, f: impl Fn() -> String + 'static) -> Self {
        self.prefix_fn = Some(Box::new(f));
        self
    }

    /// Set a persistent one-line hint shown in the info area below the bottom
    /// separator whenever no other panel (shortcuts, interrupt, suggestions) is
    /// active.  Accepts ANSI-coloured strings.
    #[must_use]
    pub fn with_hint_line(mut self, hint: impl Into<String>) -> Self {
        self.hint_line = Some(hint.into());
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
        session.static_hint = self.hint_line.clone();
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
                let text = text.replace('\r', "\n");
                let trimmed = text.trim();

                // Drag-and-drop: if paste is a single-line image path, attach it.
                if !trimmed.is_empty()
                    && !trimmed.contains('\n')
                    && crate::image_util::is_image_path(std::path::Path::new(trimmed))
                    && std::path::Path::new(trimmed).exists()
                {
                    if let Ok(bytes) = std::fs::read(trimmed) {
                        let media_type = crate::image_util::detect_media_type(&bytes)
                            .or_else(|| {
                                crate::image_util::media_type_from_extension(std::path::Path::new(
                                    trimmed,
                                ))
                            })
                            .unwrap_or("image/png");
                        let id = self.next_image_id;
                        self.next_image_id += 1;
                        self.image_store.insert(
                            id,
                            ImageData {
                                bytes,
                                media_type: media_type.to_string(),
                                source_label: trimmed.to_string(),
                            },
                        );
                        session.insert_text(&format!("[Image #{id}]"));
                        self.update_suggestions(&session);
                        session.render_with_suggestions(
                            &mut stdout,
                            &self.prompt,
                            self.vim_enabled,
                            self.suggestion_state.as_ref(),
                        )?;
                        continue;
                    }
                }

                // Cmd+V (macOS) / Ctrl+Shift+V (Linux) with image clipboard: the
                // terminal sends an empty bracketed-paste event when the clipboard
                // holds binary image data (no text representation).  Try arboard.
                if trimmed.is_empty() {
                    match super::clipboard::read_clipboard_image() {
                        Ok((bytes, media_type)) => {
                            let id = self.next_image_id;
                            self.next_image_id += 1;
                            self.image_store.insert(
                                id,
                                ImageData {
                                    bytes,
                                    media_type: media_type.to_string(),
                                    source_label: "clipboard".to_string(),
                                },
                            );
                            session.insert_text(&format!("[Image #{id}]"));
                            self.update_suggestions(&session);
                        }
                        Err(_) => {
                            // Empty paste with no image — just ignore silently.
                        }
                    }
                    session.render_with_suggestions(
                        &mut stdout,
                        &self.prompt,
                        self.vim_enabled,
                        self.suggestion_state.as_ref(),
                    )?;
                    continue;
                }

                let num_newlines = text.matches('\n').count();
                let insert =
                    if text.len() > PASTE_CHAR_THRESHOLD || num_newlines > PASTE_LINE_THRESHOLD {
                        let id = self.next_paste_id;
                        self.next_paste_id += 1;
                        self.paste_store.insert(id, text);
                        if num_newlines == 0 {
                            format!("[Pasted text #{id}]")
                        } else {
                            format!("[Pasted text #{id} +{num_newlines} lines]")
                        }
                    } else {
                        text
                    };
                session.insert_text(&insert);
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

            // Ctrl+V: attempt clipboard image paste before normal key handling.
            if key.code == KeyCode::Char('v') && key.modifiers == KeyModifiers::CONTROL {
                match super::clipboard::read_clipboard_image() {
                    Ok((bytes, media_type)) => {
                        let id = self.next_image_id;
                        self.next_image_id += 1;
                        self.image_store.insert(
                            id,
                            ImageData {
                                bytes,
                                media_type: media_type.to_string(),
                                source_label: "clipboard".to_string(),
                            },
                        );
                        session.insert_text(&format!("[Image #{id}]"));
                        self.update_suggestions(&session);
                    }
                    Err(reason) => {
                        session.transient_status = Some(reason);
                    }
                }
                session.render_with_suggestions(
                    &mut stdout,
                    &self.prompt,
                    self.vim_enabled,
                    self.suggestion_state.as_ref(),
                )?;
                // Consume Ctrl+V here so the fallthrough path never inserts 'v'.
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
                    let line = self.expand_paste_refs(line);
                    let images = self.drain_image_store();
                    session.finalize_render(&mut stdout, &self.prompt, self.vim_enabled)?;
                    self.prefix_fn = None;
                    return Ok(ReadOutcome::Submit(SubmitPayload { text: line, images }));
                }
                KeyAction::Cancel => {
                    // Stay on the same line — clear input and re-render in
                    // place so no blank lines are created.
                    session.text.clear();
                    session.cursor = 0;
                    session.history_index = None;
                    session.history_backup = None;
                    self.suggestion_state = None;
                    session.render_with_suggestions(
                        &mut stdout,
                        &self.prompt,
                        self.vim_enabled,
                        None,
                    )?;
                }
                KeyAction::Exit => {
                    session.clear_render(&mut stdout, 0)?;
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
                    session.static_hint = self.hint_line.clone();
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
            // Blank line after the banner to separate it from the input area,
            // matching Claude Code's marginTop={1} between banner and prompt.
            write!(out, "\r\n")?;
            session.prefix_line_widths.push(0);
        }

        if self.show_separator {
            super::session::write_dim_separator(out, new_cols)?;
            write!(out, "\r\n")?;
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

            return Ok(ReadOutcome::Submit(SubmitPayload {
                text: buffer,
                images: Vec::new(),
            }));
        }
    }

    fn drain_image_store(&mut self) -> Vec<ImageData> {
        std::mem::take(&mut self.image_store)
            .into_values()
            .collect()
    }

    // --- Paste reference expansion ---

    /// Replace every `[Pasted text #N …]` token in `text` with the original
    /// stored content so the AI receives the full pasted value.
    fn expand_paste_refs(&self, text: String) -> String {
        if self.paste_store.is_empty() {
            return text;
        }
        let mut result = text;
        for (id, content) in &self.paste_store {
            let prefix = format!("[Pasted text #{id}");
            let mut search_from = 0;
            while let Some(rel) = result[search_from..].find(&prefix) {
                let abs = search_from + rel;
                if let Some(close) = result[abs..].find(']') {
                    let end = abs + close + 1;
                    result.replace_range(abs..end, content);
                    search_from = abs + content.len();
                } else {
                    break;
                }
            }
        }
        result
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
        // Clear one-shot hints on every keystroke.
        session.show_interrupt_hint = false;
        session.transient_status = None;

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
        let mut out = io::stdout();
        let _ = execute!(out, EnableBracketedPaste);
        // Kitty keyboard protocol: lets the terminal report modifier keys on
        // Enter (Shift+Enter → KeyModifiers::SHIFT) so we can distinguish
        // "new line" from "submit".  Silently ignored by terminals that don't
        // support the protocol (e.g. Terminal.app).
        let _ = execute!(
            out,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        );
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = execute!(out, PopKeyboardEnhancementFlags);
        let _ = execute!(out, DisableBracketedPaste);
        let _ = terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests;
