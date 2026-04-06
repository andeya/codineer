use runtime::ContentBlock;
use serde_json::json;

/// Structured output from `@`-mention processing, carrying both text and image
/// content blocks so the caller can build a multimodal user message.
pub(crate) struct EnrichedInput {
    pub blocks: Vec<ContentBlock>,
}

pub(crate) fn final_assistant_text(summary: &runtime::TurnSummary) -> String {
    summary
        .assistant_messages
        .last()
        .map(|message| {
            message
                .blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default()
}

pub(crate) fn collect_tool_uses(summary: &runtime::TurnSummary) -> Vec<serde_json::Value> {
    summary
        .assistant_messages
        .iter()
        .flat_map(|message| message.blocks.iter())
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => Some(json!({
                "id": id,
                "name": name,
                "input": input,
            })),
            _ => None,
        })
        .collect()
}

pub(crate) fn collect_tool_results(summary: &runtime::TurnSummary) -> Vec<serde_json::Value> {
    summary
        .tool_results
        .iter()
        .flat_map(|message| message.blocks.iter())
        .filter_map(|block| match block {
            ContentBlock::ToolResult {
                tool_use_id,
                tool_name,
                output,
                is_error,
            } => Some(json!({
                "tool_use_id": tool_use_id,
                "tool_name": tool_name,
                "output": output,
                "is_error": is_error,
            })),
            _ => None,
        })
        .collect()
}

/// Build an `EnrichedInput` from user text, expanding `@` mentions into text
/// XML blocks (for code/directories) or image content blocks.  Caller-supplied
/// `extra_images` (e.g. from clipboard paste) are prepended.
pub(crate) fn process_at_mentioned_files(
    input: &str,
    extra_images: Vec<ContentBlock>,
) -> EnrichedInput {
    // Strip [Image #N] placeholder tokens when there are extra images: the
    // actual image data is transmitted as content blocks so the placeholder
    // text is redundant and confuses models that only see the text side.
    let input_stripped;
    let input = if !extra_images.is_empty() && input.contains("[Image #") {
        input_stripped = strip_image_placeholders(input);
        input_stripped.as_str()
    } else {
        input
    };

    let paths = crate::input::suggestions::extract_at_mentioned_files(input);

    let mut image_blocks: Vec<ContentBlock> = extra_images;
    let mut text_xml_parts: Vec<String> = Vec::new();

    if !paths.is_empty() {
        const MAX_LINES: usize = 2000;
        const LINE_CONTEXT: usize = 50;

        for path in &paths {
            let (file_path, line_ref) = parse_line_ref(path);
            let p = std::path::Path::new(file_path);

            if p.is_dir() {
                if let Ok(entries) = std::fs::read_dir(p) {
                    let listing: Vec<String> = entries
                        .flatten()
                        .take(100)
                        .map(|e| {
                            let name = e.file_name().to_string_lossy().to_string();
                            if e.file_type().is_ok_and(|ft| ft.is_dir()) {
                                format!("{name}/")
                            } else {
                                name
                            }
                        })
                        .collect();
                    text_xml_parts.push(format!(
                        "<attached_directory path=\"{file_path}\">\n{}\n</attached_directory>",
                        listing.join("\n")
                    ));
                } else {
                    eprintln!("[warn] @{path}: directory not readable, skipping");
                    text_xml_parts.push(format!("<not_found path=\"{file_path}\"/>"));
                }
            } else if crate::image_util::is_image_path(p) {
                match crate::image_util::read_image_as_block(p) {
                    Ok(block) => image_blocks.push(block),
                    Err(e) => {
                        eprintln!("[warn] @{path}: {e}");
                        text_xml_parts.push(format!("<not_found path=\"{file_path}\"/>"));
                    }
                }
            } else if let Ok(content) = std::fs::read_to_string(p) {
                let all_lines: Vec<&str> = content.lines().collect();
                let total = all_lines.len();

                let (selected, start_line, end_line) = match line_ref {
                    Some((start, Some(end))) => {
                        let s = start.saturating_sub(1).min(total);
                        let e = end.min(total);
                        (all_lines[s..e].to_vec(), s + 1, e)
                    }
                    Some((line, None)) => {
                        let s = line
                            .saturating_sub(1)
                            .saturating_sub(LINE_CONTEXT)
                            .min(total);
                        let e = (line + LINE_CONTEXT).min(total);
                        (all_lines[s..e].to_vec(), s + 1, e)
                    }
                    None => {
                        let e = total.min(MAX_LINES);
                        (all_lines[..e].to_vec(), 1, e)
                    }
                };

                let lines_attr = if start_line != 1 || end_line != total {
                    format!(" lines=\"{start_line}-{end_line}\"")
                } else {
                    String::new()
                };
                text_xml_parts.push(format!(
                    "<attached_file path=\"{file_path}\"{lines_attr}>\n{}\n</attached_file>",
                    selected.join("\n")
                ));
            } else if p.exists() {
                // Binary file: can't decode as text, but report metadata so the
                // model knows the file exists and how large it is.
                let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                let size_str = human_bytes(size);
                eprintln!("[warn] @{path}: binary file ({size_str}), injecting metadata only");
                text_xml_parts.push(format!(
                    "<attached_file path=\"{file_path}\" type=\"binary\" size=\"{size_str}\">\
                     [binary content — cannot be read as text]\
                     </attached_file>"
                ));
            } else {
                eprintln!("[warn] @{path}: not found, skipping");
                text_xml_parts.push(format!("<not_found path=\"{file_path}\"/>"));
            }
        }
    }

    let clean_input = if paths.is_empty() {
        input.to_string()
    } else {
        strip_at_mentions(input, &paths)
    };

    let text = if text_xml_parts.is_empty() {
        clean_input
    } else if clean_input.is_empty() {
        text_xml_parts.join("\n\n")
    } else {
        format!("{}\n\n{clean_input}", text_xml_parts.join("\n\n"))
    };

    let mut blocks = image_blocks;
    if !text.is_empty() {
        blocks.push(ContentBlock::Text { text });
    }

    EnrichedInput { blocks }
}

