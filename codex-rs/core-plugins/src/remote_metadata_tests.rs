//! Regression coverage for the fields that affect plugin behavior.

use super::*;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use crate::remote::group_remote_installed_plugins_by_marketplaces;
use codex_app_server_protocol::PluginAuthPolicy;
use codex_app_server_protocol::PluginAvailability;
use codex_app_server_protocol::PluginInstallPolicy;
use serde_json::json;

fn image_url(renewal: &str) -> String {
    format!("https://files.openai.com/plugins/icon.png?sig={renewal}")
}

fn plugin(renewal: &str) -> RemoteInstalledPlugin {
    RemoteInstalledPlugin {
        marketplace_name: REMOTE_GLOBAL_MARKETPLACE_NAME.to_string(),
        id: "plugin-test".to_string(),
        version: Some("1.0.0".to_string()),
        name: "test".to_string(),
        canonical_app_id: None,
        installed_at: None,
        enabled: true,
        install_policy: PluginInstallPolicy::Available,
        install_policy_source: None,
        must_show_installation_interstitial: None,
        auth_policy: PluginAuthPolicy::OnUse,
        availability: PluginAvailability::Available,
        disabled_reason: None,
        eligible_plan_types: None,
        interface: Some(
            serde_json::from_value(json!({
                "capabilities": [], "screenshots": [],
                "logoUrl": image_url(renewal), "logoUrlDark": image_url(renewal),
                "composerIconUrl": image_url(renewal), "screenshotUrls": [image_url(renewal)]
            }))
            .unwrap(),
        ),
        keywords: vec![],
    }
}

fn catalog(plugins: &[RemoteInstalledPlugin]) -> Vec<RemoteMarketplace> {
    group_remote_installed_plugins_by_marketplaces(plugins, &[REMOTE_GLOBAL_MARKETPLACE_NAME])
}

#[test]
fn behavioral_metadata_changes_still_invalidate() {
    let changes: &[fn(&mut RemoteInstalledPlugin)] = &[
        |plugin| plugin.id.push_str("-other"),
        |plugin| plugin.name.push_str("-other"),
        |plugin| plugin.version = Some("2.0.0".to_string()),
        |plugin| {
            plugin.installed_at =
                chrono::DateTime::from_timestamp(/*secs*/ 1, /*nsecs*/ 0)
        },
        |plugin| plugin.enabled = false,
        |plugin| plugin.install_policy = PluginInstallPolicy::NotAvailable,
        |plugin| {
            plugin.install_policy_source =
                Some(codex_app_server_protocol::PluginInstallPolicySource::WorkspaceSetting)
        },
        |plugin| plugin.must_show_installation_interstitial = Some(true),
        |plugin| plugin.auth_policy = PluginAuthPolicy::OnInstall,
        |plugin| plugin.availability = PluginAvailability::DisabledByAdmin,
        |plugin| plugin.eligible_plan_types = Some(vec!["enterprise".to_string()]),
    ];
    for change in changes {
        let previous = [plugin("old")];
        let mut current = [plugin("new")];
        change(&mut current[0]);
        assert!(!installed_plugin_metadata_eq(&previous, &current));
        assert!(!remote_catalog_metadata_eq(
            &catalog(&previous),
            &catalog(&current)
        ));
    }
    let previous = [plugin("old")];
    let mut current = [plugin("new")];
    current[0].canonical_app_id = Some("new-app".to_string());
    assert!(!installed_plugin_metadata_eq(&previous, &current));
    assert!(!installed_plugin_metadata_eq(&previous, &[]));
    assert!(!remote_catalog_metadata_eq(&catalog(&previous), &[]));
}

#[test]
fn display_metadata_does_not_invalidate() {
    let previous = [plugin("old")];
    let mut current = [plugin("new")];
    current[0].keywords.push("new keyword".to_string());
    let interface = current[0].interface.as_mut().unwrap();
    interface.logo_url = Some("https://another-host.test/different-image.svg".to_string());
    interface.display_name = Some("New display name".to_string());
    interface.short_description = Some("New description".to_string());
    interface.brand_color = Some("#123456".to_string());
    interface.default_prompt = Some(vec!["New starter prompt".to_string()]);
    interface.website_url = Some("https://another-host.test".to_string());
    assert_ne!(previous, current);
    assert!(installed_plugin_metadata_eq(&previous, &current));
    let mut current_catalog = catalog(&current);
    current_catalog[0].display_name = "New marketplace title".to_string();
    assert!(remote_catalog_metadata_eq(
        &catalog(&previous),
        &current_catalog
    ));

    current[0].interface = None;
    assert!(installed_plugin_metadata_eq(&previous, &current));
    assert!(remote_catalog_metadata_eq(
        &catalog(&previous),
        &catalog(&current)
    ));
}

#[test]
fn installed_ordering_remains_significant() {
    let mut other = plugin("old");
    other.id.push_str("-other");
    other.name.push_str("-other");
    let previous = [plugin("old"), other];
    let mut current = previous.clone();
    current.reverse();
    assert!(!installed_plugin_metadata_eq(&previous, &current));
}

#[test]
fn store_badge_changes_do_not_invalidate() {
    let mut previous = [plugin("old")];
    previous[0].interface.as_mut().unwrap().capabilities =
        vec!["tools".to_string(), "skills".to_string()];
    for badges in [vec!["skills", "tools"], vec!["updated badge"], vec![]] {
        let mut current = previous.clone();
        current[0].interface.as_mut().unwrap().capabilities =
            badges.into_iter().map(str::to_string).collect();
        assert_ne!(previous, current);
        assert!(installed_plugin_metadata_eq(&previous, &current));
        assert!(remote_catalog_metadata_eq(
            &catalog(&previous),
            &catalog(&current)
        ));
    }
}

#[test]
fn catalog_display_reordering_does_not_invalidate() {
    let mut first = plugin("old");
    first.interface.as_mut().unwrap().display_name = Some("Alpha".to_string());
    let mut second = plugin("old");
    second.id.push_str("-second");
    second.name.push_str("-second");
    second.interface.as_mut().unwrap().display_name = Some("Beta".to_string());
    let previous = [first, second];
    let mut current = previous.clone();
    current[0].interface.as_mut().unwrap().display_name = Some("Zulu".to_string());
    let previous_catalog = catalog(&previous);
    let current_catalog = catalog(&current);
    assert_ne!(
        previous_catalog[0].plugins[0].id,
        current_catalog[0].plugins[0].id
    );
    assert!(installed_plugin_metadata_eq(&previous, &current));
    assert!(remote_catalog_metadata_eq(
        &previous_catalog,
        &current_catalog
    ));

    current[0].version = Some("2.0.0".to_string());
    assert!(!remote_catalog_metadata_eq(
        &previous_catalog,
        &catalog(&current)
    ));
    assert!(!remote_catalog_metadata_eq(
        &previous_catalog,
        &catalog(&current[1..])
    ));
    current[0] = current[1].clone();
    assert!(!remote_catalog_metadata_eq(
        &previous_catalog,
        &catalog(&current)
    ));
}
