use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use plugins::{PluginError, PluginManager, PluginSummary};
use runtime::{compact_session, CompactionConfig, Session};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandManifestEntry {
    pub name: String,
    pub source: CommandSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSource {
    Builtin,
    InternalOnly,
    FeatureGated,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRegistry {
    entries: Vec<CommandManifestEntry>,
}

impl CommandRegistry {
    #[must_use]
    pub fn new(entries: Vec<CommandManifestEntry>) -> Self {
        Self { entries }
    }

    #[must_use]
    pub fn entries(&self) -> &[CommandManifestEntry] {
        &self.entries
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlashCommandCategory {
    Core,
    Workspace,
    Session,
    Git,
    Automation,
}

impl SlashCommandCategory {
    const fn title(self) -> &'static str {
        match self {
            Self::Core => "Core flow",
            Self::Workspace => "Workspace & memory",
            Self::Session => "Sessions & output",
            Self::Git => "Git & GitHub",
            Self::Automation => "Automation & discovery",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashCommandSpec {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub summary: &'static str,
    pub argument_hint: Option<&'static str>,
    pub resume_supported: bool,
    pub category: SlashCommandCategory,
}

