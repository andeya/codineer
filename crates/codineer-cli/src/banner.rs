//! Welcome panel (Claude Code-style: border-title + two-column layout).
//!
//! The banner is **re-generated on every resize event** so it always reflects
//! the current terminal width.  Width clamping and column-layout decisions live
//! in [`BannerLayout`]; all rendering helpers accept a `&BannerLayout` so the
//! same logic is exercised by unit tests without touching the terminal.

use std::path::Path;

use crate::style::Palette;
use crate::terminal_width::{
    display_width, fit_display_width, terminal_cols, truncate_display, wrap_by_display_width,
};

// Width of the " │ " column divider (visible chars).
const DIVIDER: usize = 3;
// Visible char width of the ASCII logo art (the widest glyph row).
const LOGO_WIDTH: usize = 12;
// Maximum inner width (content area, excluding the two border chars).
const MAX_INNER_WIDTH: usize = 120;
// Minimum inner width to use the two-column layout; below this the banner
// collapses to a single left column, mirroring Claude Code's behaviour.
const TWO_COL_MIN: usize = 50;

/// Pre-computed column widths for a given terminal size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BannerLayout {
    /// Content width between the two border chars.
    inner_width: usize,
    /// Width of the left column (equals `inner_width` in single-column mode).
    left_col: usize,
    /// Width of the right column (0 in single-column mode).
    right_col: usize,
    /// `true` when `inner_width >= TWO_COL_MIN`.
    two_column: bool,
}

impl BannerLayout {
    /// Build a layout for the current terminal width.
    fn new() -> Self {
        Self::new_with_cols(terminal_cols())
    }

    /// Build a layout for an explicit terminal column count.
    /// Exposed for unit tests so they can run without a real terminal.
    fn new_with_cols(cols: usize) -> Self {
        let inner_width = cols.saturating_sub(2).clamp(10, MAX_INNER_WIDTH);
        if inner_width >= TWO_COL_MIN {
            let left_col = inner_width / 2 - 2;
            Self {
                inner_width,
                left_col,
                right_col: inner_width - left_col - DIVIDER,
                two_column: true,
            }
        } else {
            Self {
                inner_width,
                left_col: inner_width,
                right_col: 0,
                two_column: false,
            }
        }
    }
}

/// Context values needed to render the banner.  All fields are borrowed
/// from the surrounding [`LiveCli`] so no extra allocation is required.
pub(crate) struct BannerContext<'a> {
    pub workspace_summary: &'a str,
    pub cwd_display: &'a str,
    pub model: &'a str,
    pub permissions: &'a str,
    pub session_id: &'a str,
    pub session_path: &'a Path,
    pub has_codineer_md: bool,
}

/// Render the full welcome banner as a `String`.
///
/// Layout is computed from the **current** `terminal_cols()` at call time,
/// so calling this again after a resize produces an adapted result.
pub(crate) fn welcome_banner(color: bool, ctx: BannerContext<'_>) -> String {
    let p = Palette::new(color);
    let layout = BannerLayout::new();
    render_banner(&p, &ctx, &layout)
}

/// Inner renderer that accepts an explicit layout (testable without a terminal).
fn render_banner(p: &Palette, ctx: &BannerContext<'_>, layout: &BannerLayout) -> String {
    let left = left_column(p, ctx, layout);
    let mut rows = Vec::with_capacity(14);
    rows.push(border_top(p, layout));

    if layout.two_column {
        let div = if p.violet.is_empty() {
            " | ".to_string()
        } else {
            format!(" {}│{} ", p.gray, p.r)
        };
        let right = right_column(p, ctx, layout);
        let row_count = left.len().max(right.len());
        for i in 0..row_count {
            let l = left.get(i).map(String::as_str).unwrap_or("");
            let r = right.get(i).map(String::as_str).unwrap_or("");
            rows.push(border_row(
                p,
                &format!(
                    "{}{}{}",
                    fit_display_width(l, layout.left_col),
                    div,
                    fit_display_width(r, layout.right_col),
                ),
            ));
        }
    } else {
        for l in &left {
            rows.push(border_row(p, &fit_display_width(l, layout.inner_width)));
        }
    }

    rows.push(border_bottom(p, layout));
    rows.join("\n")
}

