mod hooks;

mod constants;
mod definition;
mod error;
mod install;
mod lifecycle;
mod manager;
mod manifest;
mod resolve;
mod types;

#[cfg(test)]
mod tests;

pub use hooks::PluginHookRunner;
pub use runtime::{HookEvent, HookRunResult};

pub use definition::builtin_plugins;
pub use error::{PluginError, PluginManifestValidationError};
pub use manifest::load_plugin_from_directory;
pub use types::*;
