use std::cmp::Reverse;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

use std::fmt;

use glob::Pattern;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::{Searcher, SearcherBuilder, Sink, SinkContext, SinkMatch};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrepOutputMode {
    #[default]
    FilesWithMatches,
    Content,
    Count,
}

impl fmt::Display for GrepOutputMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FilesWithMatches => f.write_str("files_with_matches"),
            Self::Content => f.write_str("content"),
            Self::Count => f.write_str("count"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextFilePayload {
    #[serde(rename = "filePath")]
    pub file_path: String,
    pub content: String,
    #[serde(rename = "numLines")]
    pub num_lines: usize,
    #[serde(rename = "startLine")]
    pub start_line: usize,
    #[serde(rename = "totalLines")]
    pub total_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadFileOutput {
    /// `"text"` for plain text, `"image"` for binary images, `"pdf"` for PDFs.
    #[serde(rename = "type")]
    pub kind: String,
    pub file: TextFilePayload,
    /// Nanoseconds since UNIX epoch at the time the file was read.
    /// Pass this value as `last_modified_at` to `edit_file` to enable
    /// write-conflict detection.
    #[serde(rename = "lastModifiedAt", skip_serializing_if = "Option::is_none")]
    pub last_modified_at: Option<u64>,
    /// MIME type for image files (e.g. `"image/png"`).
    #[serde(rename = "mediaType", skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StructuredPatchHunk {
    #[serde(rename = "oldStart")]
    pub old_start: usize,
    #[serde(rename = "oldLines")]
    pub old_lines: usize,
    #[serde(rename = "newStart")]
    pub new_start: usize,
    #[serde(rename = "newLines")]
    pub new_lines: usize,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WriteFileOutput {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "filePath")]
    pub file_path: String,
    pub content: String,
    #[serde(rename = "structuredPatch")]
    pub structured_patch: Vec<StructuredPatchHunk>,
    #[serde(rename = "originalFile")]
    pub original_file: Option<String>,
    #[serde(rename = "gitDiff")]
    pub git_diff: Option<serde_json::Value>,
    /// Nanoseconds since UNIX epoch at the time of this write.
    #[serde(rename = "lastModifiedAt", skip_serializing_if = "Option::is_none")]
    pub last_modified_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditFileOutput {
    #[serde(rename = "filePath")]
    pub file_path: String,
    #[serde(rename = "oldString")]
    pub old_string: String,
    #[serde(rename = "newString")]
    pub new_string: String,
    #[serde(rename = "originalFile")]
    pub original_file: String,
    #[serde(rename = "structuredPatch")]
    pub structured_patch: Vec<StructuredPatchHunk>,
    #[serde(rename = "userModified")]
    pub user_modified: bool,
    #[serde(rename = "replaceAll")]
    pub replace_all: bool,
    #[serde(rename = "gitDiff")]
    pub git_diff: Option<serde_json::Value>,
    /// Nanoseconds since UNIX epoch after this edit was written.
    #[serde(rename = "lastModifiedAt", skip_serializing_if = "Option::is_none")]
    pub last_modified_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlobSearchOutput {
    #[serde(rename = "durationMs")]
    pub duration_ms: u128,
    #[serde(rename = "numFiles")]
    pub num_files: usize,
    pub filenames: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GrepSearchInput {
    pub pattern: String,
    pub path: Option<String>,
    pub glob: Option<String>,
    #[serde(rename = "output_mode")]
    pub output_mode: Option<GrepOutputMode>,
    #[serde(rename = "-B")]
    pub before: Option<usize>,
    #[serde(rename = "-A")]
    pub after: Option<usize>,
    #[serde(rename = "-C")]
    pub context_short: Option<usize>,
    pub context: Option<usize>,
    #[serde(rename = "-n")]
    pub line_numbers: Option<bool>,
    #[serde(rename = "-i")]
    pub case_insensitive: Option<bool>,
    #[serde(rename = "type")]
    pub file_type: Option<String>,
    pub head_limit: Option<usize>,
    pub offset: Option<usize>,
    pub multiline: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GrepSearchOutput {
    pub mode: Option<GrepOutputMode>,
    #[serde(rename = "numFiles")]
    pub num_files: usize,
    pub filenames: Vec<String>,
    pub content: Option<String>,
    #[serde(rename = "numLines")]
    pub num_lines: Option<usize>,
    #[serde(rename = "numMatches")]
    pub num_matches: Option<usize>,
    #[serde(rename = "appliedLimit")]
    pub applied_limit: Option<usize>,
    #[serde(rename = "appliedOffset")]
    pub applied_offset: Option<usize>,
}

/// Resolve and validate `path` against the workspace root.
///
/// Returns the canonical absolute path, or `PermissionDenied` if the resolved
/// path escapes the workspace, or `NotFound` if the path does not exist.
/// Use this for any tool that reads or writes files at caller-supplied paths.
pub fn workspace_safe_path(path: &str) -> io::Result<PathBuf> {
    normalize_path(path)
}

/// Maximum file size accepted by `read_file`, `write_file`, and `edit_file`.
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MiB

pub fn read_file(
    path: &str,
    offset: Option<usize>,
    limit: Option<usize>,
) -> io::Result<ReadFileOutput> {
    let absolute_path = normalize_path(path)?;
    let metadata = fs::metadata(&absolute_path)?;
    let last_modified_at = mtime_nanos(&metadata).ok();
    let file_path_str = absolute_path.to_string_lossy().into_owned();

    let ext = absolute_path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);

    // ── Image files ──────────────────────────────────────────────────────────
    if let Some(media_type) = ext.as_deref().and_then(image_media_type) {
        let bytes = fs::read(&absolute_path)?;
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
        return Ok(ReadFileOutput {
            kind: String::from("image"),
            file: TextFilePayload {
                file_path: file_path_str,
                content: b64,
                num_lines: 0,
                start_line: 1,
                total_lines: 0,
            },
            last_modified_at,
            media_type: Some(media_type.to_owned()),
        });
    }

    // ── PDF files ─────────────────────────────────────────────────────────────
    if ext.as_deref() == Some("pdf") {
        let text = extract_pdf_text(&absolute_path)?;
        let lines: Vec<&str> = text.lines().collect();
        let start_index = offset.unwrap_or(0).min(lines.len());
        let end_index = limit.map_or(lines.len(), |limit| {
            start_index.saturating_add(limit).min(lines.len())
        });
        let selected = lines[start_index..end_index].join("\n");
        return Ok(ReadFileOutput {
            kind: String::from("pdf"),
            file: TextFilePayload {
                file_path: file_path_str,
                content: selected,
                num_lines: end_index.saturating_sub(start_index),
                start_line: start_index.saturating_add(1),
                total_lines: lines.len(),
            },
            last_modified_at,
            media_type: None,
        });
    }

    // ── Text files (default) ──────────────────────────────────────────────────
    let content = fs::read_to_string(&absolute_path)?;
    let lines: Vec<&str> = content.lines().collect();
    let start_index = offset.unwrap_or(0).min(lines.len());
    let end_index = limit.map_or(lines.len(), |limit| {
        start_index.saturating_add(limit).min(lines.len())
    });
    let selected = lines[start_index..end_index].join("\n");

    Ok(ReadFileOutput {
        kind: String::from("text"),
        file: TextFilePayload {
            file_path: file_path_str,
            content: selected,
            num_lines: end_index.saturating_sub(start_index),
            start_line: start_index.saturating_add(1),
            total_lines: lines.len(),
        },
        last_modified_at,
        media_type: None,
    })
}

/// Returns the MIME type string for recognized image extensions, or `None`.
fn image_media_type(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

/// Extract plain text from a PDF file using lopdf.
fn extract_pdf_text(path: &Path) -> io::Result<String> {
    use lopdf::Document;

    let doc = Document::load(path).map_err(|e| io::Error::other(format!("PDF load error: {e}")))?;

    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    let mut out = String::new();
    for page_num in pages {
        match doc.extract_text(&[page_num]) {
            Ok(text) => {
                out.push_str(&text);
                out.push('\n');
            }
            Err(e) => {
                out.push_str(&format!("[page {page_num} extraction failed: {e}]\n"));
            }
        }
    }
    Ok(out)
}

pub fn write_file(path: &str, content: &str) -> io::Result<WriteFileOutput> {
    if content.len() as u64 > MAX_FILE_SIZE {
        return Err(io::Error::other(format!(
            "content is too large to write ({} bytes exceeds the {} MiB limit)",
            content.len(),
            MAX_FILE_SIZE / (1024 * 1024)
        )));
    }

    let absolute_path = normalize_path_allow_missing(path)?;
    let original_file = fs::read_to_string(&absolute_path).ok();
    if let Some(parent) = absolute_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Atomic write: write to sibling temp file then rename.
    atomic_write(&absolute_path, content.as_bytes())?;
    let last_modified_at = fs::metadata(&absolute_path)
        .ok()
        .and_then(|m| mtime_nanos(&m).ok());

    Ok(WriteFileOutput {
        kind: if original_file.is_some() {
            String::from("update")
        } else {
            String::from("create")
        },
        file_path: absolute_path.to_string_lossy().into_owned(),
        content: content.to_owned(),
        structured_patch: make_patch(original_file.as_deref().unwrap_or(""), content),
        original_file,
        git_diff: None,
        last_modified_at,
    })
}

/// Edit `old_string` → `new_string` inside the file at `path`.
///
/// `expected_mtime_nanos` — when supplied, the function checks that the file's
/// current mtime matches this value (nanoseconds since UNIX epoch).  A mismatch
/// means the file was changed by another process since it was last read, and the
/// call returns `Err` with `ErrorKind::Other` so the caller can re-read and retry.
pub fn edit_file(
    path: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
    expected_mtime_nanos: Option<u64>,
) -> io::Result<EditFileOutput> {
    let absolute_path = normalize_path(path)?;

    let metadata = fs::metadata(&absolute_path)?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(io::Error::other(format!(
            "file is too large to edit ({} bytes exceeds the {} MiB limit)",
            metadata.len(),
            MAX_FILE_SIZE / (1024 * 1024)
        )));
    }

    // Write-conflict detection: if the caller holds a stale mtime, abort.
    if let Some(expected) = expected_mtime_nanos {
        let current = mtime_nanos(&metadata)?;
        if current != expected {
            return Err(io::Error::other(
                "file was modified by another process since it was last read; \
                 re-read the file and retry the edit",
            ));
        }
    }

    let original_file = fs::read_to_string(&absolute_path)?;

    if old_string == new_string {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "old_string and new_string must differ",
        ));
    }

    // Work with LF-normalised content so edits succeed regardless of whether
    // the file uses CRLF or LF line endings.
    let uses_crlf = original_file.contains("\r\n");
    let content_lf = if uses_crlf {
        original_file.replace("\r\n", "\n")
    } else {
        original_file.clone()
    };
    let old_lf = old_string.replace("\r\n", "\n");
    let new_lf = new_string.replace("\r\n", "\n");

    if !content_lf.contains(old_lf.as_str()) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "old_string not found in file",
        ));
    }

    // Ambiguity guard: when the caller expects a single unique replacement but
    // old_string appears more than once, abort so the model can supply a longer,
    // unambiguous context string rather than silently patching the wrong site.
    if !replace_all {
        let occurrences = content_lf.matches(old_lf.as_str()).count();
        if occurrences > 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "old_string matches {occurrences} locations in the file; \
                     set replace_all=true to replace all occurrences, or provide \
                     a longer old_string that uniquely identifies the target location"
                ),
            ));
        }
    }

    let updated_lf = if replace_all {
        content_lf.replace(old_lf.as_str(), new_lf.as_str())
    } else {
        content_lf.replacen(old_lf.as_str(), new_lf.as_str(), 1)
    };

    // Restore the original line-ending style.
    let updated = if uses_crlf {
        updated_lf.replace('\n', "\r\n")
    } else {
        updated_lf
    };

    atomic_write(&absolute_path, updated.as_bytes())?;
    let last_modified_at = fs::metadata(&absolute_path)
        .ok()
        .and_then(|m| mtime_nanos(&m).ok());

    Ok(EditFileOutput {
        file_path: absolute_path.to_string_lossy().into_owned(),
        old_string: old_string.to_owned(),
        new_string: new_string.to_owned(),
        original_file: original_file.clone(),
        structured_patch: make_patch(&original_file, &updated),
        user_modified: false,
        replace_all,
        git_diff: None,
        last_modified_at,
    })
}

