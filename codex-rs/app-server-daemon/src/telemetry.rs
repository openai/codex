//! Bounded observations for foreground callers. This module never exports telemetry;
//! in particular, the detached updater must not create a telemetry provider.

use std::path::Path;

/// Failure of a foreground auto-start, with the original diagnostic kept separate from the metric.
#[derive(Debug)]
pub struct StartError {
    pub error: anyhow::Error,
    during_install: bool,
}

impl StartError {
    pub fn failure_reason(&self) -> &'static str {
        #[cfg(windows)]
        if self.error.is::<crate::DetachedLaunchRestricted>() {
            return "spawn";
        }
        match self.error.downcast_ref::<StartFailure>() {
            Some(StartFailure::Spawn(_)) => "spawn",
            Some(StartFailure::EarlyChildExit(_)) => "early_child_exit",
            Some(StartFailure::ReadinessTimeout(_)) => "readiness_timeout",
            None if self.during_install => "install",
            None => "other",
        }
    }

    pub(crate) fn install(error: anyhow::Error) -> Self {
        Self {
            error,
            during_install: true,
        }
    }
}

impl From<anyhow::Error> for StartError {
    fn from(error: anyhow::Error) -> Self {
        Self {
            error,
            during_install: false,
        }
    }
}

/// Typed anyhow context preserves both the original diagnostic and downcasts through it.
#[derive(Debug)]
pub(crate) enum StartFailure {
    Spawn(String),
    EarlyChildExit(String),
    ReadinessTimeout(String),
}

impl std::fmt::Display for StartFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(message)
            | Self::EarlyChildExit(message)
            | Self::ReadinessTimeout(message) => f.write_str(message),
        }
    }
}

/// Suppress child reporting when the foreground TUI handoff owns the observation.
pub const HANDOFF_ENV: &str = "CODEX_DAEMON_TELEMETRY_HANDOFF";

/// Only presence and enabled state leave the settings reader, never values or contents.
/// Missing files mean defaults; unreadable or invalid settings remain unknown.
pub async fn settings_tags(codex_home: &Path) -> [(&'static str, &'static str); 4] {
    let path = codex_home
        .join(crate::STATE_DIR_NAME)
        .join(crate::SETTINGS_FILE_NAME);
    crate::settings::telemetry_tags(&path).await.unwrap_or([
        ("auto_update", "unknown"),
        ("auto_update_setting", "unknown"),
        ("update_interval_setting", "unknown"),
        ("shutdown_grace_setting", "unknown"),
    ])
}
