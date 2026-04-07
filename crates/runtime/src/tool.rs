use async_trait::async_trait;
use serde_json::Value;

use crate::conversation::ToolError;
use crate::permissions::PermissionMode;

/// Structured result from a tool execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub output: String,
    pub is_error: bool,
    pub diagnostics: Option<String>,
}

impl ToolResult {
    #[must_use]
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            is_error: false,
            diagnostics: None,
        }
    }

    #[must_use]
    pub fn error(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            is_error: true,
            diagnostics: None,
        }
    }

    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: impl Into<String>) -> Self {
        self.diagnostics = Some(diagnostics.into());
        self
    }
}

/// Trait for implementing AI-callable tools with lifecycle methods.
///
/// Each tool has metadata (name, description, schema, permissions) and an async
/// `execute` method. Tools are registered in a `ToolRegistry` and dispatched by
/// the conversation runtime.
///
/// The `execute` method is async to support I/O-bound operations (file system,
/// network, LSP, etc.) and to enable concurrent execution of independent tools.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn required_permission(&self) -> PermissionMode;

    /// Whether this tool only reads state (no side effects).
    /// Defaults to `true`. Override to `false` for tools that modify files,
    /// execute commands, etc.
    fn is_read_only(&self) -> bool {
        true
    }

    /// Whether multiple instances of this tool can safely execute in parallel.
    /// Defaults to `true`. Override to `false` for tools that have ordering
    /// dependencies or exclusive resource access (e.g., bash, edit_file).
    fn is_concurrency_safe(&self) -> bool {
        true
    }

    /// Whether this tool is currently enabled. Disabled tools are hidden from
    /// the model and rejected at execution time.
    fn is_enabled(&self) -> bool {
        true
    }

    /// Validate tool input before execution. Return `Err` with
    /// `ToolErrorCode::InvalidInput` for malformed inputs.
    fn validate_input(&self, input: &Value) -> Result<(), ToolError>;

    /// Execute the tool with the given input. Called after permission checks
    /// and input validation.
    async fn execute(&self, input: Value) -> Result<ToolResult, ToolError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::ToolErrorCode;
    use serde_json::json;

    #[test]
    fn tool_result_success_has_correct_defaults() {
        let result = ToolResult::success("hello");
        assert_eq!(result.output, "hello");
        assert!(!result.is_error);
        assert!(result.diagnostics.is_none());
    }

    #[test]
    fn tool_result_error_sets_is_error() {
        let result = ToolResult::error("boom");
        assert!(result.is_error);
    }

    #[test]
    fn tool_result_with_diagnostics_chains() {
        let result = ToolResult::success("ok").with_diagnostics("warning: unused variable");
        assert_eq!(
            result.diagnostics.as_deref(),
            Some("warning: unused variable")
        );
    }

    #[test]
    fn tool_error_new_defaults_to_internal() {
        let err = ToolError::new("something broke");
        assert_eq!(err.code(), ToolErrorCode::InternalError);
        assert_eq!(err.message(), "something broke");
    }

    #[test]
    fn tool_error_with_code_preserves_code() {
        let err = ToolError::with_code(ToolErrorCode::NotFound, "file missing");
        assert_eq!(err.code(), ToolErrorCode::NotFound);
        assert_eq!(err.to_string(), "file missing");
    }

    struct DummyTool;

    #[async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &str {
            "dummy"
        }
        fn description(&self) -> &str {
            "A test tool"
        }
        fn input_schema(&self) -> Value {
            json!({"type": "object"})
        }
        fn required_permission(&self) -> PermissionMode {
            PermissionMode::ReadOnly
        }
        fn validate_input(&self, _input: &Value) -> Result<(), ToolError> {
            Ok(())
        }
        async fn execute(&self, _input: Value) -> Result<ToolResult, ToolError> {
            Ok(ToolResult::success("done"))
        }
    }

    #[tokio::test]
    async fn dummy_tool_executes() {
        let tool: Box<dyn Tool> = Box::new(DummyTool);
        assert_eq!(tool.name(), "dummy");
        assert!(tool.is_read_only());
        assert!(tool.is_concurrency_safe());
        assert!(tool.is_enabled());
        let result = tool.execute(json!({})).await.unwrap();
        assert_eq!(result.output, "done");
        assert!(!result.is_error);
    }
}
