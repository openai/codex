use codex_app_server_protocol::AppInfo;
use pretty_assertions::assert_eq;

use super::ExplicitConnectorMentions;

#[test]
fn resolve_keeps_ids_and_unambiguous_plain_names() {
    let connectors = vec![connector("calendar-1", "Google Calendar")];
    let mut mentions = ExplicitConnectorMentions::default();
    mentions.insert_connector_id("direct");
    mentions.insert_plain_name("google-calendar");

    assert_eq!(
        mentions.resolve(&connectors),
        ["calendar-1".to_string(), "direct".to_string()]
            .into_iter()
            .collect()
    );
}

#[test]
fn resolve_skips_ambiguous_plain_names() {
    let connectors = vec![
        connector("calendar-1", "Google Calendar"),
        connector("calendar-2", "Google Calendar"),
    ];
    let mut mentions = ExplicitConnectorMentions::default();
    mentions.insert_plain_name("google-calendar");

    assert_eq!(mentions.resolve(&connectors), Default::default());
}

fn connector(id: &str, name: &str) -> AppInfo {
    AppInfo {
        id: id.to_string(),
        name: name.to_string(),
        description: None,
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
    }
}
