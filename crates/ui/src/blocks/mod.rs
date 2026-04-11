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
    #[serde(with = "optional_duration_millis")]
    pub duration: Option<Duration>,
    pub ai_diagnosis: Option<BlockId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIBlock {
    pub meta: BlockMeta,
    pub model: String,
    pub role: Role,
    pub content: String,
    pub streaming: bool,
    pub token_count: Option<u32>,
    pub context_refs: Vec<ContextRef>,
    pub executable_snippets: Vec<CodeSnippet>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSnippet {
    pub language: String,
    pub code: String,
    pub executed: bool,
    pub result_block_id: Option<BlockId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextRef {
    Block(BlockId),
    File(PathBuf),
    GitDiff(String),
    Url(String),
    Memory(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolBlock {
    pub meta: BlockMeta,
    pub tool_use_id: String,
    pub name: String,
    pub input: String,
    pub state: ToolState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolState {
    Pending,
    Running,
    Completed { output: String, is_error: bool },
    Denied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlanBlock {
    pub meta: BlockMeta,
    pub goal: String,
    pub steps: Vec<AgentStep>,
    pub state: AgentPlanState,
    pub approval_policy: ApprovalPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub index: usize,
    pub description: String,
    pub state: StepState,
    pub child_block_id: Option<BlockId>,
    #[serde(with = "optional_duration_millis")]
    pub duration: Option<Duration>,
    pub is_dangerous: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentPlanState {
    Planning,
    Executing,
    AwaitApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepState {
    Pending,
    Running,
    Completed,
    Failed,
    NeedsApproval,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalPolicy {
    AlwaysApprove,
    DangerousOnly,
    AlwaysAsk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBlock {
    pub meta: BlockMeta,
    pub kind: SystemKind,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemKind {
    DirChange,
    Error,
    Info,
    Welcome,
    ProactiveHint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffBlock {
    pub meta: BlockMeta,
    pub file_path: String,
    pub hunks: Vec<DiffHunk>,
    pub stats: DiffStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffLine {
    Context(String),
    Addition(String),
    Deletion(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStats {
    pub additions: u32,
    pub deletions: u32,
    pub files_changed: u32,
}

mod optional_duration_millis {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(value: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(d) => (d.as_millis() as u64).serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<u64> = Option::deserialize(deserializer)?;
        Ok(opt.map(Duration::from_millis))
    }
}