pub fn glob_search(pattern: &str, path: Option<&str>) -> io::Result<GlobSearchOutput> {
    let started = Instant::now();
    let base_dir = path
        .map(normalize_path)
        .transpose()?
        .unwrap_or(std::env::current_dir()?);

    // For absolute patterns, enforce workspace boundary and match full paths.
    // For relative patterns, match each file's path relative to base_dir.
    let is_absolute_pattern = Path::new(pattern).is_absolute();
    if is_absolute_pattern {
        enforce_workspace_boundary(Path::new(pattern))?;
    }

    // Build a glob::Pattern from the raw pattern for per-entry matching.
    // When relative, we match against the relative portion of the path.
    let glob_pat = Pattern::new(pattern)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

    let mut matches = Vec::new();

    // WalkBuilder respects .gitignore, .ignore, and global gitignore.
    // hidden(false) so dotfiles like .env are included; .git internals
    // are excluded by the ignore crate's built-in special-casing.
    for result in WalkBuilder::new(&base_dir)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .build()
    {
        let entry = result.map_err(|e| io::Error::other(e.to_string()))?;
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let full_path = entry.path();

        // Determine what to match the pattern against.
        let match_target: std::borrow::Cow<'_, str> = if is_absolute_pattern {
            full_path.to_string_lossy()
        } else {
            // Match against the path relative to base_dir so that
            // patterns like `**/*.rs` work without needing an absolute prefix.
            match full_path.strip_prefix(&base_dir) {
                Ok(rel) => rel.to_string_lossy(),
                Err(_) => full_path.to_string_lossy(),
            }
        };

        if glob_pat.matches(&match_target) || glob_pat.matches_path(Path::new(&*match_target)) {
            matches.push(full_path.to_path_buf());
        }
    }

    matches.sort_by_key(|p| {
        fs::metadata(p)
            .and_then(|metadata| metadata.modified())
            .ok()
            .map(Reverse)
    });

    let truncated = matches.len() > 100;
    let filenames = matches
        .into_iter()
        .take(100)
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    Ok(GlobSearchOutput {
        duration_ms: started.elapsed().as_millis(),
        num_files: filenames.len(),
        filenames,
        truncated,
    })
}