/// Center `text` in a field of `width` visible columns, padding with spaces.
fn center_in(text: &str, width: usize) -> String {
    let w = display_width(text);
    if w >= width {
        return truncate_display(text, width);
    }
    let pad_left = (width - w) / 2;
    let pad_right = width - w - pad_left;
    format!("{}{text}{}", " ".repeat(pad_left), " ".repeat(pad_right))
}

fn border_top(p: &Palette, layout: &BannerLayout) -> String {
    let ver = crate::VERSION;
    // "─ Codineer vX.Y.Z " — fixed prefix/suffix cost in visible chars.
    let title_visible = 13 + ver.len();
    let bar = layout.inner_width.saturating_sub(title_visible);
    if p.violet.is_empty() {
        format!("+- Codineer v{ver} {}+", "-".repeat(bar))
    } else if layout.inner_width >= title_visible {
        format!(
            "{v}╭─ Codineer{r} {g}v{ver}{r} {v}{bar}╮{r}",
            v = p.violet,
            g = p.gray,
            r = p.r,
            bar = "─".repeat(bar),
        )
    } else {
        format!(
            "{v}╭─ {title}╮{r}",
            v = p.violet,
            r = p.r,
            title = truncate_display(
                &format!("Codineer v{ver}"),
                layout.inner_width.saturating_sub(2),
            ),
        )
    }
}

fn border_bottom(p: &Palette, layout: &BannerLayout) -> String {
    if p.violet.is_empty() {
        format!("+{}+", "-".repeat(layout.inner_width))
    } else {
        format!("{}╰{}╯{}", p.violet, "─".repeat(layout.inner_width), p.r)
    }
}

fn border_row(p: &Palette, inner: &str) -> String {
    if p.violet.is_empty() {
        format!("|{inner}|")
    } else {
        format!("{v}│{r}{inner}{v}│{r}", v = p.violet, r = p.r)
    }
}

fn left_column(p: &Palette, ctx: &BannerContext<'_>, layout: &BannerLayout) -> Vec<String> {
    let lc = layout.left_col;
    let logo = [
        format!("{}    ▄██▄{}", p.violet, p.r),
        format!("{} ▄██▀  ▀██▄{}", p.violet, p.r),
        format!("{}██  {}❯{}     ██{}", p.violet, p.cyan_fg, p.violet, p.r),
        format!("{}██     {}▍{}  ██{}", p.violet, p.amber, p.violet, p.r),
        format!("{} ▀██▄  ▄██▀{}", p.violet, p.r),
        format!("{}    ▀██▀{}", p.violet, p.r),
    ];
    let mut lines = vec![
        center_in(
            &format!("{}Welcome back · Codineer{}", p.bold_white, p.r),
            lc,
        ),
        String::new(),
    ];
    if lc >= LOGO_WIDTH {
        for l in &logo {
            lines.push(center_in(&fit_display_width(l, LOGO_WIDTH), lc));
        }
    }
    lines.push(center_in(
        &format!("{}{} · {}{}", p.dim, ctx.model, ctx.permissions, p.r),
        lc,
    ));
    lines.push(center_in(
        &format!(
            "{}{}{}",
            p.dim,
            truncate_display(ctx.cwd_display, lc.saturating_sub(4)),
            p.r
        ),
        lc,
    ));
    lines
}

