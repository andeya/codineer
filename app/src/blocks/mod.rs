use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

pub type BlockId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Block {
    Command(CommandBlock),
    AI(AIBlock),
    Tool(ToolBlock),
    System(SystemBlock),
    Diff(DiffBlock),
    AgentPlan(AgentPlanBlock),
}

impl Block {
    pub fn meta(&self) -> &BlockMeta {
        match self {
            Block::Command(b) => &b.meta,
            Block::AI(b) => &b.meta,
            Block::Tool(b) => &b.meta,
            Block::System(b) => &b.meta,
            Block::Diff(b) => &b.meta,
            Block::AgentPlan(b) => &b.meta,
        }
    }

    pub fn id(&self) -> BlockId {
        self.meta().id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockMeta {
    pub id: BlockId,
    pub created_at: DateTime<Utc>,
    pub collapsed: bool,
    pub parent_id: Option<BlockId>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandBlock {
    pub meta: BlockMeta,
    pub command: String,
    pub cwd: PathBuf,
    pub output_text: String,
    pub exit_code: Option<i32>,
    pub duration: Option<Duration>,
    pub ai_diagnosis: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIBlock {
    pub meta: BlockMeta,
    pub role: Role,
    pub content: String,
    pub model: String,
    pub streaming: bool,
    pub token_count: Option<u32>,
    pub executable_snippets: Vec<CodeSnippet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSnippet {
    pub language: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolState {
    Pending,
    Running,
    Completed { output: String, is_error: bool },
    Denied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolBlock {
    pub meta: BlockMeta,
    pub name: String,
    pub input: String,
    pub state: ToolState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemKind {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBlock {
    pub meta: BlockMeta,
    pub kind: SystemKind,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffBlock {
    pub meta: BlockMeta,
    pub file_path: String,
    pub hunks: Vec<DiffHunk>,
    pub status: DiffStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffLineKind {
    Context,
    Add,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffStatus {
    Pending,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentPlanState {
    Planning,
    Executing,
    AwaitApproval,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlanStep {
    pub description: String,
    pub status: AgentStepStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlanBlock {
    pub meta: BlockMeta,
    pub goal: String,
    pub steps: Vec<AgentPlanStep>,
    pub state: AgentPlanState,
}
