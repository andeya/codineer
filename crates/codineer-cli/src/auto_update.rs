//! Auto-update infrastructure for checking and applying Codineer updates.
//!
//! Checks a release endpoint for newer versions and optionally applies
//! updates. Designed to be non-blocking and user-consented.

use std::time::SystemTime;

/// Represents version information from the release endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub version: String,
    pub download_url: String,
    pub release_notes: String,
    pub published_at: String,
}

/// Result of an update check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheckResult {
    UpToDate {
        current: String,
    },
    UpdateAvailable {
        current: String,
        latest: ReleaseInfo,
    },
    CheckFailed {
        reason: String,
    },
}

/// Configuration for the auto-update system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateConfig {
    pub enabled: bool,
    pub check_interval_hours: u64,
    pub release_channel: ReleaseChannel,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval_hours: 24,
            release_channel: ReleaseChannel::Stable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseChannel {
    Stable,
    Beta,
    Nightly,
}

/// Tracks when the last update check was performed.
#[derive(Debug, Clone)]
pub struct UpdateCheckState {
    pub last_checked: Option<SystemTime>,
    pub dismissed_version: Option<String>,
}

impl UpdateCheckState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            last_checked: None,
            dismissed_version: None,
        }
    }

    /// Whether enough time has passed since the last check.
    #[must_use]
    pub fn should_check(&self, interval_hours: u64) -> bool {
        let Some(last) = self.last_checked else {
            return true;
        };
        let elapsed = SystemTime::now().duration_since(last).unwrap_or_default();
        elapsed.as_secs() >= interval_hours * 3600
    }
}

impl Default for UpdateCheckState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_config_defaults() {
        let config = UpdateConfig::default();
        assert!(config.enabled);
        assert_eq!(config.check_interval_hours, 24);
        assert_eq!(config.release_channel, ReleaseChannel::Stable);
    }

    #[test]
    fn should_check_when_never_checked() {
        let state = UpdateCheckState::new();
        assert!(state.should_check(24));
    }

    #[test]
    fn should_not_check_when_recently_checked() {
        let state = UpdateCheckState {
            last_checked: Some(SystemTime::now()),
            dismissed_version: None,
        };
        assert!(!state.should_check(24));
    }

    #[test]
    fn update_check_result_variants() {
        let up_to_date = UpdateCheckResult::UpToDate {
            current: "0.6.0".to_string(),
        };
        assert!(matches!(up_to_date, UpdateCheckResult::UpToDate { .. }));

        let available = UpdateCheckResult::UpdateAvailable {
            current: "0.6.0".to_string(),
            latest: ReleaseInfo {
                version: "0.7.0".to_string(),
                download_url: "https://example.com/release".to_string(),
                release_notes: "Bug fixes".to_string(),
                published_at: "2026-04-01".to_string(),
            },
        };
        assert!(matches!(
            available,
            UpdateCheckResult::UpdateAvailable { .. }
        ));
    }
}
