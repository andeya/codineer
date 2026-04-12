use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Settings: {0}")]
    Settings(String),

    #[error("Shell: {0}")]
    Shell(String),

    #[error("Git: {0}")]
    Git(String),

    #[error("AI: {0}")]
    Ai(String),

    #[error("Agent: {0}")]
    Agent(String),

    #[error("IO: {0}")]
    Io(#[from] std::io::Error),

    #[error("Cache: {0}")]
    Cache(String),

    #[error("File: {0}")]
    File(String),

    #[error("Session: {0}")]
    Session(String),

    #[error("Plugin: {0}")]
    Plugin(String),

    #[error("Lsp: {0}")]
    Lsp(String),

    #[error("Mcp: {0}")]
    Mcp(String),

    #[error("Gateway: {0}")]
    Gateway(String),

    #[error("Memory: {0}")]
    Memory(String),

    #[error("Channel: {0}")]
    Channel(String),

    #[error("Update: {0}")]
    Update(String),

    #[error("Not implemented: {0}")]
    NotImplemented(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