pub fn grep_search(input: &GrepSearchInput) -> io::Result<GrepSearchOutput> {
    let base_path = input
        .path
        .as_deref()
        .map(normalize_path)
        .transpose()?
        .unwrap_or(std::env::current_dir()?);

    let multiline = input.multiline.unwrap_or(false);
    let context = input.context.or(input.context_short).unwrap_or(0);
    let before = input.before.unwrap_or(context);
    let after = input.after.unwrap_or(context);
    let output_mode = input.output_mode.unwrap_or_default();
    let line_numbers_enabled = input.line_numbers.unwrap_or(true);

    // SIMD-accelerated regex matcher from the ripgrep family.
    // multi_line(true) enables cross-line `.` matching AND lets the regex
    // engine match patterns that span line boundaries.
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(input.case_insensitive.unwrap_or(false))
        .multi_line(multiline)
        .build(&input.pattern)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;

    // Streaming searcher: handles before/after context, binary detection,
    // and mmap-based reading automatically.
    let mut searcher = SearcherBuilder::new()
        .multi_line(multiline)
        .before_context(before)
        .after_context(after)
        .line_number(true)
        .build();

    let glob_filter = input
        .glob
        .as_deref()
        .map(Pattern::new)
        .transpose()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
    let file_type = input.file_type.as_deref();

    let mut all_filenames: Vec<String> = Vec::new();
    let mut all_content_lines: Vec<String> = Vec::new();
    let mut total_matches: usize = 0;

    // WalkBuilder: respects .gitignore / .ignore, hidden files included,
    // .git internals excluded by the crate's built-in special-casing.
    let walker = WalkBuilder::new(&base_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .build();

    for result in walker {
        let entry = result.map_err(|e| io::Error::other(e.to_string()))?;
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.path();

        if !matches_optional_filters(path, glob_filter.as_ref(), file_type) {
            continue;
        }

        let path_str = path.to_string_lossy().into_owned();
        let mut sink = FileSink {
            output_mode,
            path: path_str.clone(),
            line_numbers_enabled,
            has_match: false,
            match_count: 0,
            content_lines: Vec::new(),
        };

        // Errors on individual files (binary, permissions) are silently skipped.
        let _ = searcher.search_path(&matcher, path, &mut sink);

        if sink.has_match {
            all_filenames.push(path_str);
            total_matches += sink.match_count;
            all_content_lines.extend(sink.content_lines);
        }
    }

    let (filenames, applied_limit, applied_offset) =
        apply_limit(all_filenames, input.head_limit, input.offset);

    if output_mode == GrepOutputMode::Content {
        let (lines, limit, offset) = apply_limit(all_content_lines, input.head_limit, input.offset);
        return Ok(GrepSearchOutput {
            mode: Some(GrepOutputMode::Content),
            num_files: filenames.len(),
            filenames,
            num_lines: Some(lines.len()),
            content: Some(lines.join("\n")),
            num_matches: Some(total_matches),
            applied_limit: limit,
            applied_offset: offset,
        });
    }

    Ok(GrepSearchOutput {
        mode: Some(output_mode),
        num_files: filenames.len(),
        filenames,
        content: None,
        num_lines: None,
        num_matches: (output_mode == GrepOutputMode::Count).then_some(total_matches),
        applied_limit,
        applied_offset,
    })
}

/// Per-file sink for `grep_search`. Collects match events from `grep_searcher`.
struct FileSink {
    output_mode: GrepOutputMode,
    path: String,
    line_numbers_enabled: bool,
    has_match: bool,
    match_count: usize,
    content_lines: Vec<String>,
}

impl Sink for FileSink {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, io::Error> {
        self.has_match = true;
        self.match_count += 1;

        if self.output_mode == GrepOutputMode::Content {
            let base_line = mat.line_number();
            // mat.lines() iterates over individual lines within the match
            // (single line in standard mode, possibly multiple in multiline mode).
            for (i, line_bytes) in mat.lines().enumerate() {
                let line = String::from_utf8_lossy(line_bytes);
                let line = line.trim_end_matches('\n').trim_end_matches('\r');
                let formatted = if self.line_numbers_enabled {
                    if let Some(ln) = base_line {
                        format!("{}:{}:{}", self.path, ln + i as u64, line)
                    } else {
                        format!("{}:{}", self.path, line)
                    }
                } else {
                    format!("{}:{}", self.path, line)
                };
                self.content_lines.push(formatted);
            }
        }
        Ok(true)
    }

    fn context(&mut self, _searcher: &Searcher, ctx: &SinkContext<'_>) -> Result<bool, io::Error> {
        if self.output_mode == GrepOutputMode::Content {
            let text = String::from_utf8_lossy(ctx.bytes());
            let line = text.trim_end_matches('\n').trim_end_matches('\r');
            let formatted = if self.line_numbers_enabled {
                if let Some(ln) = ctx.line_number() {
                    format!("{}:{}:{}", self.path, ln, line)
                } else {
                    format!("{}:{}", self.path, line)
                }
            } else {
                format!("{}:{}", self.path, line)
            };
            self.content_lines.push(formatted);
        }
        Ok(true)
    }
}

fn matches_optional_filters(
    path: &Path,
    glob_filter: Option<&Pattern>,
    file_type: Option<&str>,
) -> bool {
    if let Some(pat) = glob_filter {
        let path_str = path.to_string_lossy();
        if !pat.matches(&path_str) && !pat.matches_path(path) {
            // Also try matching just the filename component.
            let fname_match = path
                .file_name()
                .map(|n| pat.matches(&n.to_string_lossy()))
                .unwrap_or(false);
            if !fname_match {
                return false;
            }
        }
    }

    if let Some(ext_filter) = file_type {
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some(ext_filter) {
            return false;
        }
    }

    true
}

fn apply_limit<T>(
    items: Vec<T>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> (Vec<T>, Option<usize>, Option<usize>) {
    let offset_value = offset.unwrap_or(0);
    let mut items = items.into_iter().skip(offset_value).collect::<Vec<_>>();
    let explicit_limit = limit.unwrap_or(250);
    if explicit_limit == 0 {
        return (items, None, (offset_value > 0).then_some(offset_value));
    }

    let truncated = items.len() > explicit_limit;
    items.truncate(explicit_limit);
    (
        items,
        truncated.then_some(explicit_limit),
        (offset_value > 0).then_some(offset_value),
    )
}

/// Produce a unified-style structured patch (hunks with ±3 context lines)
/// using the Myers/LCS algorithm from the `similar` crate.
///
/// Each `StructuredPatchHunk` is equivalent to one unified-diff `@@` section.
fn make_patch(original: &str, updated: &str) -> Vec<StructuredPatchHunk> {
    use similar::{ChangeTag, TextDiff};

    const CONTEXT: usize = 3;

    let diff = TextDiff::from_lines(original, updated);
    let mut hunks = Vec::new();

    for group in diff.grouped_ops(CONTEXT) {
        if group.is_empty() {
            continue;
        }

        // Compute hunk header ranges from the first and last op.
        let first = group.first().expect("non-empty group");
        let last = group.last().expect("non-empty group");

        let old_start = first.old_range().start + 1; // 1-based
        let new_start = first.new_range().start + 1;
        let old_lines = last.old_range().end.saturating_sub(first.old_range().start);
        let new_lines = last.new_range().end.saturating_sub(first.new_range().start);

        let mut lines = Vec::new();
        for op in &group {
            for change in diff.iter_changes(op) {
                let prefix = match change.tag() {
                    ChangeTag::Equal => " ",
                    ChangeTag::Insert => "+",
                    ChangeTag::Delete => "-",
                };
                let value = change.value().trim_end_matches('\n');
                lines.push(format!("{prefix}{value}"));
            }
        }

        hunks.push(StructuredPatchHunk {
            old_start,
            old_lines,
            new_start,
            new_lines,
            lines,
        });
    }

    hunks
}

/// Return the file's modification time as nanoseconds since UNIX epoch.
fn mtime_nanos(metadata: &fs::Metadata) -> io::Result<u64> {
    metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .map_err(|e| io::Error::other(e.to_string()))
}

/// Write `content` to `path` atomically by writing to a sibling temp file
/// and then renaming it.  Prevents partial-write corruption on crashes.
fn atomic_write(path: &Path, content: &[u8]) -> io::Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    // Combine pid + monotonic counter to guarantee uniqueness across concurrent
    // writes even within the same nanosecond.
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_path = parent.join(format!(".codineer-tmp-{}-{}.tmp", std::process::id(), seq));

    fs::write(&tmp_path, content)?;
    fs::rename(&tmp_path, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp_path);
    })
}

