use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use lsp_types::Position;
use tokio::sync::Mutex;

use crate::client::LspClient;
use crate::error::LspError;
use crate::types::{
    normalize_extension, FileDiagnostics, LspContextEnrichment, LspServerConfig, SymbolLocation,
    WorkspaceDiagnostics,
};

pub struct LspManager {
    server_configs: BTreeMap<String, LspServerConfig>,
    extension_map: BTreeMap<String, String>,
    clients: Mutex<BTreeMap<String, Arc<LspClient>>>,
}

impl LspManager {
    pub fn new(server_configs: Vec<LspServerConfig>) -> Result<Self, LspError> {
        let mut configs_by_name = BTreeMap::new();
        let mut extension_map = BTreeMap::new();

        for config in server_configs {
            for extension in config.extension_to_language.keys() {
                let normalized = normalize_extension(extension);
                if let Some(existing_server) =
                    extension_map.insert(normalized.clone(), config.name.clone())
                {
                    return Err(LspError::DuplicateExtension {
                        extension: normalized,
                        existing_server,
                        new_server: config.name.clone(),
                    });
                }
            }
            configs_by_name.insert(config.name.clone(), config);
        }

        Ok(Self {