#[cfg(test)]
pub(crate) fn process_at_mentioned_files_text(input: &str) -> String {
    let enriched = process_at_mentioned_files(input, Vec::new());
    enriched
        .blocks
        .into_iter()
        .filter_map(|b| match b {
            ContentBlock::Text { text } => Some(text),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Parse an optional `:LINE` or `:START-END` suffix from a path reference.
/// Returns `(actual_path, Some((start, optional_end)))` or `(path, None)`.
fn parse_line_ref(path: &str) -> (&str, Option<(usize, Option<usize>)>) {
    if let Some(colon_pos) = path.rfind(':') {
        let suffix = &path[colon_pos + 1..];
        if let Some(dash) = suffix.find('-') {
            if let (Ok(start), Ok(end)) = (
                suffix[..dash].parse::<usize>(),
                suffix[dash + 1..].parse::<usize>(),
            ) {
                if start > 0 {
                    return (&path[..colon_pos], Some((start, Some(end))));
                }
            }
        } else if let Ok(line) = suffix.parse::<usize>() {
            if line > 0 {
                return (&path[..colon_pos], Some((line, None)));
            }
        }
    }
    (path, None)
}

fn human_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

/// Remove `[Image #N]` placeholder tokens from `input`.  These are inserted
/// by the editor when images are attached via clipboard or drag-and-drop;
/// the actual image data is sent as content blocks so the placeholder text
/// is redundant.
fn strip_image_placeholders(input: &str) -> String {
    let mut result = input.to_string();
    while let Some(start) = result.find("[Image #") {
        let after_prefix = &result[start + 8..]; // len("[Image #") == 8
        let digit_end = after_prefix
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after_prefix.len());
        if digit_end > 0 && after_prefix[digit_end..].starts_with(']') {
            let end = start + 8 + digit_end + 1; // +1 for ']'
            result.drain(start..end);
        } else {
            break;
        }
    }
    // Collapse horizontal whitespace runs per line (preserving newlines).
    result
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Remove every `@path` (and `@"path"`) token from `input` and normalise the
/// resulting whitespace so the model receives clean prose.
fn strip_at_mentions(input: &str, paths: &[String]) -> String {
    let mut result = input.to_string();
    for path in paths {
        // Quoted form first (more specific) then plain form
        result = result.replace(&format!("@\"{path}\""), "");
        result = result.replace(&format!("@{path}"), "");
    }
    // Collapse runs of spaces/tabs within each line; preserve newlines.
    let mut cleaned = String::with_capacity(result.len());
    let mut prev_space = false;
    for ch in result.chars() {
        if ch == ' ' || ch == '\t' {
            if !prev_space {
                cleaned.push(' ');
            }
            prev_space = true;
        } else {
            prev_space = false;
            cleaned.push(ch);
        }
    }
    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_image_placeholders_removes_single() {
        assert_eq!(
            strip_image_placeholders("[Image #1]你收到几张图片"),
            "你收到几张图片"
        );
    }

    #[test]
    fn strip_image_placeholders_removes_multiple() {
        assert_eq!(
            strip_image_placeholders("[Image #1] and [Image #2] describe these"),
            "and describe these"
        );
    }

    #[test]
    fn strip_image_placeholders_no_op_without_images() {
        let s = "hello world";
        assert_eq!(strip_image_placeholders(s), s);
    }

    #[test]
    fn strip_image_placeholders_does_not_remove_non_numeric() {
        let s = "[Image #abc]text";
        assert_eq!(strip_image_placeholders(s), s);
    }

    #[test]
    fn strip_image_placeholders_preserves_newlines() {
        let input = "[Image #1]\nFirst paragraph.\n\nSecond paragraph.";
        assert_eq!(
            strip_image_placeholders(input),
            "First paragraph.\n\nSecond paragraph."
        );
    }
}
