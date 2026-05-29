use std::collections::BTreeSet;
use std::collections::HashMap;

use crate::metadata::connector_install_url;
use crate::normalize_connector_value;
use codex_app_server_protocol::AppInfo;

pub struct AccessibleConnectorTool {
    pub connector_id: String,
    pub connector_name: Option<String>,
    pub connector_description: Option<String>,
    pub plugin_display_names: Vec<String>,
}

pub fn collect_accessible_connectors<I>(tools: I) -> Vec<AppInfo>
where
    I: IntoIterator<Item = AccessibleConnectorTool>,
{
    let mut connectors: HashMap<String, (AppInfo, BTreeSet<String>)> = HashMap::new();
    for tool in tools {
        let connector_id = tool.connector_id;
        let connector_name = normalize_connector_value(tool.connector_name.as_deref())
            .unwrap_or_else(|| connector_id.clone());
        let connector_description =
            normalize_connector_value(tool.connector_description.as_deref());
        if let Some((existing, existing_plugin_display_names)) = connectors.get_mut(&connector_id) {
            if existing.name == connector_id && connector_name != connector_id {
                existing.name = connector_name;
            }
            if existing.description.is_none() && connector_description.is_some() {
                existing.description = connector_description;
            }
            existing_plugin_display_names.extend(tool.plugin_display_names);
        } else {
            connectors.insert(
                connector_id.clone(),
                (
                    AppInfo {
                        id: connector_id.clone(),
                        name: connector_name,
                        description: connector_description,
                        logo_url: None,
                        logo_url_dark: None,
                        distribution_channel: None,
                        branding: None,
                        app_metadata: None,
                        labels: None,
                        install_url: None,
                        is_accessible: true,
                        is_enabled: true,
                        plugin_display_names: Vec::new(),
                    },
                    tool.plugin_display_names
                        .into_iter()
                        .collect::<BTreeSet<String>>(),
                ),
            );
        }
    }
    let mut accessible: Vec<AppInfo> = connectors
        .into_values()
        .map(|(mut connector, plugin_display_names)| {
            connector.plugin_display_names = plugin_display_names.into_iter().collect();
            connector.install_url = Some(connector_install_url(&connector.name, &connector.id));
            connector
        })
        .collect();
    accessible.sort_by(|left, right| {
        right
            .is_accessible
            .cmp(&left.is_accessible)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });
    accessible
}
