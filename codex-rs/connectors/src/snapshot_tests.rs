use codex_plugin::AppConnectorId;
use pretty_assertions::assert_eq;

use super::ConnectorSnapshot;
use super::PluginConnectorSource;

#[test]
fn snapshot_preserves_connector_order_and_dedupes_provenance() {
    let snapshot = ConnectorSnapshot::from_plugin_sources([
        source("plugin-a", "Zulu", &["calendar", "drive", "calendar"]),
        source("plugin-b", "Alpha", &["calendar"]),
        source("plugin-c", "Alpha", &["calendar"]),
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

#[test]
fn merged_snapshot_keeps_first_seen_connector_order() {
    let host =
        ConnectorSnapshot::from_plugin_sources([source("host", "Host plugin", &["calendar"])]);
    let selected = ConnectorSnapshot::from_plugin_sources([source(
        "selected",
        "Selected plugin",
        &["drive", "calendar"],
    )]);

    let merged = host.merged_with(&selected);

    assert_eq!(
        merged.connector_ids(),
        &[
            AppConnectorId("calendar".to_string()),
            AppConnectorId("drive".to_string()),
        ]
    );
    assert_eq!(
        merged.plugin_display_names_for_connector_id("calendar"),
        &["Host plugin".to_string(), "Selected plugin".to_string()]
    );
}

#[test]
fn snapshot_drops_sources_without_connectors() {
    let calendar = source("calendar", "Calendar plugin", &["calendar"]);
    let snapshot = ConnectorSnapshot::from_plugin_sources([
        source("skills", "Skills only", &[]),
        calendar.clone(),
    ]);

    assert_eq!(snapshot.plugin_sources(), &[calendar]);
}

fn source(id: &str, display_name: &str, connector_ids: &[&str]) -> PluginConnectorSource {
    PluginConnectorSource::from_connector_ids(
        id,
        display_name,
        connector_ids
            .iter()
            .map(|id| AppConnectorId((*id).to_string())),
    )
}
