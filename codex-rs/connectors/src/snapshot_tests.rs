use codex_plugin::AppConnectorId;
use pretty_assertions::assert_eq;

use super::ConnectorSnapshot;
use super::PluginConnectorSource;

#[test]
fn snapshot_merges_sources_in_order_and_dedupes_provenance() {
    let host = [
        source("skills", "Skills only", &[]),
        source("host", "Zulu", &["calendar", "calendar"]),
    ];
    let selected = [
        source("selected-a", "Alpha", &["drive", "calendar"]),
        source("selected-b", "Alpha", &["calendar"]),
    ];

    let merged = ConnectorSnapshot::from_plugin_sources(host.into_iter().chain(selected), &[]);

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

#[test]
fn disabled_plugins_preserve_shared_connectors() {
    let sources = [
        source("alpha", "Alpha", &["exclusive", "shared"]),
        source("beta", "Beta", &["other", "shared"]),
    ];
    let filtered = ConnectorSnapshot::from_plugin_sources(sources.clone(), &["alpha".to_string()]);

    let expected = ConnectorSnapshot {
        disabled_connector_ids: std::collections::HashSet::from(["exclusive".to_string()]),
        ..ConnectorSnapshot::from_plugin_sources(
            [source("beta", "Beta", &["other", "shared"])],
            &[],
        )
    };
    assert_eq!(filtered, expected);
    assert_eq!(
        ConnectorSnapshot::from_plugin_sources(
            sources.iter().rev().cloned(),
            &["alpha".to_string()],
        ),
        expected
    );
    assert_eq!(
        ConnectorSnapshot::from_plugin_sources(sources, &["alpha".to_string(), "beta".to_string()],),
        ConnectorSnapshot {
            disabled_connector_ids: std::collections::HashSet::from([
                "exclusive".to_string(),
                "other".to_string(),
                "shared".to_string(),
            ]),
            ..ConnectorSnapshot::default()
        }
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
