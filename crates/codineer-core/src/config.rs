//! Shared configuration primitives used across crates.

use std::fmt::{Display, Formatter};

/// The scope / origin of a configuration entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigSource {
    User,
    Project,
    Local,
}

impl ConfigSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
            Self::Local => "local",
        }
    }
}

impl Display for ConfigSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
