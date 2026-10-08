//! Native plugin hook admission must follow policy changes within an active turn.

use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use anyhow::Result;
use codex_config::ConfigLayerEntry;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_core::ConfigRefreshOutcome;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use wiremock::Mock;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::rmcp_client::remote_aware_environment_id;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_stop_hook_uses_plugin_policy_refreshed_during_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let observer = AppsTestServer::mount(&server).await?;
    Mock::given(method("POST"))
        .and(path("/api/codex/ps/mcp"))
        .and(body_partial_json(json!({ "method": "tools/call" })))
        .respond_with(|request: &Request| {
            let request: serde_json::Value =
                serde_json::from_slice(&request.body).expect("observer tool call");
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": { "content": [{ "type": "text", "text": "{}" }] },
            }))
        })
        .with_priority(/*p*/ 1)
        .mount(&server)
        .await;
    let home = Arc::new(TempDir::new()?);
    let plugin_root = home.path().join("plugins/cache/test/example/local");
    fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        serde_json::to_vec(&json!({
            "name": "example",
            "hooks": { "hooks": { "Stop": [{ "hooks": [{
                "type": "mcp_tool",
                "server": "hook_observer",
                "tool": "calendar_list_events",
            }] }] } },
        }))?,
    )?;
    fs::write(
        home.path().join("config.toml"),
        "[features]\nplugins = true\nhooks = true\n\n[plugins.\"example@test\"]\n",
    )?;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_assistant_message("baseline", "done"),
                ev_completed("baseline"),
            ]),
            sse(vec![
                ev_function_call(
                    "pause",
                    "request_user_input",
                    r#"{"questions":[{"id":"continue","header":"Continue","question":"Continue?","options":[{"label":"Yes","description":"Finish."},{"label":"No","description":"Stop."}]}]}"#,
                ),
                ev_completed("paused-step"),
            ]),
            sse(vec![
                ev_assistant_message("finished-turn", "done"),
                ev_completed("finished-turn"),
            ]),
        ],
    )
    .await;
    let test = test_codex()
        .with_home(home)
        .with_config(move |config| {
            config.bypass_hook_trust = true;
            config
                .features
                .enable(Feature::DefaultModeRequestUserInput)
                .expect("enable request_user_input");
            config
                .mcp_servers
                .set(HashMap::from([(
                    "hook_observer".to_string(),
                    serde_json::from_value(json!({
                        "url": format!("{}/api/codex/ps/mcp", observer.chatgpt_base_url),
                        "environment_id": remote_aware_environment_id(),
                    }))
                    .expect("observer MCP configuration"),
                )]))
                .expect("configure independent hook observer");
        })
        .build_with_auto_env(&server)
        .await?;
    wait_for_mcp_server(&test.codex, "hook_observer").await?;
    test.submit_text_turn("finish the enabled baseline turn")
        .await?;
    assert_eq!(recorded_apps_tool_calls(&server).await.len(), 1);

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Ask before finishing this turn".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let request = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    let current_config = test.codex.config().await;
    let mut next_config = (*current_config).clone();
    let mut layers = current_config
        .config_layer_stack
        .all_layers_low_to_high()
        .cloned()
        .collect::<Vec<_>>();
    layers.push(ConfigLayerEntry::new(
        ConfigLayerSource::System {
            file: current_config.codex_home.join("system-config.toml"),
        },
        toml::from_str("[plugins._default]\nenabled = false")?,
    ));
    layers.sort_by_key(|layer| layer.name.precedence());
    next_config.config_layer_stack = ConfigLayerStack::new(
        layers,
        current_config.config_layer_stack.requirements().clone(),
        current_config
            .config_layer_stack
            .requirements_toml()
            .clone(),
    )
    .expect("updated system layer");
    assert_eq!(
        test.codex
            .refresh_mcp_config(current_config, next_config)
            .await,
        ConfigRefreshOutcome::Published
    );
    test.codex
        .submit(Op::UserInputAnswer {
            id: request.turn_id,
            response: RequestUserInputResponse {
                answers: HashMap::from([(
                    "continue".to_string(),
                    RequestUserInputAnswer {
                        answers: vec!["Yes".to_string()],
                    },
                )]),
            },
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(responses.requests().len(), 3);
    assert_eq!(recorded_apps_tool_calls(&server).await.len(), 1);

    test.codex
        .call_mcp_tool(
            "hook_observer",
            "calendar_list_events",
            /*arguments*/ None,
            /*meta*/ None,
        )
        .await?;
    assert_eq!(recorded_apps_tool_calls(&server).await.len(), 2);
    Ok(())
}
