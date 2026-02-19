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
            writer: Mutex::new(BufWriter::new(stdin)),
            child: Mutex::new(child),
            pending_requests: Arc::new(Mutex::new(BTreeMap::new())),
            diagnostics: Arc::new(Mutex::new(BTreeMap::new())),
            open_documents: Mutex::new(BTreeMap::new()),
            next_request_id: AtomicI64::new(1),
        };

        client.spawn_reader(stdout);
        if let Some(stderr) = stderr {
            Self::spawn_stderr_drain(stderr);
        }
        if let Err(err) = client.initialize().await {
            let _ = client.child.lock().await.kill().await;
            return Err(err);
        }
        Ok(client)
    }

    pub(crate) async fn ensure_document_open(&self, path: &Path) -> Result<(), LspError> {
        if self.is_document_open(path).await {
            return Ok(());
        }

        let contents = std::fs::read_to_string(path)?;
        self.open_document(path, &contents).await
    }

    pub(crate) async fn open_document(&self, path: &Path, text: &str) -> Result<(), LspError> {
        let uri = file_url(path)?;
        let language_id = self
            .config
            .language_id_for(path)
            .ok_or_else(|| LspError::UnsupportedDocument(path.to_path_buf()))?;

        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text,
                }
            }),
        )
        .await?;

        self.open_documents
            .lock()
            .await
            .insert(path.to_path_buf(), 1);
        Ok(())
    }

    pub(crate) async fn change_document(&self, path: &Path, text: &str) -> Result<(), LspError> {
        if !self.is_document_open(path).await {
            return self.open_document(path, text).await;
        }

        let uri = file_url(path)?;
        let next_version = {
            let mut open_documents = self.open_documents.lock().await;
            let version = open_documents
                .entry(path.to_path_buf())
                .and_modify(|value| *value += 1)
                .or_insert(1);
            *version
        };

        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {
                    "uri": uri,
                    "version": next_version,
                },
                "contentChanges": [{
                    "text": text,
                }],
            }),
        )
        .await
    }

    pub(crate) async fn save_document(&self, path: &Path) -> Result<(), LspError> {
        if !self.is_document_open(path).await {
            return Ok(());
        }

        self.notify(
            "textDocument/didSave",
            json!({
                "textDocument": {
                    "uri": file_url(path)?,
                }
            }),
        )
        .await
    }

    pub(crate) async fn close_document(&self, path: &Path) -> Result<(), LspError> {
        if !self.is_document_open(path).await {
            return Ok(());
        }

        self.notify(
            "textDocument/didClose",
            json!({
                "textDocument": {
                    "uri": file_url(path)?,
                }
            }),
        )
        .await?;

        self.open_documents.lock().await.remove(path);
        Ok(())
    }

    pub(crate) async fn is_document_open(&self, path: &Path) -> bool {
        self.open_documents.lock().await.contains_key(path)
    }

    pub(crate) async fn go_to_definition(
        &self,
        path: &Path,
        position: Position,
    ) -> Result<Vec<SymbolLocation>, LspError> {
        self.ensure_document_open(path).await?;
        let response = self
            .request::<Option<GotoDefinitionResponse>>(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": file_url(path)? },
                    "position": position,
                }),
            )
            .await?;

        Ok(match response {
            Some(GotoDefinitionResponse::Scalar(location)) => {
                location_to_symbol_locations(vec![location])
            }
            Some(GotoDefinitionResponse::Array(locations)) => {
                location_to_symbol_locations(locations)
            }
            Some(GotoDefinitionResponse::Link(links)) => location_links_to_symbol_locations(links),
            None => Vec::new(),
        })
    }

    pub(crate) async fn find_references(
        &self,
        path: &Path,
        position: Position,
        include_declaration: bool,
    ) -> Result<Vec<SymbolLocation>, LspError> {
        self.ensure_document_open(path).await?;
        let response = self
            .request::<Option<Vec<Location>>>(
                "textDocument/references",
                json!({
                    "textDocument": { "uri": file_url(path)? },
                    "position": position,
                    "context": {
                        "includeDeclaration": include_declaration,
                    },
                }),
            )
            .await?;

        Ok(location_to_symbol_locations(response.unwrap_or_default()))
    }

    pub(crate) async fn diagnostics_snapshot(&self) -> BTreeMap<String, Vec<Diagnostic>> {
        self.diagnostics.lock().await.clone()
    }

    pub(crate) async fn shutdown(&self) -> Result<(), LspError> {
        let _ = self.request::<Value>("shutdown", json!({})).await;
        let _ = self.notify("exit", Value::Null).await;

        let mut child = self.child.lock().await;
        if child.kill().await.is_err() {
            let _ = child.wait().await;
            return Ok(());
        }
        let _ = child.wait().await;
        Ok(())
    }

    fn spawn_reader(&self, stdout: ChildStdout) {
        let diagnostics = &self.diagnostics;
        let pending_requests = &self.pending_requests;

        let diagnostics = diagnostics.clone();
        let pending_requests = pending_requests.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let result = async {
                while let Some(message) = read_message(&mut reader).await? {
                    if let Some(id) = message.get("id").and_then(Value::as_i64) {
                        let response = if let Some(error) = message.get("error") {
                            Err(LspError::Protocol(error.to_string()))
                        } else {
                            Ok(message.get("result").cloned().unwrap_or(Value::Null))
                        };
