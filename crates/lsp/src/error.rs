use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug)]
pub enum LspError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidHeader(String),
    MissingContentLength,
    InvalidContentLength(String),
    UnsupportedDocument(PathBuf),
    UnknownServer(String),
    DuplicateExtension {
        extension: String,
        existing_server: String,
        new_server: String,
    },
    PathToUrl(PathBuf),
    Protocol(String),
}

impl Display for LspError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::InvalidHeader(header) => write!(f, "invalid LSP header: {header}"),
            Self::MissingContentLength => write!(f, "missing LSP Content-Length header"),
            Self::InvalidContentLength(value) => {
                write!(f, "invalid LSP Content-Length value: {value}")
            }
            Self::UnsupportedDocument(path) => {
                write!(f, "no LSP server configured for {}", path.display())
            }
            Self::UnknownServer(name) => write!(f, "unknown LSP server: {name}"),
            Self::DuplicateExtension {
                extension,
                existing_server,
                new_server,
            } => write!(
                f,
                "duplicate LSP extension mapping for {extension}: {existing_server} and {new_server}"
            ),
            Self::PathToUrl(path) => write!(f, "failed to convert path to file URL: {}", path.display()),
            Self::Protocol(message) => write!(f, "LSP protocol error: {message}"),
        }
    }
}

impl std::error::Error for LspError {}

impl From<std::io::Error> for LspError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for LspError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_all_variants() {
        let io_err = LspError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "gone"));
        assert!(io_err.to_string().contains("gone"));

        let json_str = "not json";
        let json_err: serde_json::Error = serde_json::from_str::<bool>(json_str).unwrap_err();
        let lsp_json = LspError::Json(json_err);
        assert!(!lsp_json.to_string().is_empty());

        assert!(LspError::InvalidHeader("bad".into()).to_string().contains("invalid LSP header"));
        assert!(LspError::MissingContentLength.to_string().contains("Content-Length"));
        assert!(LspError::InvalidContentLength("abc".into()).to_string().contains("abc"));
        assert!(LspError::UnsupportedDocument(PathBuf::from("foo.xyz")).to_string().contains("foo.xyz"));
        assert!(LspError::UnknownServer("pyright".into()).to_string().contains("pyright"));
        assert!(LspError::DuplicateExtension {
            extension: ".rs".into(),
            existing_server: "rust-analyzer".into(),
            new_server: "rls".into(),
        }.to_string().contains("rust-analyzer"));
        assert!(LspError::PathToUrl(PathBuf::from("/tmp/x")).to_string().contains("/tmp/x"));
        assert!(LspError::Protocol("bad frame".into()).to_string().contains("bad frame"));
    }

    #[test]
    fn from_io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe");
        let lsp: LspError = io_err.into();
        assert!(matches!(lsp, LspError::Io(_)));
    }

    #[test]
    fn from_serde_json_error_converts() {
        let err: serde_json::Error = serde_json::from_str::<i32>("???").unwrap_err();
        let lsp: LspError = err.into();
        assert!(matches!(lsp, LspError::Json(_)));
    }

    #[test]
    fn error_trait_is_implemented() {
        let err = LspError::Protocol("test".into());
        let _: &dyn std::error::Error = &err;
    }
}
