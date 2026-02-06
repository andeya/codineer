use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use lsp_types::{Diagnostic, Range};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub workspace_root: PathBuf,
    pub initialization_options: Option<Value>,
    pub extension_to_language: BTreeMap<String, String>,
}

impl LspServerConfig {
    #[must_use]
    pub fn language_id_for(&self, path: &Path) -> Option<&str> {
        let extension = normalize_extension(path.extension()?.to_string_lossy().as_ref());
        self.extension_to_language
            .get(&extension)
            .map(String::as_str)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileDiagnostics {
    pub path: PathBuf,
    pub uri: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorkspaceDiagnostics {
    pub files: Vec<FileDiagnostics>,
}

impl WorkspaceDiagnostics {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    #[must_use]
    pub fn total_diagnostics(&self) -> usize {
        self.files.iter().map(|file| file.diagnostics.len()).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolLocation {
    pub path: PathBuf,
