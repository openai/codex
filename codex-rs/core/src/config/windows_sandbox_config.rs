//! Windows sandbox mode selection after managed features have been resolved.
//! Keep configured mode distinct from the effective level unless policy pins it.

use super::apply_requirement_constrained_value;
use codex_config::ConstrainedWithSource;
use codex_config::types::WindowsSandboxModeToml;
use codex_protocol::config_types::WindowsSandboxLevel;

#[derive(Debug, PartialEq)]
pub struct PreparedWindowsSandboxConfig {
    /// Explicit or requirement-constrained mode; excludes feature-only fallback.
    pub mode: Option<WindowsSandboxModeToml>,
    /// Effective sandbox level after Windows requirements have been applied.
    pub level: WindowsSandboxLevel,
}

/// Applies Windows requirements to the configured mode or feature-derived level.
/// The feature-derived level must already reflect managed feature requirements.
pub fn prepare_windows_sandbox_config(
    configured_mode: Option<WindowsSandboxModeToml>,
    feature_level: WindowsSandboxLevel,
    constraint: &mut ConstrainedWithSource<Option<WindowsSandboxModeToml>>,
    warnings: &mut Vec<String>,
) -> std::io::Result<PreparedWindowsSandboxConfig> {
    let selected_mode = configured_mode.or(match feature_level {
        WindowsSandboxLevel::Elevated => Some(WindowsSandboxModeToml::Elevated),
        WindowsSandboxLevel::RestrictedToken => Some(WindowsSandboxModeToml::Unelevated),
        WindowsSandboxLevel::Disabled | WindowsSandboxLevel::Mxc => None,
    });
    apply_requirement_constrained_value("windows.sandbox", selected_mode, constraint, warnings)?;
    let effective_mode = *constraint.get();
    let mode = if constraint.source.is_some() {
        effective_mode
    } else {
        configured_mode
    };
    let level = match effective_mode {
        Some(WindowsSandboxModeToml::Elevated) => WindowsSandboxLevel::Elevated,
        Some(WindowsSandboxModeToml::Unelevated) => WindowsSandboxLevel::RestrictedToken,
        None => WindowsSandboxLevel::Disabled,
    };
    Ok(PreparedWindowsSandboxConfig { mode, level })
}

#[cfg(test)]
#[path = "windows_sandbox_config_tests.rs"]
mod tests;
