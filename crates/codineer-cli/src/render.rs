use std::fmt::Write as FmtWrite;
use std::io::{self, Write};

use crossterm::cursor::{MoveToColumn, RestorePosition, SavePosition};
use crossterm::style::{Color, Print, ResetColor, SetForegroundColor, Stylize};
use crossterm::terminal::{Clear, ClearType};
use crossterm::{execute, queue};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

fn color_enabled() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if let Some(val) = std::env::var_os("CLICOLOR") {
        return val != "0";
    }
    true
}

fn styled(text: &str, color: Color) -> String {
    if color_enabled() {
        format!("{}", text.with(color))
    } else {
        text.to_string()
    }
}

fn styled_bold(text: &str, color: Color) -> String {
    if color_enabled() {
        format!("{}", text.bold().with(color))
    } else {
        text.to_string()
    }
}

fn styled_underlined(text: &str, color: Color) -> String {
    if color_enabled() {
        format!("{}", text.underlined().with(color))
    } else {
        text.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTheme {
    heading: Color,
    emphasis: Color,
    strong: Color,
    inline_code: Color,
    link: Color,
    quote: Color,
    table_border: Color,
    code_block_border: Color,
    spinner_active: Color,
    spinner_done: Color,
    spinner_failed: Color,
}

impl Default for ColorTheme {
    fn default() -> Self {
        Self {
            heading: Color::Cyan,
            emphasis: Color::Magenta,
            strong: Color::Yellow,
            inline_code: Color::Green,
            link: Color::Blue,
            quote: Color::DarkGrey,
            table_border: Color::DarkCyan,
            code_block_border: Color::DarkGrey,
            spinner_active: Color::Blue,
            spinner_done: Color::Green,
            spinner_failed: Color::Red,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Spinner {
    frame_index: usize,
}

impl Spinner {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(
        &mut self,
        label: &str,
        theme: &ColorTheme,
        out: &mut impl Write,
    ) -> io::Result<()> {
        let frame = Self::FRAMES[self.frame_index % Self::FRAMES.len()];
        self.frame_index += 1;
        if color_enabled() {
            queue!(
                out,
                SavePosition,
                MoveToColumn(0),
                Clear(ClearType::CurrentLine),
                SetForegroundColor(theme.spinner_active),
                Print(format!("{frame} {label}")),
                ResetColor,
                RestorePosition
            )?;
        } else {
            queue!(
                out,
                SavePosition,
                MoveToColumn(0),
                Clear(ClearType::CurrentLine),
                Print(format!("{frame} {label}")),
                RestorePosition
            )?;
        }
        out.flush()
    }

    pub fn finish(
        &mut self,
        label: &str,
        theme: &ColorTheme,
        out: &mut impl Write,
    ) -> io::Result<()> {
        self.frame_index = 0;
        if color_enabled() {
            execute!(
                out,
                MoveToColumn(0),
                Clear(ClearType::CurrentLine),
                SetForegroundColor(theme.spinner_done),
                Print(format!("✔ {label}\n")),
                ResetColor
            )?;
        } else {
            execute!(
                out,
                MoveToColumn(0),
                Clear(ClearType::CurrentLine),
                Print(format!("✔ {label}\n"))
            )?;
        }
        out.flush()
    }

    pub fn fail(
        &mut self,
        label: &str,
        theme: &ColorTheme,
        out: &mut impl Write,
    ) -> io::Result<()> {
        self.frame_index = 0;
        if color_enabled() {
            execute!(
                out,
                MoveToColumn(0),
                Clear(ClearType::CurrentLine),
                SetForegroundColor(theme.spinner_failed),
                Print(format!("✘ {label}\n")),
                ResetColor
            )?;
        } else {
            execute!(
                out,
                MoveToColumn(0),
                Clear(ClearType::CurrentLine),
                Print(format!("✘ {label}\n"))
            )?;
        }
        out.flush()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ListKind {
    Unordered,
    Ordered { next_index: u64 },
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct TableState {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    current_row: Vec<String>,
    current_cell: String,
    in_head: bool,
}

impl TableState {
    fn push_cell(&mut self) {
        let cell = self.current_cell.trim().to_string();
        self.current_row.push(cell);
        self.current_cell.clear();
    }

    fn finish_row(&mut self) {
        if self.current_row.is_empty() {
            return;
        }
        let row = std::mem::take(&mut self.current_row);
        if self.in_head {
            self.headers = row;
        } else {
            self.rows.push(row);
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct RenderState {
    emphasis: usize,
    strong: usize,
    heading_level: Option<u8>,
    quote: usize,
    list_stack: Vec<ListKind>,
    link_stack: Vec<LinkState>,
    table: Option<TableState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkState {
    destination: String,
    text: String,
}

impl RenderState {
    fn style_text(&self, text: &str, theme: &ColorTheme) -> String {
        if !color_enabled() {
            return text.to_string();
        }

        let mut style = text.stylize();

        if matches!(self.heading_level, Some(1 | 2)) || self.strong > 0 {
            style = style.bold();
        }
        if self.emphasis > 0 {
            style = style.italic();
        }

        if let Some(level) = self.heading_level {
            style = match level {
                1 => style.with(theme.heading),
                2 => style.white(),
                3 => style.with(Color::Blue),
                _ => style.with(Color::Grey),
            };
        } else if self.strong > 0 {
            style = style.with(theme.strong);
        } else if self.emphasis > 0 {
            style = style.with(theme.emphasis);
        }

        if self.quote > 0 {
            style = style.with(theme.quote);
        }

        format!("{style}")
    }

    fn append_raw(&mut self, output: &mut String, text: &str) {
        if let Some(link) = self.link_stack.last_mut() {
            link.text.push_str(text);
        } else if let Some(table) = self.table.as_mut() {
            table.current_cell.push_str(text);
        } else {
            output.push_str(text);
        }
    }

    fn append_styled(&mut self, output: &mut String, text: &str, theme: &ColorTheme) {
        let styled = self.style_text(text, theme);
        self.append_raw(output, &styled);
    }
}

#[derive(Debug)]
pub struct TerminalRenderer {
    syntax_set: SyntaxSet,
    syntax_theme: Theme,
    color_theme: ColorTheme,
}

impl Default for TerminalRenderer {
    fn default() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let syntax_theme = ThemeSet::load_defaults()
            .themes
            .remove("base16-ocean.dark")
            .unwrap_or_default();
        Self {
            syntax_set,
            syntax_theme,
            color_theme: ColorTheme::default(),
        }
    }
}

impl TerminalRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn color_theme(&self) -> &ColorTheme {
        &self.color_theme
    }

    #[must_use]
    pub fn render_markdown(&self, markdown: &str) -> String {
        let mut output = String::new();
        let mut state = RenderState::default();
        let mut code_language = String::new();
        let mut code_buffer = String::new();
        let mut in_code_block = false;

        for event in Parser::new_ext(markdown, Options::all()) {
            self.render_event(
                event,
                &mut state,
                &mut output,
                &mut code_buffer,
                &mut code_language,
                &mut in_code_block,
            );
        }

        output.trim_end().to_string()
    }

    #[must_use]
    pub fn markdown_to_ansi(&self, markdown: &str) -> String {
        self.render_markdown(markdown)
    }

    #[allow(clippy::too_many_lines)]
    fn render_event(
        &self,
        event: Event<'_>,
        state: &mut RenderState,
        output: &mut String,
        code_buffer: &mut String,
        code_language: &mut String,
        in_code_block: &mut bool,
    ) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                Self::start_heading(state, level as u8, output);
            }
            Event::End(TagEnd::Paragraph) => output.push_str("\n\n"),
            Event::Start(Tag::BlockQuote(..)) => self.start_quote(state, output),
            Event::End(TagEnd::BlockQuote(..)) => {
                state.quote = state.quote.saturating_sub(1);
                output.push('\n');
            }
            Event::End(TagEnd::Heading(..)) => {
                state.heading_level = None;
                output.push_str("\n\n");
            }
            Event::End(TagEnd::Item) | Event::SoftBreak | Event::HardBreak => {
                state.append_raw(output, "\n");
            }
            Event::Start(Tag::List(first_item)) => {
                let kind = match first_item {
                    Some(index) => ListKind::Ordered { next_index: index },
                    None => ListKind::Unordered,
                };
                state.list_stack.push(kind);
            }
            Event::End(TagEnd::List(..)) => {
                state.list_stack.pop();
                output.push('\n');
            }
            Event::Start(Tag::Item) => Self::start_item(state, output),
            Event::Start(Tag::CodeBlock(kind)) => {
                *in_code_block = true;
                *code_language = match kind {
                    CodeBlockKind::Indented => String::from("text"),
                    CodeBlockKind::Fenced(lang) => lang.to_string(),
                };
                code_buffer.clear();
                self.start_code_block(code_language, output);
            }
            Event::End(TagEnd::CodeBlock) => {
                self.finish_code_block(code_buffer, code_language, output);
                *in_code_block = false;
                code_language.clear();
                code_buffer.clear();
            }
            Event::Start(Tag::Emphasis) => state.emphasis += 1,
            Event::End(TagEnd::Emphasis) => state.emphasis = state.emphasis.saturating_sub(1),
            Event::Start(Tag::Strong) => state.strong += 1,
            Event::End(TagEnd::Strong) => state.strong = state.strong.saturating_sub(1),
            Event::Code(code) => {
                let rendered = styled(&format!("`{code}`"), self.color_theme.inline_code);
                state.append_raw(output, &rendered);
            }
            Event::Rule => output.push_str("---\n"),
            Event::Text(text) => {
                self.push_text(text.as_ref(), state, output, code_buffer, *in_code_block);
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                state.append_raw(output, &html);
            }
            Event::FootnoteReference(reference) => {
                state.append_raw(output, &format!("[{reference}]"));
            }
            Event::TaskListMarker(done) => {
                state.append_raw(output, if done { "[x] " } else { "[ ] " });
            }
            Event::InlineMath(math) | Event::DisplayMath(math) => {
                state.append_raw(output, &math);
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                state.link_stack.push(LinkState {
                    destination: dest_url.to_string(),
                    text: String::new(),
                });
            }
            Event::End(TagEnd::Link) => {
                if let Some(link) = state.link_stack.pop() {
                    let label = if link.text.is_empty() {
                        link.destination.clone()
                    } else {
                        link.text
                    };
                    let rendered = styled_underlined(
                        &format!("[{label}]({})", link.destination),
                        self.color_theme.link,
                    );
                    state.append_raw(output, &rendered);
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let rendered = styled(&format!("[image:{dest_url}]"), self.color_theme.link);
                state.append_raw(output, &rendered);
            }
            Event::Start(Tag::Table(..)) => state.table = Some(TableState::default()),
            Event::End(TagEnd::Table) => {
                if let Some(table) = state.table.take() {
                    output.push_str(&self.render_table(&table));
                    output.push_str("\n\n");
                }
            }
            Event::Start(Tag::TableHead) => {
                if let Some(table) = state.table.as_mut() {
                    table.in_head = true;
                }
            }
            Event::End(TagEnd::TableHead) => {
                if let Some(table) = state.table.as_mut() {
                    table.finish_row();
                    table.in_head = false;
                }
            }
            Event::Start(Tag::TableRow) => {
                if let Some(table) = state.table.as_mut() {
                    table.current_row.clear();
                    table.current_cell.clear();
                }
            }
            Event::End(TagEnd::TableRow) => {
                if let Some(table) = state.table.as_mut() {
                    table.finish_row();
                }
            }
            Event::Start(Tag::TableCell) => {
                if let Some(table) = state.table.as_mut() {
                    table.current_cell.clear();
                }
            }
            Event::End(TagEnd::TableCell) => {
                if let Some(table) = state.table.as_mut() {
                    table.push_cell();
                }
            }
            Event::Start(Tag::Paragraph | Tag::MetadataBlock(..) | _)
            | Event::End(TagEnd::Image | TagEnd::MetadataBlock(..) | _) => {}
        }
    }

    fn start_heading(state: &mut RenderState, level: u8, output: &mut String) {
        state.heading_level = Some(level);
        if !output.is_empty() {
            output.push('\n');
        }
    }

    fn start_quote(&self, state: &mut RenderState, output: &mut String) {
        state.quote += 1;
        let _ = write!(output, "{}", styled("│ ", self.color_theme.quote));
    }

    fn start_item(state: &mut RenderState, output: &mut String) {
        let depth = state.list_stack.len().saturating_sub(1);
        output.push_str(&"  ".repeat(depth));

        let marker = match state.list_stack.last_mut() {
            Some(ListKind::Ordered { next_index }) => {
                let value = *next_index;
                *next_index += 1;
                format!("{value}. ")
            }
            _ => "• ".to_string(),
        };
        output.push_str(&marker);
    }

    fn start_code_block(&self, code_language: &str, output: &mut String) {
        let label = if code_language.is_empty() {
            "code".to_string()
        } else {
            code_language.to_string()
        };
        let _ = writeln!(
            output,
            "{}",
            styled_bold(&format!("╭─ {label}"), self.color_theme.code_block_border)
        );
    }

    fn finish_code_block(&self, code_buffer: &str, code_language: &str, output: &mut String) {
        output.push_str(&self.highlight_code(code_buffer, code_language));
        let _ = write!(
            output,
            "{}",
            styled_bold("╰─", self.color_theme.code_block_border)
        );
        output.push_str("\n\n");
    }

    fn push_text(
        &self,
        text: &str,
        state: &mut RenderState,
        output: &mut String,
        code_buffer: &mut String,
        in_code_block: bool,
    ) {
        if in_code_block {
            code_buffer.push_str(text);
        } else {
            state.append_styled(output, text, &self.color_theme);
        }
    }

    fn render_table(&self, table: &TableState) -> String {
        let mut rows = Vec::new();
        if !table.headers.is_empty() {
            rows.push(table.headers.clone());
        }
        rows.extend(table.rows.iter().cloned());

        if rows.is_empty() {
            return String::new();
        }

        let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
        let widths = (0..column_count)
            .map(|column| {
                rows.iter()
                    .filter_map(|row| row.get(column))
                    .map(|cell| visible_width(cell))
                    .max()
                    .unwrap_or(0)
            })
            .collect::<Vec<_>>();

        let border = styled("│", self.color_theme.table_border);
        let separator = widths
            .iter()
            .map(|width| "─".repeat(*width + 2))
            .collect::<Vec<_>>()
            .join(&styled("┼", self.color_theme.table_border));
        let separator = format!("{border}{separator}{border}");

        let mut output = String::new();
        if !table.headers.is_empty() {
            output.push_str(&self.render_table_row(&table.headers, &widths, true));
            output.push('\n');
            output.push_str(&separator);
            if !table.rows.is_empty() {
                output.push('\n');
            }
        }

        for (index, row) in table.rows.iter().enumerate() {
            output.push_str(&self.render_table_row(row, &widths, false));
            if index + 1 < table.rows.len() {
                output.push('\n');
            }
        }

        output
    }

    fn render_table_row(&self, row: &[String], widths: &[usize], is_header: bool) -> String {
        let border = styled("│", self.color_theme.table_border);
        let mut line = String::new();
        line.push_str(&border);

        for (index, width) in widths.iter().enumerate() {
            let cell = row.get(index).map_or("", String::as_str);
            line.push(' ');
            if is_header {
                let _ = write!(line, "{}", styled_bold(cell, self.color_theme.heading));
            } else {
                line.push_str(cell);
            }
            let padding = width.saturating_sub(visible_width(cell));
            line.push_str(&" ".repeat(padding + 1));
            line.push_str(&border);
        }

        line
    }

    #[must_use]
    pub fn highlight_code(&self, code: &str, language: &str) -> String {
        if !color_enabled() {
            return code.to_string();
        }

        let syntax = self
            .syntax_set
            .find_syntax_by_token(language)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        let mut syntax_highlighter = HighlightLines::new(syntax, &self.syntax_theme);
        let mut colored_output = String::new();

        for line in LinesWithEndings::from(code) {
            match syntax_highlighter.highlight_line(line, &self.syntax_set) {
                Ok(ranges) => {
                    let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
                    colored_output.push_str(&apply_code_block_background(&escaped));
                }
                Err(_) => colored_output.push_str(&apply_code_block_background(line)),
            }
        }

        colored_output
    }

