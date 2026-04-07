//! Tool call orchestration extracted from the conversation loop.
//!
//! Handles tool call classification, batching, and execution order.
//! This module enables the conversation loop to delegate tool execution
//! without embedding the logic directly.

use crate::conversation::{ToolError, ToolExecutor};

/// Classification of how a tool call should be processed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSlot {
    /// Safe to run concurrently with other Concurrent slots.
    Concurrent,
    /// Must run sequentially (has side effects or ordering deps).
    Sequential,
    /// Tool was denied by permission policy.
    Denied { reason: String },
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
pub fn partition_tool_calls<T: ToolExecutor>(
    calls: &[ToolCall],
    executor: &T,
) -> Vec<ToolBatch> {
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
    batches.push(ToolBatch {
        calls,
        concurrent,
    });
}

/// Execute a single batch, returning results in the same order.
pub fn execute_batch<T: ToolExecutor>(
    batch: &ToolBatch,
    executor: &mut T,
) -> Vec<ToolCallResult> {
    if batch.concurrent && batch.calls.len() > 1 {
        if let Some(batch_results) = executor.execute_batch(
            &batch
                .calls
                .iter()
                .map(|c| (c.name.as_str(), c.input.as_str()))
                .collect::<Vec<_>>(),
        ) {
            return batch
                .calls
                .iter()
                .zip(batch_results)
                .map(|(call, result)| ToolCallResult {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    result,
                })
                .collect();
        }
    }

    batch
        .calls
        .iter()
        .map(|call| {
            let result = match &call.slot {
                ToolSlot::Denied { reason } => Err(ToolError::new(reason)),
                _ => executor.execute(&call.name, &call.input),
            };
            ToolCallResult {
                id: call.id.clone(),
                name: call.name.clone(),
                result,
            }
        })
        .collect()
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
            call("bash", ToolSlot::Denied { reason: "blocked".into() }),
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
            calls: vec![call("bash", ToolSlot::Denied { reason: "no".into() })],
            concurrent: false,
        };
        let results = execute_batch(&batch, &mut executor);
        assert!(results[0].result.is_err());
    }
}
