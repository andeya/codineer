//! Structured tool output and error types.
//!
//! Replaces the `Result<String, String>` convention with typed results
//! that carry metadata and structured error codes.

/// Successful tool execution result.
#[derive(Debug, Clone)]
pub struct ToolOutput {
    pub content: String,
    pub is_error: bool,
    pub metadata: Option<serde_json::Value>,
}

impl ToolOutput {
    /// Create a successful output with content.
    #[must_use]
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
            metadata: None,
        }
    }

    /// Create a tool-level error output (tool ran but reported a problem).
    #[must_use]
    pub fn error(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
            metadata: None,
        }
    }

    /// Attach arbitrary JSON metadata.
    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Tool execution error.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("unsupported tool: {name}")]
    Unsupported { name: String },
    #[error("input deserialization: {0}")]
    InputError(#[from] serde_json::Error),
    #[error("execution failed: {message}")]
    Execution { message: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("permission denied: {reason}")]
    PermissionDenied { reason: String },
}

impl ToolError {
    pub fn execution(message: impl Into<String>) -> Self {
        Self::Execution {
            message: message.into(),
        }
    }
}

impl From<ToolOutput> for Result<String, String> {
    fn from(output: ToolOutput) -> Self {
        Ok(output.content)
    }
}

impl From<ToolError> for String {
    fn from(err: ToolError) -> Self {
        err.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_output_ok() {
        let output = ToolOutput::ok("hello");
        assert_eq!(output.content, "hello");
        assert!(!output.is_error);
        assert!(output.metadata.is_none());
    }

    #[test]
    fn tool_output_error() {
        let output = ToolOutput::error("oops");
        assert!(output.is_error);
    }

    #[test]
    fn tool_error_display() {
        let err = ToolError::Unsupported { name: "foo".into() };
        assert_eq!(err.to_string(), "unsupported tool: foo");

        let err = ToolError::execution("something broke");
        assert_eq!(err.to_string(), "execution failed: something broke");
    }

    #[test]
    fn tool_error_from_serde() {
        let bad_json: Result<i32, _> = serde_json::from_str("not json");
        let tool_err: ToolError = bad_json.unwrap_err().into();
        assert!(matches!(tool_err, ToolError::InputError(_)));
    }
}