fn right_column(p: &Palette, ctx: &BannerContext<'_>, layout: &BannerLayout) -> Vec<String> {
    let rc = layout.right_col;
    let header = |text: &str| {
        if p.violet.is_empty() {
            text.to_string()
        } else {
            format!("{}{text}{}", p.violet, p.r)
        }
    };
    let tips: &[&str] = if ctx.has_codineer_md {
        if crate::platform::vim_installed() {
            &["/help · Tab completes slash", "/vim for modal edit"]
        } else {
            &["/help · Tab completes slash", "? for shortcuts"]
        }
    } else {
        &["/init · /help · /status", "— then ask for a task"]
    };
    let resume = tilde_session_path(ctx.session_path);
    let separator = if p.violet.is_empty() {
        "-".repeat(rc)
    } else {
        format!("{}{}{}", p.dim, "─".repeat(rc), p.r)
    };
    let tr = |s: &str| truncate_display(s, rc.saturating_sub(1));
    let mut lines = vec![header("Tips for getting started")];
    lines.extend(tips.iter().map(|t| format!(" {t}")));
    lines.extend([
        separator,
        header("Session"),
        format!(" {}", tr(ctx.session_id)),
        format!(" {}", tr(ctx.workspace_summary)),
        String::new(),
        header("Resume"),
    ]);
    // Wrap the resume command so the full session path is always visible
    // even on narrow terminals.  Each continuation line is indented by two
    // spaces so it reads as a natural continuation of the command.
    let resume_cmd = format!("codineer --resume {}", resume.display());
    let wrap_width = rc.saturating_sub(1); // account for leading ' '
    let chunks = wrap_by_display_width(&resume_cmd, wrap_width.max(10));
    for (i, chunk) in chunks.iter().enumerate() {
        let indent = if i == 0 { " " } else { "  " };
        lines.push(format!("{indent}{chunk}"));
    }
    lines
}

