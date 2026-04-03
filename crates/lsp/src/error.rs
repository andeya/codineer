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
    PayloadTooLarge {
        content_length: usize,
        limit: usize,
    },
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
            Self::PayloadTooLarge {
                content_length,
                limit,
            } => write!(
                f,
                "LSP payload too large: Content-Length {content_length} exceeds {limit} byte limit"
            ),
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
        let json_display = LspError::Json(json_err);
        assert!(!json_display.to_string().is_empty());

        assert_eq!(
            LspError::InvalidHeader("bad".into()).to_string(),
            "invalid LSP header: bad"
        );
        assert_eq!(
            LspError::MissingContentLength.to_string(),
            "missing LSP Content-Length header"
        );
        assert_eq!(
            LspError::InvalidContentLength("xyz".into()).to_string(),
            "invalid LSP Content-Length value: xyz"
        );
        assert!(LspError::UnsupportedDocument(PathBuf::from("/foo.txt"))
            .to_string()
            .contains("/foo.txt"));
        assert_eq!(
            LspError::UnknownServer("rust-analyzer".into()).to_string(),
            "unknown LSP server: rust-analyzer"
        );
        assert!(LspError::DuplicateExtension {
            extension: ".rs".into(),
            existing_server: "a".into(),
            new_server: "b".into(),
        }
        .to_string()
        .contains(".rs"));
        assert!(LspError::PathToUrl(PathBuf::from("/bad"))
            .to_string()
            .contains("/bad"));
        assert_eq!(
            LspError::Protocol("timeout".into()).to_string(),
            "LSP protocol error: timeout"
        );
        assert!(LspError::PayloadTooLarge {
            content_length: 100_000_000,
            limit: 8_000_000,
        }
        .to_string()
        .contains("100000000"));
    }

    #[test]
    fn from_io_error_converts() {
        let err: LspError = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe").into();
        assert!(matches!(err, LspError::Io(_)));
    }

    #[test]
    fn from_json_error_converts() {
        let json_err: serde_json::Error = serde_json::from_str::<bool>("x").unwrap_err();
        let err: LspError = json_err.into();
        assert!(matches!(err, LspError::Json(_)));
    }
}
