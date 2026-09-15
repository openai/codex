//! Canonical connector ownership overrides other plugins' shared contributions.

use std::sync::Arc;

use anyhow::Result;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::SEARCH_CALENDAR_CREATE_TOOL;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::ev_tool_search_call;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::namespace_child_tool;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn canonical_plugin_disable_overrides_shared_connector_and_can_be_cleared() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let apps = AppsTestServer::mount_with_connector_name(&server, "Google Calendar").await?;
    Mock::given(method("GET"))
        .and(path("/ps/plugins/installed"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "plugins": [{
                "id": "plugins~Plugin_calendar",
                "name": "calendar",
                "scope": "GLOBAL",
                "status": "ENABLED",
                "installation_policy": "AVAILABLE",
                "authentication_policy": "ON_USE",
                "canonical_app_id": "calendar",
                "release": {
                    "version": "local",
                    "display_name": "Calendar",
                    "description": "Calendar connector",
                    "interface": {},
                },
                "enabled": true,
            }],
            "pagination": {"next_page_token": null},
        })))
        .mount(&server)
        .await;
    let home = Arc::new(TempDir::new()?);
    std::fs::write(
        home.path().join("config.toml"),
        "[features]\nplugins = true\nremote_plugin = true\n[plugins.\"shared@test\"]\nenabled = true\n",
    )?;
    for (marketplace, name) in [("test", "shared"), ("openai-curated-remote", "calendar")] {
        let root = home
            .path()
            .join(format!("plugins/cache/{marketplace}/{name}/local"));
        std::fs::create_dir_all(root.join(".codex-plugin"))?;
        std::fs::write(
            root.join(".codex-plugin/plugin.json"),
            format!(r#"{{"name":"{name}"}}"#),
        )?;
        std::fs::write(
            root.join(".app.json"),
            r#"{"apps":{"calendar":{"id":"calendar"}}}"#,
        )?;
    }
    let mut builder = test_codex()
        .with_home(home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config.features.enable(Feature::Apps).unwrap();
            config.chatgpt_base_url = apps.chatgpt_base_url;
        });
    let test = builder.build_with_auto_env(&server).await?;
    let manager = test.thread_manager.plugins_manager();
    let auth = test.thread_manager.auth_manager().auth().await;
    manager
        .reconcile_remote_installed_plugins(&test.config.plugins_config_input(), auth.as_ref())
        .await?;
    for (phase, (disabled, visible)) in [
        (vec!["shared@test"], true),
        (vec!["calendar@openai-curated-remote"], false),
        (vec![], true),
    ]
    .into_iter()
    .enumerate()
    {
        let call_id = format!("calendar-search-{phase}");
        let mock = mount_sse_sequence(
            &server,
            vec![
                sse(vec![
                    ev_response_created("search"),
                    ev_tool_search_call(
                        &call_id,
                        &serde_json::json!({"query":"create calendar event"}),
                    ),
                    ev_completed("search"),
                ]),
                sse(vec![ev_response_created("done"), ev_completed("done")]),
            ],
        )
        .await;
        test.codex
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: "Find the calendar tool.".to_string(),
                    text_elements: Vec::new(),
                }])
                .with_thread_settings(ThreadSettingsOverrides {
                    disabled_plugin_ids: Some(disabled.into_iter().map(str::to_string).collect()),
                    ..Default::default()
                }),
            )
            .await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        assert_eq!(
            namespace_child_tool(
                &mock.requests()[1].tool_search_output(&call_id),
                "mcp__codex_apps__google_calendar",
                SEARCH_CALENDAR_CREATE_TOOL,
            )
            .is_some(),
            visible
        );
    }
    Ok(())
}
