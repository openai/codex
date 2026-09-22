//! Configured Windows sandbox modes and the effective local backend.
//! Keep the configured backend for remote inheritance; apply the local rollout
//! using the preference resolved during config loading.

use super::Config;
use super::EffectivePermissionSelection;
use super::apply_requirement_constrained_value;
use super::network_proxy_toml_config;
use super::permissions::network_proxy_config_for_profile_selection;
use super::profile_allows_configured_network_proxy;
use codex_config::ConstrainedWithSource;
use codex_config::NetworkConstraints;
use codex_config::Sourced;
use codex_config::types::WindowsSandboxModeToml;
use codex_features::FeaturesToml;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_sandboxing::SandboxType;

impl Config {
    /// Configured backend, without the local rollout preference. Remote executors inherit this.
    pub fn windows_sandbox_type_from_config(&self) -> SandboxType {
        self.permissions.windows_sandbox_type
    }

    /// Effective local backend, using eligibility and availability resolved at config load.
    pub fn effective_local_windows_sandbox_type(&self) -> SandboxType {
        resolve_windows_sandbox_type(self.windows_sandbox_type_from_config(), self.prefer_mxc)
    }
}

pub(super) fn resolve_windows_sandbox_type(
    configured: SandboxType,
    prefer_mxc: bool,
) -> SandboxType {
    if prefer_mxc {
        SandboxType::WindowsMxc
    } else {
        configured
    }
}

#[derive(Debug, PartialEq)]
pub struct PreparedWindowsSandboxConfig {
    /// Explicit or requirement-constrained mode; excludes feature-only fallback.
    pub mode: Option<WindowsSandboxModeToml>,
    /// Selected implementation, kept separate from the legacy setup level.
    pub sandbox_type: SandboxType,
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
        WindowsSandboxLevel::Disabled => None,
    });
    apply_requirement_constrained_value("windows.sandbox", selected_mode, constraint, warnings)?;
    let effective_mode = *constraint.get();
    let mode = if constraint.source.is_some() {
        effective_mode
    } else {
        configured_mode
    };
    let (sandbox_type, level) = match effective_mode {
        Some(WindowsSandboxModeToml::Elevated) => (
            SandboxType::WindowsRestrictedToken,
            WindowsSandboxLevel::Elevated,
        ),
        Some(WindowsSandboxModeToml::Unelevated) => (
            SandboxType::WindowsRestrictedToken,
            WindowsSandboxLevel::RestrictedToken,
        ),
        Some(WindowsSandboxModeToml::Mxc) => {
            (SandboxType::WindowsMxc, WindowsSandboxLevel::Disabled)
        }
        None => (SandboxType::None, WindowsSandboxLevel::Disabled),
    };
    Ok(PreparedWindowsSandboxConfig {
        mode,
        sandbox_type,
        level,
    })
}

/// Managed requirements take precedence; otherwise preserve explicit
/// feature-level or active-profile binding denials during automatic selection.
pub(super) fn network_config_allows_mxc(
    permission_selection: &EffectivePermissionSelection<'_>,
    profiles_are_active: bool,
    permission_profile: Option<&PermissionProfile>,
    network_requirements: Option<&Sourced<NetworkConstraints>>,
    features: Option<&FeaturesToml>,
    enable_network_proxy: bool,
) -> std::io::Result<bool> {
    let profile_local_binding = if profiles_are_active
        && permission_profile.is_none_or(profile_allows_configured_network_proxy)
        && let Some(profile) = permission_selection.selected_profile_id
    {
        network_proxy_config_for_profile_selection(permission_selection.profiles.as_ref(), profile)?
            .allow_local_binding
    } else {
        None
    };
    let allow_local_binding = network_requirements
        .and_then(|requirements| requirements.value.allow_local_binding)
        .or_else(|| {
            network_proxy_toml_config(features)
                .filter(|config| enable_network_proxy && config.allow_local_binding == Some(false))
                .and_then(|config| config.allow_local_binding)
        })
        .or(profile_local_binding);
    Ok(allow_local_binding != Some(false))
}

#[cfg(test)]
#[path = "windows_sandbox_config_tests.rs"]
mod tests;
