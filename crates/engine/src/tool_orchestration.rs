//! Tool call orchestration extracted from the conversation loop.
//!
//! Handles tool call classification, batching, and execution order.
//! This module enables the conversation loop to delegate tool execution
//! without embedding the logic directly.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::conversation::{ToolError, ToolExecutor};

/// Classification of how a tool call should be processed.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSlot {
    /// Safe to run concurrently with other Concurrent slots.
    Concurrent,
    /// Must run sequentially (has side effects or ordering deps).
    Sequential,
    /// Tool was denied by permission policy.
    Denied { reason: String },
}

/// Cooperative abort signal for tools in the same batch when `bash` fails.
///
/// Backed by [`Arc`]`<`[`AtomicBool`]`>` (same pattern as [`protocol::cancel::CancelToken`]).
#[derive(Debug, Clone)]
pub struct SiblingAbortController {
    flag: Arc<AtomicBool>,
}

impl SiblingAbortController {
    #[must_use]
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Shared flag for [`ToolExecutor::execute_batch_with_abort`].
    #[must_use]
    pub fn flag(&self) -> &Arc<AtomicBool> {
        &self.flag
    }

    pub fn signal_abort(&self) {
        self.flag.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_aborted(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}

impl Default for SiblingAbortController {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle for incremental tool feedback during batch execution.
///
/// Call [`Self::progress`] with a status line; orchestration forwards it through
/// [`ExecuteBatchOptions::on_tool_progress`] (callers often emit [`protocol::events::RuntimeEvent::ToolProgress`]).
pub struct ToolProgressSink<'a> {
    tool_use_id: &'a str,
    tool_name: &'a str,
    emit: &'a dyn Fn(&str),
}

impl std::fmt::Debug for ToolProgressSink<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolProgressSink")
            .field("tool_use_id", &self.tool_use_id)
            .field("tool_name", &self.tool_name)
            .finish_non_exhaustive()
    }
}

impl ToolProgressSink<'_> {
    pub fn progress(&self, message: &str) {
        (self.emit)(message);
    }
}

/// Callback invoked when a tool emits incremental progress: `(tool_use_id, tool_name, message)`.
pub type OnToolProgress<'a> = dyn FnMut(&str, &str, &str) + 'a;

/// Optional callbacks for [`execute_batch_with_options`].
#[derive(Default)]
pub struct ExecuteBatchOptions<'a> {
    pub on_tool_progress: Option<&'a mut OnToolProgress<'a>>,
}

/// A batch of tool calls that can run together.
#[derive(Debug)]
pub struct ToolBatch {
    pub calls: Vec<ToolCall>,
    pub concurrent: bool,
}

/// A pending tool call with its metadata.
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: String,
    pub slot: ToolSlot,
}

/// Partition tool calls into sequential or concurrent batches.
///
/// Consecutive concurrent calls are grouped into a single batch.
/// Sequential calls get their own single-call batch.
pub fn partition_tool_calls<T: ToolExecutor>(calls: &[ToolCall], executor: &T) -> Vec<ToolBatch> {
    let mut batches = Vec::new();
    let mut concurrent_group = Vec::new();

    for call in calls {
        match &call.slot {
            ToolSlot::Denied { .. } => {
                flush_concurrent(&mut concurrent_group, &mut batches);
                batches.push(ToolBatch {
                    calls: vec![call.clone()],
                    concurrent: false,
                });
            }
            ToolSlot::Sequential => {
                flush_concurrent(&mut concurrent_group, &mut batches);
                batches.push(ToolBatch {
                    calls: vec![call.clone()],
                    concurrent: false,
                });
            }
            ToolSlot::Concurrent => {
                if executor.is_concurrency_safe(&call.name) {
                    concurrent_group.push(call.clone());
                } else {
                    flush_concurrent(&mut concurrent_group, &mut batches);
                    batches.push(ToolBatch {
                        calls: vec![call.clone()],
                        concurrent: false,
                    });
                }
            }
        }
    }

    flush_concurrent(&mut concurrent_group, &mut batches);
    batches
}

