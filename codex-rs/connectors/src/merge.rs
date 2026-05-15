use std::collections::HashMap;
use std::collections::HashSet;

use crate::metadata::connector_install_url;
use crate::metadata::sort_connectors_by_accessibility_and_name;
use codex_app_server_protocol::AppInfo;

pub fn merge_connectors(
    connectors: Vec<AppInfo>,
    accessible_connectors: Vec<AppInfo>,
) -> Vec<AppInfo> {
    let mut merged: HashMap<String, AppInfo> = connectors
        .into_iter()
        .map(|mut connector| {
            connector.is_accessible = false;
            (connector.id.clone(), connector)
        })
        .collect();

    for mut connector in accessible_connectors {
        connector.is_accessible = true;
        let connector_id = connector.id.clone();
        if let Some(existing) = merged.get_mut(&connector_id) {
            existing.is_accessible = true;
            if existing.name == existing.id && connector.name != connector.id {
                existing.name = connector.name;
            }
            if existing.description.is_none() && connector.description.is_some() {
                existing.description = connector.description;
            }
            if existing.logo_url.is_none() && connector.logo_url.is_some() {
                existing.logo_url = connector.logo_url;
            }
            if existing.logo_url_dark.is_none() && connector.logo_url_dark.is_some() {
                existing.logo_url_dark = connector.logo_url_dark;
            }
            if existing.distribution_channel.is_none() && connector.distribution_channel.is_some() {
                existing.distribution_channel = connector.distribution_channel;
            }
            existing
                .plugin_display_names
                .extend(connector.plugin_display_names);
        } else {
            merged.insert(connector_id, connector);
        }
    }

    let mut merged = merged.into_values().collect::<Vec<_>>();
    for connector in &mut merged {
        if connector.install_url.is_none() {
            connector.install_url = Some(connector_install_url(&connector.name, &connector.id));
        }
        connector.plugin_display_names.sort_unstable();
        connector.plugin_display_names.dedup();
    }
    sort_connectors_by_accessibility_and_name(&mut merged);
    merged
}

pub fn merge_plugin_connectors<I>(connectors: Vec<AppInfo>, plugin_app_ids: I) -> Vec<AppInfo>
where
    I: IntoIterator<Item = String>,
{
    let mut merged = connectors;
    let mut connector_ids = merged
        .iter()
        .map(|connector| connector.id.clone())
        .collect::<HashSet<_>>();

    for connector_id in plugin_app_ids {
        if connector_ids.insert(connector_id.clone()) {
            merged.push(plugin_connector_to_app_info(connector_id));
        }
    }

    sort_connectors_by_accessibility_and_name(&mut merged);
    merged
}

pub fn merge_plugin_connectors_with_accessible<I>(
    plugin_app_ids: I,
    accessible_connectors: Vec<AppInfo>,
) -> Vec<AppInfo>
where
    I: IntoIterator<Item = String>,
{
    let accessible_connector_ids: HashSet<&str> = accessible_connectors
        .iter()
        .map(|connector| connector.id.as_str())
        .collect();
    let plugin_connectors = plugin_app_ids
        .into_iter()
        .filter(|connector_id| accessible_connector_ids.contains(connector_id.as_str()))
        .map(plugin_connector_to_app_info)
        .collect::<Vec<_>>();
    merge_connectors(plugin_connectors, accessible_connectors)
}

pub fn plugin_connector_to_app_info(connector_id: String) -> AppInfo {
    // Leave the placeholder name as the connector id so merge_connectors() can
    // replace it with canonical app metadata from directory fetches or
    // connector_name values from codex_apps tool discovery.
    let name = connector_id.clone();
    AppInfo {
        id: connector_id.clone(),
        name: name.clone(),
        description: None,
        logo_url: None,
        logo_url_dark: None,
        distribution_channel: None,
        branding: None,
        app_metadata: None,
        labels: None,
        install_url: Some(connector_install_url(&name, &connector_id)),
        is_accessible: false,
        is_enabled: true,
        plugin_display_names: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::connector_install_url;
    use crate::metadata::connector_mention_slug;
    use pretty_assertions::assert_eq;

    fn plugin_names(names: &[&str]) -> Vec<String> {
        names.iter().map(ToString::to_string).collect()
    }

    fn google_calendar_accessible_connector(plugin_display_names: &[&str]) -> AppInfo {
        AppInfo {
            id: "calendar".to_string(),
            name: "Google Calendar".to_string(),
            description: Some("Plan events".to_string()),
            logo_url: Some("https://example.com/logo.png".to_string()),
            logo_url_dark: Some("https://example.com/logo-dark.png".to_string()),
            distribution_channel: Some("workspace".to_string()),
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: None,
            is_accessible: true,
            is_enabled: true,
            plugin_display_names: plugin_names(plugin_display_names),
        }
    }

    #[test]
    fn merge_connectors_replaces_plugin_placeholder_name_with_accessible_name() {
        let plugin = plugin_connector_to_app_info("calendar".to_string());
        let accessible = google_calendar_accessible_connector(&[]);

        let merged = merge_connectors(vec![plugin], vec![accessible]);

        assert_eq!(
            merged,
            vec![AppInfo {
                id: "calendar".to_string(),
                name: "Google Calendar".to_string(),
                description: Some("Plan events".to_string()),
                logo_url: Some("https://example.com/logo.png".to_string()),
                logo_url_dark: Some("https://example.com/logo-dark.png".to_string()),
                distribution_channel: Some("workspace".to_string()),
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some(connector_install_url("calendar", "calendar")),
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: Vec::new(),
            }]
        );
        assert_eq!(connector_mention_slug(&merged[0]), "google-calendar");
    }

    #[test]
    fn merge_connectors_unions_and_dedupes_plugin_display_names() {
        let mut plugin = plugin_connector_to_app_info("calendar".to_string());
        plugin.plugin_display_names = plugin_names(&["sample", "alpha", "sample"]);

        let accessible = google_calendar_accessible_connector(&["beta", "alpha"]);

        let merged = merge_connectors(vec![plugin], vec![accessible]);

        assert_eq!(
            merged,
            vec![AppInfo {
                id: "calendar".to_string(),
                name: "Google Calendar".to_string(),
                description: Some("Plan events".to_string()),
                logo_url: Some("https://example.com/logo.png".to_string()),
                logo_url_dark: Some("https://example.com/logo-dark.png".to_string()),
                distribution_channel: Some("workspace".to_string()),
                branding: None,
                app_metadata: None,
                labels: None,
                install_url: Some(connector_install_url("calendar", "calendar")),
                is_accessible: true,
                is_enabled: true,
                plugin_display_names: plugin_names(&["alpha", "beta", "sample"]),
            }]
        );
    }
}