fn workspace_root() -> io::Result<PathBuf> {
    if let Ok(override_root) = std::env::var("CODINEER_WORKSPACE_ROOT") {
        return Ok(PathBuf::from(override_root));
    }
    std::env::current_dir()
}

fn enforce_workspace_boundary(resolved: &Path) -> io::Result<()> {
    let root = dunce::canonicalize(workspace_root()?).map_err(|e| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("cannot resolve workspace root: {e}"),
        )
    })?;
    if root.as_os_str().is_empty() || !resolved.starts_with(&root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "path '{}' is outside the workspace root '{}'; access denied",
                resolved.display(),
                root.display(),
            ),
        ));
    }
    Ok(())
}

fn normalize_path(path: &str) -> io::Result<PathBuf> {
    let candidate = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        std::env::current_dir()?.join(path)
    };
    match dunce::canonicalize(&candidate) {
        Ok(resolved) => {
            enforce_workspace_boundary(&resolved)?;
            Ok(resolved)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            // The file does not exist yet; canonicalize() cannot resolve it.
            // Compare the raw candidate against the raw (non-canonical) workspace
            // root so that both sides are consistently unresolved and symlinks do
            // not skew the comparison.  This ensures we return PermissionDenied
            // rather than NotFound for paths that are clearly outside the workspace
            // (e.g. /etc/passwd on Windows where it does not exist).
            let root = workspace_root()?;
            if !candidate.starts_with(&root) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "path '{}' is outside the workspace root '{}'; access denied",
                        candidate.display(),
                        root.display(),
                    ),
                ));
            }
            Err(err)
        }
        Err(err) => Err(err),
    }
}

