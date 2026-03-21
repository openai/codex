use super::*;
use codex_features::Feature;
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
        Some(WindowsSandboxMode::Elevated)
    );
}

#[test]
fn legacy_mode_supports_alias_key() {
    let mut entries = BTreeMap::new();
    entries.insert("enable_experimental_windows_sandbox".to_string(), true);

    assert_eq!(
        legacy_windows_sandbox_mode_from_entries(&entries),
        Some(WindowsSandboxMode::Unelevated)
    );
}

#[test]
fn resolve_windows_sandbox_mode_prefers_profile_windows() {
    assert_eq!(
        resolve_windows_sandbox_mode(
            Some(WindowsSandboxMode::Elevated),
            /*profile_features*/ None,
            Some(WindowsSandboxMode::Unelevated),
            /*cfg_features*/ None,
        ),
        Some(WindowsSandboxMode::Elevated)
    );
}

#[test]
fn resolve_windows_sandbox_mode_falls_back_to_legacy_keys() {
    let mut entries = BTreeMap::new();
    entries.insert("experimental_windows_sandbox".to_string(), true);
    let cfg_features = FeaturesToml { entries };

    assert_eq!(
        resolve_windows_sandbox_mode(
            /*profile_mode*/ None,
            /*profile_features*/ None,
            /*cfg_mode*/ None,
            Some(&cfg_features),
        ),
        Some(WindowsSandboxMode::Unelevated)
    );
}

#[test]
fn resolve_windows_sandbox_mode_profile_legacy_false_blocks_top_level_legacy_true() {
    let mut profile_entries = BTreeMap::new();
    profile_entries.insert("experimental_windows_sandbox".to_string(), false);
    let profile_features = FeaturesToml {
        entries: profile_entries,
    };

    let mut cfg_entries = BTreeMap::new();
    cfg_entries.insert("experimental_windows_sandbox".to_string(), true);
    let cfg_features = FeaturesToml {
        entries: cfg_entries,
    };

    assert_eq!(
        resolve_windows_sandbox_mode(
            /*profile_mode*/ None,
            Some(&profile_features),
            /*cfg_mode*/ None,
            Some(&cfg_features),
        ),
        None
    );
}

#[test]
fn resolve_windows_sandbox_private_desktop_prefers_profile_windows() {
    assert!(resolve_windows_sandbox_private_desktop(
        /*profile_private_desktop*/ Some(true),
        /*cfg_private_desktop*/ Some(false),
    ));
}

#[test]
fn resolve_windows_sandbox_private_desktop_defaults_to_true() {
    let profile_private_desktop = None;
    let cfg_private_desktop = None;

    assert!(resolve_windows_sandbox_private_desktop(
        profile_private_desktop,
        cfg_private_desktop,
    ));
}

#[test]
fn resolve_windows_sandbox_private_desktop_respects_explicit_cfg_value() {
    assert!(!resolve_windows_sandbox_private_desktop(
        /*profile_private_desktop*/ None,
        /*cfg_private_desktop*/ Some(false),
    ));
}
