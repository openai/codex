use crate::config::Config;
use crate::features::Feature;
use crate::features::Features;
use crate::protocol::SandboxPolicy;
use codex_protocol::config_types::WindowsSandboxLevel;
use std::collections::HashMap;
use std::path::Path;

/// Kill switch for the elevated sandbox NUX on Windows.
///
/// When false, revert to the previous sandbox NUX, which only
/// prompts users to enable the legacy sandbox feature.
pub const ELEVATED_SANDBOX_NUX_ENABLED: bool = true;

pub trait WindowsSandboxLevelExt {
    fn from_config(config: &Config) -> WindowsSandboxLevel;
    fn from_features(features: &Features) -> WindowsSandboxLevel;
}

impl WindowsSandboxLevelExt for WindowsSandboxLevel {
    fn from_config(config: &Config) -> WindowsSandboxLevel {
        Self::from_features(&config.features)
    }

    fn from_features(features: &Features) -> WindowsSandboxLevel {
        if !features.enabled(Feature::WindowsSandbox) {
            return WindowsSandboxLevel::Disabled;
        }
        if features.enabled(Feature::WindowsSandboxElevated) {
            WindowsSandboxLevel::Elevated
        } else {
            WindowsSandboxLevel::RestrictedToken
        }
    }
}

pub fn windows_sandbox_level_from_config(config: &Config) -> WindowsSandboxLevel {
    WindowsSandboxLevel::from_config(config)
}

pub fn windows_sandbox_level_from_features(features: &Features) -> WindowsSandboxLevel {
    WindowsSandboxLevel::from_features(features)
}

#[cfg(target_os = "windows")]
pub fn sandbox_setup_is_complete(codex_home: &Path) -> bool {
    codex_windows_sandbox::sandbox_setup_is_complete(codex_home)
}

#[cfg(not(target_os = "windows"))]
pub fn sandbox_setup_is_complete(_codex_home: &Path) -> bool {
    false
}

#[cfg(target_os = "windows")]
pub fn run_elevated_setup(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
) -> anyhow::Result<()> {
    codex_windows_sandbox::run_elevated_setup(
        policy,
        policy_cwd,
        command_cwd,
        env_map,
        codex_home,
        None,
        None,
    )
}

#[cfg(not(target_os = "windows"))]
pub fn run_elevated_setup(
    _policy: &SandboxPolicy,
    _policy_cwd: &Path,
    _command_cwd: &Path,
    _env_map: &HashMap<String, String>,
    _codex_home: &Path,
) -> anyhow::Result<()> {
    anyhow::bail!("elevated Windows sandbox setup is only supported on Windows")
}
