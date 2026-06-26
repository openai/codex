use super::*;
use pretty_assertions::assert_eq;

#[test]
fn filter_request_plugin_install_candidates_omits_plugins_for_codex_tui() {
    let plugin = DiscoverablePluginInfo {
        id: "slack@openai-curated".to_string(),
        remote_plugin_id: None,
        name: "Slack".to_string(),
        description: Some("Search Slack messages".to_string()),
        has_skills: true,
        mcp_server_names: vec!["slack".to_string()],
        app_connector_ids: vec!["connector_slack".to_string()],
    };

    assert_eq!(
        filter_request_plugin_install_candidates_for_client(
            vec![plugin.clone()],
            /*app_server_client_name*/ None,
        ),
        vec![plugin.clone()]
    );
    assert_eq!(
        filter_request_plugin_install_candidates_for_client(vec![plugin], Some("codex-tui")),
        Vec::new()
    );
}
