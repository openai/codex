//! Configured Windows sandbox modes and the effective local backend.
//! Keep the configured backend for remote inheritance; apply the local rollout
//! using the preference resolved during config loading.
//! Validate MXC requirements only after bootstrap has loaded the complete config.

use super::Config;
use super::EffectivePermissionSelection;
use super::apply_requirement_constrained_value;
use super::network_proxy_toml_config;
use super::permissions::network_proxy_config_for_profile_selection;
use super::profile_allows_configured_network_proxy;
use codex_config::ConstrainedWithSource;
use codex_config::NetworkConstraints;
use codex_config::Sourced;
use codex_config::config_toml::ConfigToml;
use codex_config::types::WindowsSandboxModeToml;
use codex_features::FeaturesToml;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_sandboxing::SandboxType;

impl Config {
    /// Reject a non-MXC selection without changing the final configuration.
    /// Call after cloud configuration loads and outside config-recovery fallbacks.
    pub fn validate_windows_mxc_requirement(&self) -> std::io::Result<()> {
        self.validate_windows_mxc_requirement_from(self)
    }

    /// Check this execution config against separately loaded managed requirements.
    pub fn validate_windows_mxc_requirement_from(
        &self,
        policy_config: &Self,
    ) -> std::io::Result<()> {
        if cfg!(windows)
            && policy_config
                .config_layer_stack
                .requirements_toml()
                .windows
                .as_ref()
                .and_then(|windows| windows.require_mxc)
                == Some(true)
            && self.effective_local_windows_sandbox_type() != SandboxType::WindowsMxc
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "windows.require_mxc = true requires MXC to be selected on this Windows host",
            ));
        }
        Ok(())
    }

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
    if configured_mode == Some(WindowsSandboxModeToml::Mxc) {
        constraint.can_set(&configured_mode)?;
    }
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

/// Preserve ordinary and managed MXC opt-outs plus network binding denials.
pub(super) fn config_allows_mxc(
    windows_sandbox_mode: &ConstrainedWithSource<Option<WindowsSandboxModeToml>>,
    permission_selection: &EffectivePermissionSelection<'_>,
    profiles_are_active: bool,
    permission_profile: Option<&PermissionProfile>,
    network_requirements: Option<&Sourced<NetworkConstraints>>,
    cfg: &ConfigToml,
    enable_network_proxy: bool,
) -> std::io::Result<bool> {
    if cfg.windows.as_ref().and_then(|windows| windows.allow_mxc) == Some(false)
        || windows_sandbox_mode
            .can_set(&Some(WindowsSandboxModeToml::Mxc))
            .is_err()
    {
        return Ok(false);
    }
    let profile_local_binding = if profiles_are_active
        && permission_profile.is_none_or(profile_allows_configured_network_proxy)
        && let Some(profile) = permission_selection.selected_profile_id
    {
        network_proxy_config_for_profile_selection(permission_selection.profiles.as_ref(), profile)?
            .allow_local_binding
    } else {
        None
    };
    Ok(windows_mxc_allowed_by_config(
        windows_sandbox_mode,
        network_requirements,
        cfg.features.as_ref(),
        enable_network_proxy,
        profile_local_binding,
    ))
}

/// Eligibility shared with remote selection after the caller resolves its active profile.
/// Managed network requirements outrank feature and profile binding restrictions.
pub fn windows_mxc_allowed_by_config(
    windows_sandbox_mode: &ConstrainedWithSource<Option<WindowsSandboxModeToml>>,
    network_requirements: Option<&Sourced<NetworkConstraints>>,
    features: Option<&FeaturesToml>,
    enable_network_proxy: bool,
    profile_local_binding: Option<bool>,
) -> bool {
    if windows_sandbox_mode
        .can_set(&Some(WindowsSandboxModeToml::Mxc))
        .is_err()
    {
        return false;
    }
    let allow_local_binding = network_requirements
        .and_then(|requirements| requirements.value.allow_local_binding)
        .or_else(|| {
            network_proxy_toml_config(features)
                .filter(|config| enable_network_proxy && config.allow_local_binding == Some(false))
                .and_then(|config| config.allow_local_binding)
        })
        .or(profile_local_binding);
    allow_local_binding != Some(false)
}

#[cfg(test)]
#[path = "windows_sandbox_config_tests.rs"]
mod tests;
