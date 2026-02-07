use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use lsp_types::{
    Diagnostic, GotoDefinitionResponse, Location, LocationLink, Position, PublishDiagnosticsParams,
};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{oneshot, Mutex};

use crate::error::LspError;
use crate::types::{LspServerConfig, SymbolLocation};

type PendingMap = BTreeMap<i64, oneshot::Sender<Result<Value, LspError>>>;

pub(crate) struct LspClient {
    config: LspServerConfig,
    writer: Mutex<BufWriter<ChildStdin>>,
    child: Mutex<Child>,
    pending_requests: Arc<Mutex<PendingMap>>,
    diagnostics: Arc<Mutex<BTreeMap<String, Vec<Diagnostic>>>>,
    open_documents: Mutex<BTreeMap<PathBuf, i32>>,
    next_request_id: AtomicI64,
}

impl LspClient {
    pub(crate) async fn connect(config: LspServerConfig) -> Result<Self, LspError> {
        let mut command = Command::new(&config.command);
        command
            .args(&config.args)
            .current_dir(&config.workspace_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(config.env.clone());

        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LspError::Protocol("missing LSP stdin pipe".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LspError::Protocol("missing LSP stdout pipe".to_string()))?;
        let stderr = child.stderr.take();

        let client = Self {
            config,
