use codex_plugin::AppConnectorId;
use pretty_assertions::assert_eq;

use super::ConnectorSnapshot;
use super::PluginConnectorSource;

#[test]
fn snapshot_merges_sources_in_order_and_dedupes_provenance() {
    let host_source = source("host", "Zulu", &["calendar", "calendar"]);
    let host = ConnectorSnapshot::from_plugin_sources([
        source("skills", "Skills only", &[]),
        host_source.clone(),
    ]);
    let selected = ConnectorSnapshot::from_plugin_sources([
        source("selected-a", "Alpha", &["drive", "calendar"]),
        source("selected-b", "Alpha", &["calendar"]),
    ]);

    let merged = host.merged_with(&selected);

    assert_eq!(host.sources, vec![host_source]);
    assert_eq!(
        merged.connector_ids(),
        &[
            AppConnectorId("calendar".to_string()),
            AppConnectorId("drive".to_string()),
        ]
    );
    assert_eq!(
        merged.plugin_display_names_for_connector_id("calendar"),
        &["Alpha".to_string(), "Zulu".to_string()]
    );
    assert_eq!(
        merged.plugin_display_names_for_connector_id("missing"),
        &[] as &[String]
    );
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
