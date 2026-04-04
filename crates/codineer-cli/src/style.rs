use std::io::IsTerminal;

/// Unified color / TTY detection for the CLI.
///
/// Respects `NO_COLOR` (https://no-color.org/), `CLICOLOR`, and TTY status.
/// All modules should use this instead of ad-hoc checks.
pub(crate) fn color_for_stdout() -> bool {
    std::io::stdout().is_terminal() && color_env_allows()
}

pub(crate) fn color_for_stderr() -> bool {
    std::io::stderr().is_terminal() && color_env_allows()
}

fn color_env_allows() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if let Some(val) = std::env::var_os("CLICOLOR") {
        return val != "0";
    }
    true
}

/// Pre-computed ANSI escape codes.
///
/// Construct via `Palette::for_stdout()` or `Palette::for_stderr()`.
/// When color is disabled, all fields are empty strings.
#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub(crate) dim: &'static str,
    pub(crate) r: &'static str,
    pub(crate) bold_cyan: &'static str,
    pub(crate) bold_green: &'static str,
    pub(crate) bold_yellow: &'static str,
    pub(crate) bold_red: &'static str,
    pub(crate) bold_white: &'static str,
    pub(crate) gray: &'static str,
    pub(crate) red_fg: &'static str,
    pub(crate) green_fg: &'static str,
    pub(crate) violet: &'static str,
    pub(crate) cyan_fg: &'static str,
    pub(crate) amber: &'static str,
    pub(crate) bash_bg: &'static str,
}

impl Palette {
    pub(crate) fn for_stdout() -> Self {
        Self::new(color_for_stdout())
    }

    pub(crate) fn for_stderr() -> Self {
        Self::new(color_for_stderr())
    }

    pub(crate) fn new(color: bool) -> Self {
        if color {
            Self {
                dim: "\x1b[2m",
                r: "\x1b[0m",
                bold_cyan: "\x1b[1;36m",
                bold_green: "\x1b[1;32m",
                bold_yellow: "\x1b[1;33m",
                bold_red: "\x1b[1;31m",
                bold_white: "\x1b[1;97m",
                gray: "\x1b[38;5;245m",
                red_fg: "\x1b[38;5;203m",
                green_fg: "\x1b[38;5;70m",
                violet: "\x1b[38;5;99m",
                cyan_fg: "\x1b[38;5;81m",
                amber: "\x1b[38;5;214m",
                bash_bg: "\x1b[48;5;236;38;5;255m",
            }
        } else {
            Self {
                dim: "",
                r: "",
                bold_cyan: "",
                bold_green: "",
                bold_yellow: "",
                bold_red: "",
                bold_white: "",
                gray: "",
                red_fg: "",
                green_fg: "",
                violet: "",
                cyan_fg: "",
                amber: "",
                bash_bg: "",
            }
        }
    }

    /// Bold cyan section title.
    pub(crate) fn title(&self, text: &str) -> String {
        format!("{}{text}{}", self.bold_cyan, self.r)
    }

    /// Dim text for secondary information.
    pub(crate) fn dim_text(&self, text: &str) -> String {
        format!("{}{text}{}", self.dim, self.r)
    }

    /// Truncation notice for long output.
    pub(crate) fn truncation_notice(&self) -> String {
        format!(
            "{}… output truncated for display; full result preserved in session.{}",
            self.dim, self.r
        )
    }
}
