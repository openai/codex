use super::*;
use crate::config::types::WindowsToml;
use codex_features::Features;
use codex_features::FeaturesToml;
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
            ..Default::default()
        }),
        ..Default::default()
    };
    let profile = ConfigProfile {
        windows: Some(WindowsToml {
            sandbox: Some(WindowsSandboxModeToml::Elevated),
            ..Default::default()
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

#[test]
fn resolve_windows_sandbox_private_desktop_prefers_profile_windows() {
    let cfg = ConfigToml {
        windows: Some(WindowsToml {
            sandbox: Some(WindowsSandboxModeToml::Unelevated),
            sandbox_private_desktop: Some(false),
        }),
        ..Default::default()
    };
    let profile = ConfigProfile {
        windows: Some(WindowsToml {
            sandbox: Some(WindowsSandboxModeToml::Elevated),
            sandbox_private_desktop: Some(true),
        }),
        ..Default::default()
    };

    assert!(resolve_windows_sandbox_private_desktop(&cfg, &profile));
}

#[test]
fn resolve_windows_sandbox_private_desktop_defaults_to_true() {
    assert!(resolve_windows_sandbox_private_desktop(
        &ConfigToml::default(),
        &ConfigProfile::default()
    ));
}

#[test]
fn resolve_windows_sandbox_private_desktop_respects_explicit_cfg_value() {
    let cfg = ConfigToml {
        windows: Some(WindowsToml {
            sandbox_private_desktop: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    };

    assert!(!resolve_windows_sandbox_private_desktop(
        &cfg,
        &ConfigProfile::default()
    ));
}
