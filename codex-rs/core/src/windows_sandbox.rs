use crate::config::Config;
use crate::config::ConfigToml;
use crate::config::profile::ConfigProfile;
use crate::config::types::WindowsSandboxModeToml;
use crate::features::Feature;
use crate::features::Features;
use crate::features::FeaturesToml;
use crate::protocol::SandboxPolicy;
use codex_protocol::config_types::WindowsSandboxLevel;
use std::collections::BTreeMap;
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
        match config.windows_sandbox_mode {
            Some(WindowsSandboxModeToml::Elevated) => WindowsSandboxLevel::Elevated,
            Some(WindowsSandboxModeToml::Unelevated) => WindowsSandboxLevel::RestrictedToken,
            None => Self::from_features(&config.features),
        }
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

pub fn resolve_windows_sandbox_mode(
    cfg: &ConfigToml,
    profile: &ConfigProfile,
) -> Option<WindowsSandboxModeToml> {
    if let Some(mode) = legacy_windows_sandbox_mode(profile.features.as_ref()) {
        return Some(mode);
    }
    if legacy_windows_sandbox_keys_present(profile.features.as_ref()) {
        return None;
    }

    profile
        .windows
        .as_ref()
        .and_then(|windows| windows.sandbox)
        .or_else(|| cfg.windows.as_ref().and_then(|windows| windows.sandbox))
        .or_else(|| legacy_windows_sandbox_mode(cfg.features.as_ref()))
}

fn legacy_windows_sandbox_keys_present(features: Option<&FeaturesToml>) -> bool {
    let Some(entries) = features.map(|features| &features.entries) else {
        return false;
    };
    entries.contains_key(Feature::WindowsSandboxElevated.key())
        || entries.contains_key(Feature::WindowsSandbox.key())
        || entries.contains_key("enable_experimental_windows_sandbox")
}

pub fn legacy_windows_sandbox_mode(
    features: Option<&FeaturesToml>,
) -> Option<WindowsSandboxModeToml> {
    let entries = features.map(|features| &features.entries)?;
    legacy_windows_sandbox_mode_from_entries(entries)
}

pub fn legacy_windows_sandbox_mode_from_entries(
    entries: &BTreeMap<String, bool>,
) -> Option<WindowsSandboxModeToml> {
    if entries
        .get(Feature::WindowsSandboxElevated.key())
        .copied()
        .unwrap_or(false)
    {
        return Some(WindowsSandboxModeToml::Elevated);
    }
    if entries
        .get(Feature::WindowsSandbox.key())
        .copied()
        .unwrap_or(false)
        || entries
            .get("enable_experimental_windows_sandbox")
            .copied()
            .unwrap_or(false)
    {
        Some(WindowsSandboxModeToml::Unelevated)
    } else {
        None
    }
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

#[cfg(target_os = "windows")]
pub fn run_legacy_setup_preflight(
    policy: &SandboxPolicy,
    policy_cwd: &Path,
    command_cwd: &Path,
    env_map: &HashMap<String, String>,
    codex_home: &Path,
) -> anyhow::Result<()> {
    codex_windows_sandbox::run_windows_sandbox_legacy_preflight(
        policy,
        policy_cwd,
        codex_home,
        command_cwd,
        env_map,
    )
}

#[cfg(not(target_os = "windows"))]
pub fn run_legacy_setup_preflight(
    _policy: &SandboxPolicy,
    _policy_cwd: &Path,
    _command_cwd: &Path,
    _env_map: &HashMap<String, String>,
    _codex_home: &Path,
) -> anyhow::Result<()> {
    anyhow::bail!("legacy Windows sandbox setup is only supported on Windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::WindowsToml;
    use crate::features::Features;
    use crate::features::FeaturesToml;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;

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

    #[test]
    fn legacy_mode_prefers_elevated() {
        let mut entries = BTreeMap::new();
        entries.insert("experimental_windows_sandbox".to_string(), true);
        entries.insert("elevated_windows_sandbox".to_string(), true);

        assert_eq!(
            legacy_windows_sandbox_mode_from_entries(&entries),
            Some(WindowsSandboxModeToml::Elevated)
        );
    }

    #[test]
    fn legacy_mode_supports_alias_key() {
        let mut entries = BTreeMap::new();
        entries.insert("enable_experimental_windows_sandbox".to_string(), true);

        assert_eq!(
            legacy_windows_sandbox_mode_from_entries(&entries),
            Some(WindowsSandboxModeToml::Unelevated)
        );
    }

    #[test]
    fn resolve_windows_sandbox_mode_prefers_profile_windows() {
        let cfg = ConfigToml {
            windows: Some(WindowsToml {
                sandbox: Some(WindowsSandboxModeToml::Unelevated),
            }),
            ..Default::default()
        };
        let profile = ConfigProfile {
            windows: Some(WindowsToml {
                sandbox: Some(WindowsSandboxModeToml::Elevated),
            }),
            ..Default::default()
        };

        assert_eq!(
            resolve_windows_sandbox_mode(&cfg, &profile),
            Some(WindowsSandboxModeToml::Elevated)
        );
    }

    #[test]
    fn resolve_windows_sandbox_mode_falls_back_to_legacy_keys() {
        let mut entries = BTreeMap::new();
        entries.insert("experimental_windows_sandbox".to_string(), true);
        let cfg = ConfigToml {
            features: Some(FeaturesToml { entries }),
            ..Default::default()
        };

        assert_eq!(
            resolve_windows_sandbox_mode(&cfg, &ConfigProfile::default()),
            Some(WindowsSandboxModeToml::Unelevated)
        );
    }

    #[test]
    fn resolve_windows_sandbox_mode_profile_legacy_false_blocks_top_level_legacy_true() {
        let mut profile_entries = BTreeMap::new();
        profile_entries.insert("experimental_windows_sandbox".to_string(), false);
        let profile = ConfigProfile {
            features: Some(FeaturesToml {
                entries: profile_entries,
            }),
            ..Default::default()
        };

        let mut cfg_entries = BTreeMap::new();
        cfg_entries.insert("experimental_windows_sandbox".to_string(), true);
        let cfg = ConfigToml {
            features: Some(FeaturesToml {
                entries: cfg_entries,
            }),
            ..Default::default()
        };

        assert_eq!(resolve_windows_sandbox_mode(&cfg, &profile), None);
    }
}
