use std::io;

pub(super) fn is_vim_toggle(line: &str) -> bool {
    line.trim() == "/vim"
}

pub(super) fn previous_boundary(text: &str, cursor: usize) -> usize {
    if cursor == 0 {
        return 0;
    }

    text[..cursor]
        .char_indices()
        .next_back()
        .map_or(0, |(index, _)| index)
}

pub(super) fn previous_command_boundary(text: &str, cursor: usize) -> usize {
    previous_boundary(text, cursor).max(1)
}

pub(super) fn next_boundary(text: &str, cursor: usize) -> usize {
    if cursor >= text.len() {
        return text.len();
    }

    text[cursor..]
        .chars()
        .next()
        .map_or(text.len(), |ch| cursor + ch.len_utf8())
}

pub(super) fn remove_previous_char(text: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }

    let start = previous_boundary(text, *cursor);
    text.drain(start..*cursor);
    *cursor = start;
}

pub(super) fn line_start(text: &str, cursor: usize) -> usize {
    text[..cursor].rfind('\n').map_or(0, |index| index + 1)
}

pub(super) fn line_end(text: &str, cursor: usize) -> usize {
    text[cursor..]
        .find('\n')
        .map_or(text.len(), |index| cursor + index)
}

pub(super) fn move_vertical(text: &str, cursor: usize, delta: isize) -> usize {
    let starts = line_starts(text);
    let current_row = text[..cursor].bytes().filter(|byte| *byte == b'\n').count();
    let current_start = starts[current_row];
    let current_col = text[current_start..cursor].chars().count();

    let max_row = isize::try_from(starts.len().saturating_sub(1)).unwrap_or(isize::MAX);
    let target_row =
        usize::try_from((isize::try_from(current_row).unwrap_or(0) + delta).clamp(0, max_row))
            .unwrap_or(0);
    if target_row == current_row {
        return cursor;
    }

    let target_start = starts[target_row];
    let target_end = if target_row + 1 < starts.len() {
        starts[target_row + 1] - 1
    } else {
        text.len()
    };
    byte_index_for_char_column(&text[target_start..target_end], current_col) + target_start
}

fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, ch) in text.char_indices() {
        if ch == '\n' {
            starts.push(index + 1);
        }
    }
    starts
}

fn byte_index_for_char_column(text: &str, column: usize) -> usize {
    text.char_indices()
        .nth(column)
        .map_or(text.len(), |(idx, _)| idx)
}

pub(super) fn current_line_delete_range(text: &str, cursor: usize) -> (usize, usize, usize) {
    let line_start_idx = line_start(text, cursor);
    let line_end_core = line_end(text, cursor);
    let line_end_idx = if line_end_core < text.len() {
        line_end_core + 1
    } else {
        line_end_core
    };
    let delete_start_idx = if line_end_idx == text.len() && line_start_idx > 0 {
        line_start_idx - 1
    } else {
        line_start_idx
    };
    (line_start_idx, line_end_idx, delete_start_idx)
}

pub(super) fn selection_bounds(text: &str, anchor: usize, cursor: usize) -> Option<(usize, usize)> {
    if text.is_empty() {
        return None;
    }

    if cursor >= anchor {
        let end = next_boundary(text, cursor);
        Some((anchor.min(text.len()), end.min(text.len())))
    } else {
        let end = next_boundary(text, anchor);
        Some((cursor.min(text.len()), end.min(text.len())))
    }
}

pub(super) fn render_selected_text(text: &str, start: usize, end: usize) -> String {
    let mut rendered = String::new();
    let mut in_selection = false;

    for (index, ch) in text.char_indices() {
        if !in_selection && index == start {
            rendered.push_str("\x1b[7m");
            in_selection = true;
        }
        if in_selection && index == end {
            rendered.push_str("\x1b[0m");
            in_selection = false;
        }
        rendered.push(ch);
    }

    if in_selection {
        rendered.push_str("\x1b[0m");
    }

    rendered
}

pub(super) fn slash_command_prefix(line: &str, pos: usize) -> Option<&str> {
    if pos != line.len() {
        return None;
    }

    let prefix = &line[..pos];
    if prefix.contains(char::is_whitespace) || !prefix.starts_with('/') {
        return None;
    }

    Some(prefix)
}

pub(super) fn to_u16(value: usize) -> io::Result<u16> {
    u16::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "terminal position overflowed u16",
        )
    })
}