pub(crate) fn tilde_session_path(path: &Path) -> std::path::PathBuf {
    let Some(home) = runtime::home_dir() else {
        return path.to_path_buf();
    };
    if path.starts_with(&home) {
        std::path::PathBuf::from("~").join(path.strip_prefix(&home).unwrap_or(path))
    } else {
        path.to_path_buf()
    }
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Helper: a no-color palette.
    fn p() -> Palette {
        Palette::new(false)
    }

    fn ctx<'a>(
        model: &'a str,
        permissions: &'a str,
        session_id: &'a str,
        session_path: &'a std::path::Path,
    ) -> BannerContext<'a> {
        BannerContext {
            workspace_summary: "myproject · main",
            cwd_display: "/home/user/myproject",
            model,
            permissions,
            session_id,
            session_path,
            has_codineer_md: false,
        }
    }

    // ── BannerLayout ─────────────────────────────────────────────────────────

    #[test]
    fn layout_two_column_at_wide_terminal() {
        let l = BannerLayout::new_with_cols(120);
        assert!(l.two_column);
        assert_eq!(l.inner_width, 118); // 120 - 2
        assert_eq!(l.left_col + DIVIDER + l.right_col, l.inner_width);
    }

    #[test]
    fn layout_single_column_at_narrow_terminal() {
        let l = BannerLayout::new_with_cols(40);
        assert!(!l.two_column);
        assert_eq!(l.left_col, l.inner_width);
        assert_eq!(l.right_col, 0);
    }

    #[test]
    fn layout_switches_at_two_col_min_boundary() {
        // Exactly TWO_COL_MIN inner width → two-column.
        // inner_width = cols - 2, so cols = TWO_COL_MIN + 2.
        let at = BannerLayout::new_with_cols(TWO_COL_MIN + 2);
        assert!(at.two_column, "should be two-column at threshold");

        // One column narrower → single-column.
        let below = BannerLayout::new_with_cols(TWO_COL_MIN + 1);
        assert!(!below.two_column, "should be single-column below threshold");
    }

    #[test]
    fn layout_clamps_inner_width_to_max() {
        let l = BannerLayout::new_with_cols(999);
        assert_eq!(l.inner_width, MAX_INNER_WIDTH);
    }

    #[test]
    fn layout_clamps_inner_width_to_minimum_ten() {
        let l = BannerLayout::new_with_cols(0);
        assert_eq!(l.inner_width, 10);
    }

    // ── center_in ────────────────────────────────────────────────────────────

    #[test]
    fn center_in_pads_short_string() {
        let result = center_in("hi", 10);
        assert_eq!(result.len(), 10);
        assert!(result.contains("hi"));
        // left padding >= right padding (floor division)
        let left_pad = result.find("hi").unwrap();
        let right_pad = 10 - left_pad - 2;
        assert!(left_pad <= right_pad + 1);
    }

    #[test]
    fn center_in_truncates_long_string() {
        let long = "abcdefghijklmnopqrstuvwxyz";
        let result = center_in(long, 5);
        assert_eq!(display_width(&result), 5);
    }

    #[test]
    fn center_in_exact_fit_is_unchanged() {
        let result = center_in("hello", 5);
        assert_eq!(result, "hello");
    }

    // ── border_top ───────────────────────────────────────────────────────────

    #[test]
    fn border_top_no_color_normal_width() {
        let l = BannerLayout::new_with_cols(100);
        let top = border_top(&p(), &l);
        assert!(top.starts_with('+'));
        assert!(top.ends_with('+'));
        assert!(top.contains("Codineer"));
    }

    #[test]
    fn border_top_very_narrow_does_not_panic() {
        let l = BannerLayout::new_with_cols(0); // inner_width = 10
        let top = border_top(&p(), &l);
        // Must not panic and must be non-empty.
        assert!(!top.is_empty());
    }

    // ── render_banner (welcome_banner without terminal) ──────────────────────

    #[test]
    fn render_banner_two_column_has_divider() {
        let l = BannerLayout::new_with_cols(120);
        let path = PathBuf::from("/tmp/session");
        let c = ctx("claude-3-5", "read-only", "abc123", &path);
        let out = render_banner(&p(), &c, &l);
        assert!(out.contains('|'), "divider char expected in no-color mode");
    }

    #[test]
    fn render_banner_single_column_no_divider() {
        let l = BannerLayout::new_with_cols(30); // single-column
        let path = PathBuf::from("/tmp/session");
        let c = ctx("gpt-4o", "workspace-write", "xyz789", &path);
        let out = render_banner(&p(), &c, &l);
        // Divider " | " should not appear (no two-column layout).
        // Each line starts and ends with '|' border, but " | " mid-line is absent.
        let lines: Vec<&str> = out.lines().collect();
        for line in &lines[1..lines.len() - 1] {
            // Strip leading/trailing '|' and check no mid-divider.
            let inner = &line[1..line.len() - 1];
            assert!(!inner.contains(" | "), "mid-divider in single-column mode");
        }
    }

    #[test]
    fn render_banner_lines_count_is_consistent() {
        let path = PathBuf::from("/tmp/s");
        for cols in [30, 60, 80, 120, 200] {
            let l = BannerLayout::new_with_cols(cols);
            let c = ctx("m", "p", "id", &path);
            let out = render_banner(&p(), &c, &l);
            let lines: Vec<&str> = out.lines().collect();
            // First and last must be border_top / border_bottom.
            assert!(
                lines
                    .first()
                    .is_some_and(|l| l.starts_with('+') || l.starts_with('╭')),
                "cols={cols}: first line should be border_top"
            );
            assert!(
                lines
                    .last()
                    .is_some_and(|l| l.starts_with('+') || l.starts_with('╰')),
                "cols={cols}: last line should be border_bottom"
            );
        }
    }

    #[test]
    fn render_banner_contains_model_and_session_id() {
        let l = BannerLayout::new_with_cols(100);
        let path = PathBuf::from("/tmp/s");
        let c = ctx("my-model-name", "read", "unique-session-xyz", &path);
        let out = render_banner(&p(), &c, &l);
        assert!(out.contains("my-model-name"));
        assert!(out.contains("unique-session-xyz"));
    }
}
