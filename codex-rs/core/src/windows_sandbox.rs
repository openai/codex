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
        if features.enabled(Feature::WindowsSandboxElevated) {
            return WindowsSandboxLevel::Elevated;
        }
        if features.enabled(Feature::WindowsSandbox) {
            WindowsSandboxLevel::RestrictedToken
        } else {
            WindowsSandboxLevel::Disabled
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
pub fn elevated_setup_failure_details(err: &anyhow::Error) -> Option<(String, String)> {
    let failure = codex_windows_sandbox::extract_setup_failure(err)?;
    let code = failure.code.as_str().to_string();
    let message = codex_windows_sandbox::sanitize_setup_metric_tag_value(&failure.message);
    Some((code, message))
}

#[cfg(not(target_os = "windows"))]
pub fn elevated_setup_failure_details(_err: &anyhow::Error) -> Option<(String, String)> {
    None
}

#[cfg(target_os = "windows")]
pub fn elevated_setup_failure_metric_name(err: &anyhow::Error) -> &'static str {
    if codex_windows_sandbox::extract_setup_failure(err).is_some_and(|failure| {
        matches!(
            failure.code,
            codex_windows_sandbox::SetupErrorCode::OrchestratorHelperLaunchCanceled
        )
    }) {
        "codex.windows_sandbox.elevated_setup_canceled"
    } else {
        "codex.windows_sandbox.elevated_setup_failure"
    }
}

#[cfg(not(target_os = "windows"))]
pub fn elevated_setup_failure_metric_name(_err: &anyhow::Error) -> &'static str {
    panic!("elevated_setup_failure_metric_name is only supported on Windows")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::Features;
    use pretty_assertions::assert_eq;

    #[test]
    fn elevated_flag_works_by_itself() {
        let mut features = Features::with_defaults();
        features.enable(Feature::WindowsSandboxElevated);

        assert_eq!(
            WindowsSandboxLevel::from_features(&features),
            WindowsSandboxLevel::Elevated
        );
    }

    #[test]
    fn restricted_token_flag_works_by_itself() {
        let mut features = Features::with_defaults();
        features.enable(Feature::WindowsSandbox);

        assert_eq!(
            WindowsSandboxLevel::from_features(&features),
            WindowsSandboxLevel::RestrictedToken
        );
    }

    #[test]
    fn no_flags_means_no_sandbox() {
        let features = Features::with_defaults();

        assert_eq!(
            WindowsSandboxLevel::from_features(&features),
            WindowsSandboxLevel::Disabled
        );
    }

    #[test]
    fn elevated_wins_when_both_flags_are_enabled() {
        let mut features = Features::with_defaults();
        features.enable(Feature::WindowsSandbox);
        features.enable(Feature::WindowsSandboxElevated);

        assert_eq!(
            WindowsSandboxLevel::from_features(&features),
            WindowsSandboxLevel::Elevated
        );
    }
}
