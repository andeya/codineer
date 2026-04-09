//! Swarm orchestration types for multi-agent coordination.
//!
//! Enables parallel execution of sub-agents with centralized work
//! distribution and result aggregation. Designed for tasks that can
//! be decomposed into independent sub-tasks.

use std::collections::BTreeMap;

/// Describes a unit of work for a swarm agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmTask {
    pub id: String,
    pub description: String,
    pub context: BTreeMap<String, String>,
    pub priority: SwarmPriority,
}

/// Task priority levels for scheduling.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum SwarmPriority {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

/// Result from a completed swarm task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmTaskResult {
    pub task_id: String,
    pub status: SwarmTaskStatus,
    pub output: String,
    pub artifacts: Vec<SwarmArtifact>,
}

/// Completion status for a swarm task.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwarmTaskStatus {
    Success,
    Failed,
    Cancelled,
    TimedOut,
}

/// An artifact produced by a swarm agent (e.g., a file diff, test result).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmArtifact {
    pub kind: String,
    pub path: Option<String>,
    pub content: String,
}

/// Configuration for swarm orchestration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmConfig {
    pub max_agents: usize,
    pub timeout_secs: u64,
    pub model: String,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            max_agents: 4,
            timeout_secs: 300,
            model: String::new(),
        }
    }
}

/// Trait for swarm orchestrators.
///
/// Manages the lifecycle of parallel agent execution,
/// from task distribution through result collection.
pub trait SwarmOrchestrator: Send {
    fn submit(&mut self, task: SwarmTask);
    fn pending_count(&self) -> usize;
    fn collect_results(&mut self) -> Vec<SwarmTaskResult>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swarm_priority_ordering() {
        assert!(SwarmPriority::Low < SwarmPriority::Normal);
        assert!(SwarmPriority::Normal < SwarmPriority::High);
        assert!(SwarmPriority::High < SwarmPriority::Critical);
    }

    #[test]
    fn swarm_config_defaults() {
        let config = SwarmConfig::default();
        assert_eq!(config.max_agents, 4);
        assert_eq!(config.timeout_secs, 300);
    }

    #[test]
    fn swarm_task_construction() {
        let task = SwarmTask {
            id: "task-1".to_string(),
            description: "Run tests".to_string(),
            context: BTreeMap::new(),
            priority: SwarmPriority::High,
        };
        assert_eq!(task.id, "task-1");
        assert_eq!(task.priority, SwarmPriority::High);
    }
}
