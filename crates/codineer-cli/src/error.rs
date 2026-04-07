//! Structured error types for the CLI application.

/// Top-level error type for the Codineer CLI.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Runtime(#[from] runtime::RuntimeError),

    #[error(transparent)]
    Api(#[from] api::ApiError),

    #[error(transparent)]
    Config(#[from] runtime::ConfigError),

    #[error(transparent)]
    Session(#[from] runtime::SessionError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Plugin(#[from] plugins::PluginError),

    #[error(transparent)]
    PromptBuild(#[from] runtime::PromptBuildError),

    #[error("{0}")]
    Other(String),
}

impl From<Box<dyn std::error::Error>> for CliError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        Self::Other(err.to_string())
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for CliError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::Other(err.to_string())
    }
}

impl From<String> for CliError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<&str> for CliError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_string())
    }
}

/// Convenience type alias for CLI results.
pub type CliResult<T> = Result<T, CliError>;
