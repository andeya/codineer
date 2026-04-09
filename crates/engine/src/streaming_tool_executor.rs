//! Streaming tool executor: queues tool calls during model streaming
//! and executes them as soon as complete inputs are received.
//!
//! This allows tool execution to overlap with model output generation,
//! reducing overall latency for multi-tool responses.

use std::collections::VecDeque;

use crate::conversation::ToolExecutor;
use crate::tool_orchestration::{
    execute_batch_with_options, ExecuteBatchOptions, ToolBatch, ToolCall, ToolSlot,
};

/// Status of a queued tool call.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolStatus {
    /// Waiting for execution.
    Pending,
    /// Currently being executed.
    Running,
    /// Execution completed.
    Completed { output: String, is_error: bool },
}

/// A tool call in the streaming execution queue.
#[derive(Debug)]
struct QueuedTool {
    call: ToolCall,
    status: ToolStatus,
}

/// Manages concurrent tool execution during streaming.
///
/// Tool calls are enqueued as they arrive from the model stream.
/// `drain_ready` executes ready tools in batches (parallel when allowed)
/// and returns completed results.
#[derive(Debug)]
pub struct StreamingToolExecutor {
    queue: VecDeque<QueuedTool>,
}

impl StreamingToolExecutor {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    /// Queue a tool call for execution.
    pub fn enqueue(&mut self, call: ToolCall) {
        self.queue.push_back(QueuedTool {
            call,
            status: ToolStatus::Pending,
        });
    }

    /// Number of pending (not yet executed) tool calls.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.queue
            .iter()
            .filter(|t| matches!(t.status, ToolStatus::Pending))
            .count()
    }

    /// Execute all pending tools and return completed results.
    ///
    /// Consecutive concurrency-safe tools are batched for parallel execution
    /// when supported by the executor; otherwise tools run sequentially.
    pub fn drain_ready<T: ToolExecutor>(&mut self, executor: &mut T) -> Vec<CompletedToolCall> {
        self.drain_ready_with_options(executor, ExecuteBatchOptions::default())
    }

    /// Like [`Self::drain_ready`] with optional per-tool progress callbacks.
    pub fn drain_ready_with_options<T: ToolExecutor>(
        &mut self,
        executor: &mut T,
        mut opts: ExecuteBatchOptions<'_>,
    ) -> Vec<CompletedToolCall> {
        let mut results = Vec::new();

        while !self.queue.is_empty() {
            let Some((take_n, batch)) = collect_next_batch(&self.queue, executor) else {
                break;
            };

            let mut calls = Vec::with_capacity(take_n);
            for _ in 0..take_n {
                let q = self.queue.pop_front().expect("batch size matches queue");
                calls.push(q.call);
            }
            debug_assert_eq!(calls.len(), batch.calls.len());

            let batch = ToolBatch {
                calls,
                concurrent: batch.concurrent,
            };

            let exec_results = execute_batch_with_options(&batch, executor, &mut opts);

            for (call, tr) in batch.calls.iter().zip(exec_results) {
                let (output, is_error) = match tr.result {
                    Ok(o) => (o, false),
                    Err(e) => (e.to_string(), true),
                };
                results.push(CompletedToolCall {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    output,
                    is_error,
                });
            }
        }

        results
    }

    /// Check if all tool calls have completed.
    #[must_use]
    pub fn all_completed(&self) -> bool {
        self.queue
            .iter()
            .all(|t| matches!(t.status, ToolStatus::Completed { .. }))
    }

    /// Total number of tool calls (in any status).
    #[must_use]
    pub fn total(&self) -> usize {
        self.queue.len()
    }
}

fn collect_next_batch<T: ToolExecutor>(
    queue: &VecDeque<QueuedTool>,
    executor: &T,
) -> Option<(usize, ToolBatch)> {
    let first = queue.front()?;
    if !matches!(first.status, ToolStatus::Pending) {
        return None;
    }
    match &first.call.slot {
        ToolSlot::Denied { .. } => Some((
            1,
            ToolBatch {
                calls: vec![first.call.clone()],
                concurrent: false,
            },
        )),
        ToolSlot::Sequential => Some((
            1,
            ToolBatch {
                calls: vec![first.call.clone()],
                concurrent: false,
            },
        )),
        ToolSlot::Concurrent => {
            if !executor.is_concurrency_safe(&first.call.name) {
                return Some((
                    1,
                    ToolBatch {
                        calls: vec![first.call.clone()],
                        concurrent: false,
                    },
                ));
            }
            let mut calls = vec![first.call.clone()];
            let mut count = 1usize;
            while count < queue.len() {
                let q = queue.get(count)?;
                if !matches!(q.status, ToolStatus::Pending) {
                    break;
                }
                match &q.call.slot {
                    ToolSlot::Concurrent if executor.is_concurrency_safe(&q.call.name) => {
                        calls.push(q.call.clone());
                        count += 1;
                    }
                    _ => break,
                }
            }
            let concurrent = calls.len() > 1;
            Some((count, ToolBatch { calls, concurrent }))
        }
    }
}