fn normalize_path_allow_missing(path: &str) -> io::Result<PathBuf> {
    let candidate = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        std::env::current_dir()?.join(path)
    };

    if let Ok(canonical) = dunce::canonicalize(&candidate) {
        enforce_workspace_boundary(&canonical)?;
        return Ok(canonical);
    }

    // Walk up the path to find the deepest existing ancestor, canonicalize it
    // (which resolves UNC prefixes and 8.3 short names on Windows), then
    // re-attach the remaining suffix so the boundary check operates on fully
    // resolved paths and avoids false positives from RUNNER~1 vs runneradmin.
    let mut ancestor = candidate.clone();
    let mut suffix = PathBuf::new();
    loop {
        if let Ok(canonical_ancestor) = dunce::canonicalize(&ancestor) {
            enforce_workspace_boundary(&canonical_ancestor)?;
            return Ok(if suffix.as_os_str().is_empty() {
                canonical_ancestor
            } else {
                canonical_ancestor.join(&suffix)
            });
        }
        match ancestor.file_name() {
            Some(name) => {
                suffix = if suffix.as_os_str().is_empty() {
                    PathBuf::from(name)
                } else {
                    PathBuf::from(name).join(&suffix)
                };
            }
            None => break,
        }
        match ancestor.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                ancestor = parent.to_path_buf();
            }
            _ => break,
        }
    }

    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        edit_file, glob_search, grep_search, read_file, write_file, GrepOutputMode, GrepSearchInput,
    };

    fn workspace_dir() -> std::path::PathBuf {
        std::env::temp_dir().join("codineer-test-workspace")
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        workspace_dir().join(format!("codineer-native-{name}-{unique}"))
    }

    fn allow_temp_workspace() {
        let ws = workspace_dir();
        std::fs::create_dir_all(&ws).ok();
        let ws = ws.canonicalize().unwrap_or(ws);
        std::env::set_var("CODINEER_WORKSPACE_ROOT", ws);
    }

    #[test]
    fn reads_and_writes_files() {
        allow_temp_workspace();
        let path = temp_path("read-write.txt");
        let write_output = write_file(path.to_string_lossy().as_ref(), "one\ntwo\nthree")
            .expect("write should succeed");
        assert_eq!(write_output.kind, "create");

        let read_output = read_file(path.to_string_lossy().as_ref(), Some(1), Some(1))
            .expect("read should succeed");
        assert_eq!(read_output.file.content, "two");
    }

    #[test]
    fn edits_file_contents() {
        allow_temp_workspace();
        let path = temp_path("edit.txt");
        write_file(path.to_string_lossy().as_ref(), "alpha beta alpha")
            .expect("initial write should succeed");
        let output = edit_file(
            path.to_string_lossy().as_ref(),
            "alpha",
            "omega",
            true,
            None,
        )
        .expect("edit should succeed");
        assert!(output.replace_all);
    }

    #[test]
    fn edit_returns_mtime_and_conflict_check_works() {
        allow_temp_workspace();
        let path = temp_path("mtime-conflict.txt");
        let written = write_file(path.to_string_lossy().as_ref(), "hello world")
            .expect("write should succeed");
        let mtime = written.last_modified_at.expect("mtime should be present");

        // Edit with correct mtime succeeds.
        let edited = edit_file(
            path.to_string_lossy().as_ref(),
            "hello",
            "goodbye",
            false,
            Some(mtime),
        )
        .expect("edit with correct mtime should succeed");
        assert!(edited.last_modified_at.is_some());

        // Supplying the old (now stale) mtime must fail.
        let conflict = edit_file(
            path.to_string_lossy().as_ref(),
            "goodbye",
            "hello",
            false,
            Some(mtime), // stale
        );
        assert!(
            conflict.is_err(),
            "edit with stale mtime should be rejected"
        );
    }

    #[test]
    fn edit_preserves_crlf_line_endings() {
        allow_temp_workspace();
        let path = temp_path("crlf.txt");
        let crlf_content = "line one\r\nline two\r\nline three\r\n";
        write_file(path.to_string_lossy().as_ref(), crlf_content)
            .expect("write crlf should succeed");
        // The old_string may be given with LF endings (as the model typically sends).
        let output = edit_file(
            path.to_string_lossy().as_ref(),
            "line two",
            "LINE TWO",
            false,
            None,
        )
        .expect("edit crlf file should succeed");
        let result = std::fs::read_to_string(path).expect("re-read should succeed");
        assert!(
            result.contains("\r\n"),
            "CRLF line endings must be preserved"
        );
        assert!(result.contains("LINE TWO"), "replacement must be applied");
        drop(output);
    }

    #[test]
    fn globs_and_greps_directory() {
        allow_temp_workspace();
        let dir = temp_path("search-dir");
        std::fs::create_dir_all(&dir).expect("directory should be created");
        let file = dir.join("demo.rs");
        write_file(
            file.to_string_lossy().as_ref(),
            "fn main() {\n println!(\"hello\");\n}\n",
        )
        .expect("file write should succeed");

        let globbed = glob_search("**/*.rs", Some(dir.to_string_lossy().as_ref()))
            .expect("glob should succeed");
        assert_eq!(globbed.num_files, 1);

        let grep_output = grep_search(&GrepSearchInput {
            pattern: String::from("hello"),
            path: Some(dir.to_string_lossy().into_owned()),
            glob: Some(String::from("**/*.rs")),
            output_mode: Some(GrepOutputMode::Content),
            before: None,
            after: None,
            context_short: None,
            context: None,
            line_numbers: Some(true),
            case_insensitive: Some(false),
            file_type: None,
            head_limit: Some(10),
            offset: Some(0),
            multiline: Some(false),
        })
        .expect("grep should succeed");
        assert!(grep_output.content.unwrap_or_default().contains("hello"));
    }

    #[test]
    #[cfg(unix)]
    fn rejects_absolute_path_outside_workspace() {
        allow_temp_workspace();
        let result = read_file("/etc/passwd", None, None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    }

    #[test]
    #[cfg(windows)]
    fn rejects_absolute_path_outside_workspace_windows() {
        allow_temp_workspace();
        let result = read_file("C:\\Windows\\System32\\drivers\\etc\\hosts", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_relative_path_traversal_above_workspace() {
        allow_temp_workspace();
        let result = read_file("../../../etc/passwd", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_write_outside_workspace() {
        allow_temp_workspace();
        let sentinel = std::env::temp_dir().join("codineer-test-outside-sentinel");
        let sentinel_str = sentinel.to_string_lossy().to_string();
        let result = write_file(&sentinel_str, "malicious");
        let denied = result.is_err()
            && result
                .as_ref()
                .unwrap_err()
                .kind()
                .eq(&std::io::ErrorKind::PermissionDenied);
        if !denied {
            let _ = std::fs::remove_file(&sentinel);
        }
        assert!(denied, "write outside workspace must be denied");
    }

    #[test]
    fn allows_operations_within_workspace() {
        allow_temp_workspace();
        let path = temp_path("inside-workspace.txt");
        write_file(path.to_string_lossy().as_ref(), "safe content")
            .expect("write within workspace should succeed");
        let read_output = read_file(path.to_string_lossy().as_ref(), None, None)
            .expect("read within workspace should succeed");
        assert_eq!(read_output.file.content, "safe content");
    }

    #[test]
    fn edit_rejects_ambiguous_match_when_replace_all_false() {
        allow_temp_workspace();
        let path = temp_path("ambiguous.txt");
        write_file(path.to_string_lossy().as_ref(), "foo bar\nfoo baz\nfoo qux").expect("write");
        let result = edit_file(
            path.to_string_lossy().as_ref(),
            "foo",
            "FOO",
            false, // single replacement requested
            None,
        );
        assert!(result.is_err(), "ambiguous match should be rejected");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("3"),
            "error should report occurrence count: {msg}"
        );
    }

    #[test]
    fn edit_allows_replace_all_for_multiple_occurrences() {
        allow_temp_workspace();
        let path = temp_path("multi-replace.txt");
        write_file(path.to_string_lossy().as_ref(), "foo foo foo").expect("write");
        let output = edit_file(
            path.to_string_lossy().as_ref(),
            "foo",
            "bar",
            true, // replace_all
            None,
        )
        .expect("replace_all should succeed even with multiple occurrences");
        assert!(output.replace_all);
        let result = std::fs::read_to_string(&path).expect("re-read");
        assert_eq!(result, "bar bar bar");
    }

    #[test]
    fn edit_rejects_identical_old_and_new_string() {
        allow_temp_workspace();
        let path = temp_path("edit-reject.txt");
        write_file(path.to_string_lossy().as_ref(), "content").expect("write");
        let result = edit_file(
            path.to_string_lossy().as_ref(),
            "content",
            "content",
            false,
            None,
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn edit_rejects_missing_old_string() {
        allow_temp_workspace();
        let path = temp_path("edit-missing.txt");
        write_file(path.to_string_lossy().as_ref(), "alpha beta").expect("write");
        let result = edit_file(
            path.to_string_lossy().as_ref(),
            "gamma",
            "delta",
            false,
            None,
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
    }
}
