use codex_plugin::AppConnectorId;
use codex_plugin::PluginCapabilitySummary;
use pretty_assertions::assert_eq;

use super::ConnectorSnapshot;

#[test]
fn snapshot_preserves_connector_order_and_dedupes_provenance() {
    let snapshot = ConnectorSnapshot::from_plugin_capability_summaries(&[
        summary("skills", "Skills only", &[]),
        summary("host", "Zulu", &["calendar", "calendar"]),
        summary("selected-a", "Alpha", &["drive", "calendar"]),
        summary("selected-b", "Alpha", &["calendar"]),
    ]);

    assert_eq!(
        snapshot.connector_ids(),
        &[
            AppConnectorId("calendar".to_string()),
            AppConnectorId("drive".to_string()),
        ]
    );
    assert_eq!(
        snapshot.plugin_display_names_for_connector_id("calendar"),
        &["Alpha".to_string(), "Zulu".to_string()]
    );
    assert_eq!(
        snapshot.plugin_display_names_for_connector_id("missing"),
        &[] as &[String]
    );
}

fn summary(id: &str, display_name: &str, connector_ids: &[&str]) -> PluginCapabilitySummary {
    PluginCapabilitySummary {
        config_name: id.to_string(),
        display_name: display_name.to_string(),
        app_connector_ids: connector_ids
            .iter()
            .map(|id| AppConnectorId((*id).to_string()))
            .collect(),
        ..Default::default()
    }
}
