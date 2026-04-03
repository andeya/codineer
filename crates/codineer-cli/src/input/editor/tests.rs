use super::super::session::{EditSession, EditorMode, KeyAction};
use super::super::text::{is_vim_toggle, selection_bounds, slash_command_prefix};
use super::LineEditor;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn extracts_only_terminal_slash_command_prefixes() {
    // given
    let complete_prefix = slash_command_prefix("/he", 3);
    let whitespace_prefix = slash_command_prefix("/help me", 5);
    let plain_text_prefix = slash_command_prefix("hello", 5);
    let mid_buffer_prefix = slash_command_prefix("/help", 2);

    // when
    let result = (
        complete_prefix,
        whitespace_prefix,
        plain_text_prefix,
        mid_buffer_prefix,
    );

    // then
    assert_eq!(result, (Some("/he"), None, None, None));
}

#[test]
fn toggle_submission_detects_vim_command() {
    assert!(is_vim_toggle("/vim"));
    assert!(is_vim_toggle("  /vim  "));
    assert!(!is_vim_toggle("/help"));
    assert!(!is_vim_toggle("hello"));
}

#[test]
fn normal_mode_supports_motion_and_insert_transition() {
    // given
    let mut editor = LineEditor::new("> ", vec![]);
    editor.vim_enabled = true;
    let mut session = EditSession::new(true);
    session.text = "hello".to_string();
    session.cursor = session.text.len();
    let _ = session.handle_escape();

    // when
    editor.handle_char(&mut session, 'h');
    editor.handle_char(&mut session, 'i');
    editor.handle_char(&mut session, '!');

    // then
    assert_eq!(session.mode, EditorMode::Insert);
    assert_eq!(session.text, "hel!lo");
}

#[test]
fn yy_and_p_paste_yanked_line_after_current_line() {
    // given
    let mut editor = LineEditor::new("> ", vec![]);
    editor.vim_enabled = true;
    let mut session = EditSession::new(true);
    session.text = "alpha\nbeta\ngamma".to_string();
    session.cursor = 0;
    let _ = session.handle_escape();

    // when
    editor.handle_char(&mut session, 'y');
    editor.handle_char(&mut session, 'y');
    editor.handle_char(&mut session, 'p');

    // then
    assert_eq!(session.text, "alpha\nalpha\nbeta\ngamma");
}

#[test]
fn dd_and_p_paste_deleted_line_after_current_line() {
    // given
    let mut editor = LineEditor::new("> ", vec![]);
    editor.vim_enabled = true;
    let mut session = EditSession::new(true);
    session.text = "alpha\nbeta\ngamma".to_string();
    session.cursor = 0;
    let _ = session.handle_escape();

    // when
    editor.handle_char(&mut session, 'j');
    editor.handle_char(&mut session, 'd');
    editor.handle_char(&mut session, 'd');
    editor.handle_char(&mut session, 'p');

    // then
    assert_eq!(session.text, "alpha\ngamma\nbeta\n");
}

#[test]
fn visual_mode_tracks_selection_with_motions() {
    // given
    let mut editor = LineEditor::new("> ", vec![]);
    editor.vim_enabled = true;
    let mut session = EditSession::new(true);
    session.text = "alpha\nbeta".to_string();
    session.cursor = 0;
    let _ = session.handle_escape();

    // when
    editor.handle_char(&mut session, 'v');
    editor.handle_char(&mut session, 'j');
    editor.handle_char(&mut session, 'l');

    // then
    assert_eq!(session.mode, EditorMode::Visual);
    assert_eq!(
        selection_bounds(
            &session.text,
            session.visual_anchor.unwrap_or(0),
            session.cursor
        ),
        Some((0, 8))
    );
}

#[test]
fn command_mode_submits_colon_prefixed_input() {
    // given
    let mut editor = LineEditor::new("> ", vec![]);
    editor.vim_enabled = true;
    let mut session = EditSession::new(true);
    session.text = "draft".to_string();
    session.cursor = session.text.len();
    let _ = session.handle_escape();

    // when
    editor.handle_char(&mut session, ':');
    editor.handle_char(&mut session, 'q');
    editor.handle_char(&mut session, '!');
    let action = session.submit_or_toggle();

    // then
    assert_eq!(session.mode, EditorMode::Command);
    assert_eq!(session.command_buffer, ":q!");
    assert!(matches!(action, KeyAction::Submit(line) if line == ":q!"));
}

#[test]
fn push_history_ignores_blank_entries() {
    // given
    let mut editor = LineEditor::new("> ", vec!["/help".to_string()]);

    // when
    editor.push_history("   ");
    editor.push_history("/help");

    // then
    assert_eq!(editor.history, vec!["/help".to_string()]);
}

#[test]
fn tab_completes_matching_slash_commands() {
    // given
    let mut editor = LineEditor::new("> ", vec!["/help".to_string(), "/hello".to_string()]);
    let mut session = EditSession::new(false);
    session.text = "/he".to_string();
    session.cursor = session.text.len();

    // when
    editor.complete_slash_command(&mut session);

    // then
    assert_eq!(session.text, "/help");
    assert_eq!(session.cursor, 5);
}

#[test]
fn tab_cycles_between_matching_slash_commands() {
    // given
    let mut editor = LineEditor::new(
        "> ",
        vec!["/permissions".to_string(), "/plugin".to_string()],
    );
    let mut session = EditSession::new(false);
    session.text = "/p".to_string();
    session.cursor = session.text.len();

    // when
    editor.complete_slash_command(&mut session);
    let first = session.text.clone();
    session.cursor = session.text.len();
    editor.complete_slash_command(&mut session);
    let second = session.text.clone();

    // then
    assert_eq!(first, "/permissions");
    assert_eq!(second, "/plugin");
}

#[test]
fn ctrl_c_cancels_when_input_exists() {
    // given
    let mut editor = LineEditor::new("> ", vec![]);
    let mut session = EditSession::new(false);
    session.text = "draft".to_string();
    session.cursor = session.text.len();

    // when
    let action = editor.handle_key_event(
        &mut session,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    );

    // then
    assert!(matches!(action, KeyAction::Cancel));
}