fn flush_concurrent(group: &mut Vec<ToolCall>, batches: &mut Vec<ToolBatch>) {
    if group.is_empty() {
        return;
    }
    let calls = std::mem::take(group);
    let concurrent = calls.len() > 1;
    batches.push(ToolBatch { calls, concurrent });
}

/// Execute a single batch, returning results in the same order.
pub fn execute_batch<T: ToolExecutor>(batch: &ToolBatch, executor: &mut T) -> Vec<ToolCallResult> {
    let mut opts = ExecuteBatchOptions::default();
    execute_batch_with_options(batch, executor, &mut opts)
}

/// Execute a batch with optional progress callbacks and sibling abort when `bash` fails.
pub fn execute_batch_with_options<T: ToolExecutor>(
    batch: &ToolBatch,
    executor: &mut T,
    opts: &mut ExecuteBatchOptions<'_>,
) -> Vec<ToolCallResult> {
    let sibling_abort = SiblingAbortController::new();
    let calls_ref: Vec<(&str, &str)> = batch
        .calls
        .iter()
        .map(|c| (c.name.as_str(), c.input.as_str()))
        .collect();

    let try_parallel = batch.concurrent && batch.calls.len() > 1;
    if try_parallel {
        if let Some(batch_results) =
            executor.execute_batch_with_abort(&calls_ref, sibling_abort.flag())
        {
            return zip_batch_results(batch, batch_results, &sibling_abort);
        }
        if let Some(batch_results) = executor.execute_batch(&calls_ref) {
            return zip_batch_results(batch, batch_results, &sibling_abort);
        }
    }

    let mut out = Vec::with_capacity(batch.calls.len());
    for call in &batch.calls {
        if sibling_abort.is_aborted() {
            out.push(ToolCallResult {
                id: call.id.clone(),
                name: call.name.clone(),
                result: Err(ToolError::new("tool aborted: sibling bash tool failed")),
            });
            continue;
        }

        let result = match &call.slot {
            ToolSlot::Denied { reason } => Err(ToolError::new(reason.clone())),
            _ => run_with_progress(executor, call, &mut opts.on_tool_progress),
        };

        if call.name == "bash" && result.is_err() {
            sibling_abort.signal_abort();
        }

        out.push(ToolCallResult {
            id: call.id.clone(),
            name: call.name.clone(),
            result,
        });
    }
    out
}

fn zip_batch_results(
    batch: &ToolBatch,
    batch_results: Vec<Result<String, ToolError>>,
    sibling_abort: &SiblingAbortController,
) -> Vec<ToolCallResult> {
    let mut out = Vec::with_capacity(batch.calls.len());
    for (call, result) in batch.calls.iter().zip(batch_results) {
        if call.name == "bash" && result.is_err() {
            sibling_abort.signal_abort();
        }
        out.push(ToolCallResult {
            id: call.id.clone(),
            name: call.name.clone(),
            result,
        });
    }
    out
}

fn run_with_progress<T: ToolExecutor>(
    executor: &mut T,
    call: &ToolCall,
    on_progress: &mut Option<&mut OnToolProgress<'_>>,
) -> Result<String, ToolError> {
    if let Some(cb) = on_progress.as_mut() {
        let id = call.id.as_str();
        let name = call.name.as_str();
        let mut inner = |msg: &str| {
            (**cb)(id, name, msg);
        };
        executor.execute_with_progress(&call.name, &call.input, Some(&mut inner))
    } else {
        executor.execute(&call.name, &call.input)
    }
}