impl Default for StreamingToolExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// A completed tool call result ready to be sent back to the model.
#[derive(Debug, Clone)]
pub struct CompletedToolCall {
    pub id: String,
    pub name: String,
    pub output: String,
    pub is_error: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::ToolError;
    use protocol::events::{EventKind, RuntimeEvent};

    struct EchoExecutor;

    impl ToolExecutor for EchoExecutor {
        fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError> {
            Ok(format!("{tool_name}: {input}"))
        }
    }

    fn make_call(name: &str) -> ToolCall {
        ToolCall {
            id: format!("id-{name}"),
            name: name.to_string(),
            input: "test".to_string(),
            slot: ToolSlot::Sequential,
        }
    }

    #[test]
    fn enqueue_and_drain() {
        let mut ste = StreamingToolExecutor::new();
        let mut executor = EchoExecutor;

        ste.enqueue(make_call("bash"));
        ste.enqueue(make_call("read_file"));

        assert_eq!(ste.pending_count(), 2);
        assert_eq!(ste.total(), 2);

        let results = ste.drain_ready(&mut executor);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "bash");
        assert_eq!(results[0].output, "bash: test");
        assert!(!results[0].is_error);
        assert_eq!(results[1].name, "read_file");
    }

    #[test]
    fn denied_tool_returns_error() {
        let mut ste = StreamingToolExecutor::new();
        let mut executor = EchoExecutor;

        ste.enqueue(ToolCall {
            id: "id-1".into(),
            name: "bash".into(),
            input: "rm -rf /".into(),
            slot: ToolSlot::Denied {
                reason: "blocked".into(),
            },
        });

        let results = ste.drain_ready(&mut executor);
        assert_eq!(results.len(), 1);
        assert!(results[0].is_error);
        assert_eq!(results[0].output, "blocked");
    }

    #[test]
    fn empty_executor_returns_nothing() {
        let mut ste = StreamingToolExecutor::new();
        let mut executor = EchoExecutor;
        let results = ste.drain_ready(&mut executor);
        assert!(results.is_empty());
    }

    #[test]
    fn all_completed_check() {
        let mut ste = StreamingToolExecutor::new();
        assert!(ste.all_completed()); // empty = all completed

        ste.enqueue(make_call("bash"));
        assert!(!ste.all_completed()); // pending

        let mut executor = EchoExecutor;
        ste.drain_ready(&mut executor);
        assert!(ste.all_completed());
    }

    struct ProgressEchoExecutor;

    impl ToolExecutor for ProgressEchoExecutor {
        fn execute(&mut self, tool_name: &str, input: &str) -> Result<String, ToolError> {
            self.execute_with_progress(tool_name, input, None)
        }

        fn execute_with_progress(
            &mut self,
            tool_name: &str,
            input: &str,
            on_progress: Option<&mut dyn FnMut(&str)>,
        ) -> Result<String, ToolError> {
            if let Some(cb) = on_progress {
                cb("working");
            }
            Ok(format!("{tool_name}: {input}"))
        }
    }

    #[test]
    fn drain_ready_emits_tool_progress_via_options() {
        let mut ste = StreamingToolExecutor::new();
        ste.enqueue(make_call("bash"));
        let mut executor = ProgressEchoExecutor;
        let mut kinds = Vec::new();
        let opts = ExecuteBatchOptions {
            on_tool_progress: Some(&mut |id, name, msg| {
                let ev = RuntimeEvent::ToolProgress {
                    tool_use_id: id,
                    tool_name: name,
                    progress: msg,
                };
                kinds.push(ev.kind());
            }),
        };
        let results = ste.drain_ready_with_options(&mut executor, opts);
        assert_eq!(results.len(), 1);
        assert_eq!(kinds, vec![EventKind::ToolProgress]);
    }

    struct ConcurrentSafeBashReadExecutor;

    impl ToolExecutor for ConcurrentSafeBashReadExecutor {
        fn is_concurrency_safe(&self, tool_name: &str) -> bool {
            matches!(tool_name, "bash" | "read_file")
        }

        fn execute_batch(&self, _calls: &[(&str, &str)]) -> Option<Vec<Result<String, ToolError>>> {
            None
        }

        fn execute_batch_with_abort(
            &self,
            _calls: &[(&str, &str)],
            _abort_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
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
    fn sibling_abort_in_streaming_batch() {
        let mut ste = StreamingToolExecutor::new();
        ste.enqueue(ToolCall {
            id: "id-bash".into(),
            name: "bash".into(),
            input: "{}".into(),
            slot: ToolSlot::Concurrent,
        });
        ste.enqueue(ToolCall {
            id: "id-rf".into(),
            name: "read_file".into(),
            input: "{}".into(),
            slot: ToolSlot::Concurrent,
        });
        let mut executor = ConcurrentSafeBashReadExecutor;
        let results = ste.drain_ready(&mut executor);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_error);
        assert!(results[1].is_error);
        assert!(results[1].output.contains("aborted"));
    }
}
