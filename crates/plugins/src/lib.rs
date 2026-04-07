mod hooks;

mod bundled;
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

// PluginHookRunner is preserved in hooks.rs for reference but not re-exported;
// its functionality is superseded by HookDispatcher in the runtime crate.
pub use runtime::{HookEvent, HookRunResult};

pub use definition::builtin_plugins;
pub use error::{PluginError, PluginManifestValidationError};
pub use manifest::load_plugin_from_directory;
pub use types::*;