/// Result of a single tool call execution.
#[derive(Debug)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub result: Result<String, ToolError>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::events::{EventKind, RuntimeEvent};

    struct MockExecutor {
        safe_tools: Vec<String>,
    }

    impl ToolExecutor for MockExecutor {
        fn execute(&mut self, tool_name: &str, _input: &str) -> Result<String, ToolError> {
            Ok(format!("executed {tool_name}"))
        }

        fn is_concurrency_safe(&self, tool_name: &str) -> bool {
            self.safe_tools.iter().any(|t| t == tool_name)
        }
    }

    fn call(name: &str, slot: ToolSlot) -> ToolCall {
        ToolCall {
            id: format!("id-{name}"),
            name: name.to_string(),
            input: "{}".to_string(),
            slot,
        }
    }

    #[test]
    fn sequential_calls_get_own_batches() {
        let executor = MockExecutor { safe_tools: vec![] };
        let calls = vec![
            call("bash", ToolSlot::Sequential),
            call("write_file", ToolSlot::Sequential),
        ];
        let batches = partition_tool_calls(&calls, &executor);
        assert_eq!(batches.len(), 2);
        assert!(!batches[0].concurrent);
        assert!(!batches[1].concurrent);
    }

    #[test]
    fn concurrent_calls_grouped() {
        let executor = MockExecutor {
            safe_tools: vec!["read_file".into(), "grep_search".into()],
        };
        let calls = vec![
            call("read_file", ToolSlot::Concurrent),
            call("grep_search", ToolSlot::Concurrent),
        ];
        let batches = partition_tool_calls(&calls, &executor);
        assert_eq!(batches.len(), 1);
        assert!(batches[0].concurrent);
        assert_eq!(batches[0].calls.len(), 2);
    }

    #[test]
    fn mixed_calls_partition_correctly() {
        let executor = MockExecutor {
            safe_tools: vec!["read_file".into(), "glob_search".into()],
        };
        let calls = vec![
            call("read_file", ToolSlot::Concurrent),
            call("glob_search", ToolSlot::Concurrent),
            call("bash", ToolSlot::Sequential),
            call("read_file", ToolSlot::Concurrent),
        ];
        let batches = partition_tool_calls(&calls, &executor);
        assert_eq!(batches.len(), 3);
        assert!(batches[0].concurrent); // read_file + glob_search
        assert!(!batches[1].concurrent); // bash
        assert!(!batches[2].concurrent); // single read_file
    }

    #[test]
    fn denied_calls_isolated() {
        let executor = MockExecutor { safe_tools: vec![] };
        let calls = vec![
            call(
                "bash",
                ToolSlot::Denied {
                    reason: "blocked".into(),
                },
            ),
            call("read_file", ToolSlot::Sequential),
        ];
        let batches = partition_tool_calls(&calls, &executor);
        assert_eq!(batches.len(), 2);
    }

    #[test]
    fn execute_batch_sequential() {
        let mut executor = MockExecutor { safe_tools: vec![] };
        let batch = ToolBatch {
            calls: vec![call("bash", ToolSlot::Sequential)],
            concurrent: false,
        };
        let results = execute_batch(&batch, &mut executor);
        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_ok());
        assert_eq!(results[0].result.as_ref().unwrap(), "executed bash");
    }

    #[test]
    fn execute_denied_returns_error() {
        let mut executor = MockExecutor { safe_tools: vec![] };
        let batch = ToolBatch {
            calls: vec![call(
                "bash",
                ToolSlot::Denied {
                    reason: "no".into(),
                },
            )],
            concurrent: false,
        };
        let results = execute_batch(&batch, &mut executor);
        assert!(results[0].result.is_err());
    }

    struct ProgressReportingExecutor;

    impl ToolExecutor for ProgressReportingExecutor {
        fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError> {
            self.execute_with_progress(tool_name, input, None)
        }

        fn execute_with_progress(
            &mut self,
            tool_name: &str,
            _input: &str,
            on_progress: Option<&mut dyn FnMut(&str)>,
        ) -> Result<String, ToolError> {
            if let Some(cb) = on_progress {
                cb("p1");
                cb("p2");
            }
            Ok(format!("done-{tool_name}"))
        }
    }

    #[test]
    fn tool_progress_invokes_callback_and_matches_runtime_event() {
        let mut executor = ProgressReportingExecutor;
        let batch = ToolBatch {
            calls: vec![call("read_file", ToolSlot::Sequential)],
            concurrent: false,
        };
        let mut recorded: Vec<(String, String, String)> = Vec::new();
        let mut opts = ExecuteBatchOptions {
            on_tool_progress: Some(&mut |id, name, msg| {
                recorded.push((id.to_string(), name.to_string(), msg.to_string()));
            }),
        };
        let results = execute_batch_with_options(&batch, &mut executor, &mut opts);
        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_ok());
        assert_eq!(recorded.len(), 2);
        let ev = RuntimeEvent::ToolProgress {
            tool_use_id: recorded[0].0.as_str(),
            tool_name: recorded[0].1.as_str(),
            progress: recorded[0].2.as_str(),
        };
        assert_eq!(ev.kind(), EventKind::ToolProgress);
        assert_eq!(recorded[0].0, "id-read_file");
        assert_eq!(recorded[0].1, "read_file");
        assert_eq!(recorded[0].2, "p1");
        assert_eq!(recorded[1].2, "p2");
    }

    struct BashThenReadExecutor;

    impl ToolExecutor for BashThenReadExecutor {
        fn execute_batch(&self, _calls: &[(&str, &str)]) -> Option<Vec<Result<String, ToolError>>> {
            None
        }

        fn execute_batch_with_abort(
            &self,
            _calls: &[(&str, &str)],
            _abort_flag: &Arc<AtomicBool>,
        ) -> Option<Vec<Result<String, ToolError>>> {
            None
        }

        fn execute(&mut self, tool_name: &str, _input: &str) -> Result<String, ToolError> {
            if tool_name == "bash" {
                Err(ToolError::new("bash failed"))
            } else {
                Ok("ok".into())
            }
        }
    }

    #[test]
    fn sibling_abort_cancels_pending_after_bash_failure() {
        let mut executor = BashThenReadExecutor;
        let batch = ToolBatch {
            calls: vec![
                call("bash", ToolSlot::Concurrent),
                call("read_file", ToolSlot::Concurrent),
            ],
            concurrent: true,
        };
        let results = execute_batch(&batch, &mut executor);
        assert_eq!(results.len(), 2);
        assert!(results[0].result.is_err());
        assert!(results[1].result.is_err());
        assert!(
            results[1]
                .result
                .as_ref()
                .err()
                .unwrap()
                .message()
                .contains("aborted"),
            "{:?}",
            results[1].result
        );
    }

    struct ReadFailsThenBashOkExecutor;

    impl ToolExecutor for ReadFailsThenBashOkExecutor {
        fn execute_batch(&self, _calls: &[(&str, &str)]) -> Option<Vec<Result<String, ToolError>>> {
            None
        }

        fn execute_batch_with_abort(
            &self,
            _calls: &[(&str, &str)],
            _abort_flag: &Arc<AtomicBool>,
        ) -> Option<Vec<Result<String, ToolError>>> {
            None
        }

        fn execute(&mut self, tool_name: &str, _input: &str) -> Result<String, ToolError> {
            match tool_name {
                "read_file" => Err(ToolError::new("read failed")),
                "bash" => Ok("shell ok".into()),
                _ => Ok("ok".into()),
            }
        }
    }

    #[test]
    fn non_bash_failure_does_not_abort_siblings() {
        let mut executor = ReadFailsThenBashOkExecutor;
        let batch = ToolBatch {
            calls: vec![
                call("read_file", ToolSlot::Concurrent),
                call("bash", ToolSlot::Concurrent),
            ],
            concurrent: true,
        };
        let results = execute_batch(&batch, &mut executor);
        assert_eq!(results.len(), 2);
        assert!(results[0].result.is_err());
        assert!(results[1].result.is_ok());
        assert_eq!(results[1].result.as_ref().unwrap(), "shell ok");
    }
}
