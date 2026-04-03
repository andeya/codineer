use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use super::{
    agent_permission_policy, allowed_tools_for_subagent, execute_agent_with_spawn, execute_tool,
    final_assistant_text, mvp_tool_specs, persist_agent_terminal_state, push_output_block,
    AgentInput, AgentJob, SubagentToolExecutor,
};
use api::OutputContentBlock;
use runtime::{ApiRequest, AssistantEvent, ConversationRuntime, RuntimeError, Session};
use serde_json::json;

include!("suite1.rs");
include!("suite2.rs");
