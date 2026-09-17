//! Bounded observations for foreground callers. This module never exports telemetry;
//! in particular, the detached updater must not create a telemetry provider.

use std::path::Path;

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
