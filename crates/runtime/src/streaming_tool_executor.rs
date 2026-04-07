//! Streaming tool executor: queues tool calls during model streaming
//! and executes them as soon as complete inputs are received.
//!
//! This allows tool execution to overlap with model output generation,
//! reducing overall latency for multi-tool responses.

use std::collections::VecDeque;

use crate::conversation::{ToolError, ToolExecutor};
use crate::tool_orchestration::{ToolCall, ToolSlot};

/// Status of a queued tool call.
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
/// `drain_ready` executes all ready tools and returns completed results.
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
    /// Sequential tools are executed in order. Concurrent-safe tools
    /// are batched together for parallel execution when supported.
    pub fn drain_ready<T: ToolExecutor>(
        &mut self,
        executor: &mut T,
    ) -> Vec<CompletedToolCall> {
        let mut results = Vec::new();

        while let Some(queued) = self.queue.front_mut() {
            if !matches!(queued.status, ToolStatus::Pending) {
                let completed = self.queue.pop_front().unwrap();
                if let ToolStatus::Completed { output, is_error } = completed.status {
                    results.push(CompletedToolCall {
                        id: completed.call.id,
                        name: completed.call.name,
                        output,
                        is_error,
                    });
                }
                continue;
            }

            match &queued.call.slot {
                ToolSlot::Denied { reason } => {
                    queued.status = ToolStatus::Completed {
                        output: reason.clone(),
                        is_error: true,
                    };
                }
                ToolSlot::Sequential | ToolSlot::Concurrent => {
                    let result = executor.execute(&queued.call.name, &queued.call.input);
                    match result {
                        Ok(output) => {
                            queued.status = ToolStatus::Completed {
                                output,
                                is_error: false,
                            };
                        }
                        Err(err) => {
                            queued.status = ToolStatus::Completed {
                                output: err.message().to_string(),
                                is_error: true,
                            };
                        }
                    }
                }
            }
        }

        // Drain remaining completed
        while let Some(queued) = self.queue.front() {
            if let ToolStatus::Completed { .. } = &queued.status {
                let completed = self.queue.pop_front().unwrap();
                if let ToolStatus::Completed { output, is_error } = completed.status {
                    results.push(CompletedToolCall {
                        id: completed.call.id,
                        name: completed.call.name,
                        output,
                        is_error,
                    });
                }
            } else {
                break;
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
            slot: ToolSlot::Denied { reason: "blocked".into() },
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
}
