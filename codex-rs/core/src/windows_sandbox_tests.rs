use super::*;
use codex_config::types::WindowsToml;
use codex_features::Features;
use codex_features::FeaturesToml;
use codex_network_proxy::NetworkProxyConfig;
use codex_protocol::models::PermissionProfile;
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
    entries.insert(
        "experimental_windows_sandbox".to_string(),
        /*value*/ true,
    );
    entries.insert("elevated_windows_sandbox".to_string(), /*value*/ true);

    assert_eq!(
        legacy_windows_sandbox_mode_from_entries(&entries),
        Some(WindowsSandboxModeToml::Elevated)
    );
}

#[test]
fn legacy_mode_supports_alias_key() {
    let mut entries = BTreeMap::new();
    entries.insert(
        "enable_experimental_windows_sandbox".to_string(),
        /*value*/ true,
    );

    assert_eq!(
        legacy_windows_sandbox_mode_from_entries(&entries),
        Some(WindowsSandboxModeToml::Unelevated)
    );
}

#[test]
fn resolve_windows_sandbox_mode_falls_back_to_legacy_keys() {
    let mut entries = BTreeMap::new();
    entries.insert(
        "experimental_windows_sandbox".to_string(),
        /*value*/ true,
    );
    let cfg = ConfigToml {
        features: Some(FeaturesToml::from(entries)),
        ..Default::default()
    };

    assert_eq!(
        resolve_windows_sandbox_mode(&cfg),
        Some(WindowsSandboxModeToml::Unelevated)
    );
}

#[test]
fn resolve_windows_sandbox_private_desktop_defaults_to_true() {
    assert!(resolve_windows_sandbox_private_desktop(
        &ConfigToml::default()
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

    assert!(!resolve_windows_sandbox_private_desktop(&cfg));
}

#[test]
fn provisioning_settings_omit_the_disabled_socks_proxy() {
    let config = NetworkProxyConfig {
        enabled: true,
        proxy_url: "http://127.0.0.1:43128".to_string(),
        enable_socks5: false,
        socks_url: "socks5h://127.0.0.1:48081".to_string(),
        allow_local_binding: true,
        ..Default::default()
    };
    let spec = crate::config::NetworkProxySpec::from_config_and_constraints(
        config,
        /*requirements*/ None,
        &PermissionProfile::workspace_write(),
    )
    .expect("managed proxy config should resolve");

    assert_eq!(
        provisioning_settings(Some(&spec)).expect("provisioning settings should resolve"),
        codex_windows_sandbox::WindowsSandboxProvisioningSettings {
            proxy_ports: vec![43128],
            allow_local_binding: true,
        }
    );
}

#[test]
fn provisioning_settings_are_empty_when_managed_network_is_disabled() {
    let spec = crate::config::NetworkProxySpec::from_config_and_constraints(
        NetworkProxyConfig::default(),
        /*requirements*/ None,
        &PermissionProfile::workspace_write(),
    )
    .expect("managed proxy config should resolve");

    assert_eq!(
        provisioning_settings(Some(&spec)).expect("provisioning settings should resolve"),
        codex_windows_sandbox::WindowsSandboxProvisioningSettings::default()
    );
}
