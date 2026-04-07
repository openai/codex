#![cfg(not(target_os = "windows"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::stdio_server_bin;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use tempfile::TempDir;
use wiremock::MockServer;

const SAMPLE_PLUGIN_CONFIG_NAME: &str = "sample@test";
const SAMPLE_PLUGIN_DISPLAY_NAME: &str = "sample";
const SAMPLE_PLUGIN_DESCRIPTION: &str = "inspect sample data";

fn sample_plugin_root(home: &TempDir) -> std::path::PathBuf {
    home.path().join("plugins/cache/test/sample/local")
}

fn write_sample_plugin_manifest_and_config(home: &TempDir) -> std::path::PathBuf {
    let plugin_root = sample_plugin_root(home);
    std::fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create plugin manifest dir");
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        format!(
            r#"{{"name":"{SAMPLE_PLUGIN_DISPLAY_NAME}","description":"{SAMPLE_PLUGIN_DESCRIPTION}"}}"#
        ),
    )
    .expect("write plugin manifest");
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            "[features]\nplugins = true\n\n[plugins.\"{SAMPLE_PLUGIN_CONFIG_NAME}\"]\nenabled = true\n"
        ),
    )
    .expect("write config");
    plugin_root
}

fn write_plugin_skill_plugin(home: &TempDir) -> std::path::PathBuf {
    let plugin_root = write_sample_plugin_manifest_and_config(home);
    let skill_dir = plugin_root.join("skills/sample-search");
    std::fs::create_dir_all(skill_dir.as_path()).expect("create plugin skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\ndescription: inspect sample data\n---\n\n# body\n",
    )
    .expect("write plugin skill");
    skill_dir.join("SKILL.md")
}

fn write_plugin_mcp_plugin(home: &TempDir, command: &str) {
    let plugin_root = write_sample_plugin_manifest_and_config(home);
    std::fs::write(
        plugin_root.join(".mcp.json"),
        format!(
            r#"{{
  "mcpServers": {{
    "sample": {{
      "command": "{command}"
    }}
  }}
}}"#
        ),
    )
    .expect("write plugin mcp config");
}

fn write_plugin_app_plugin(home: &TempDir) {
    let plugin_root = write_sample_plugin_manifest_and_config(home);
    std::fs::write(
        plugin_root.join(".app.json"),
        r#"{
  "apps": {
    "calendar": {
      "id": "calendar"
    }
  }
}"#,
    )
    .expect("write plugin app config");
}

async fn build_plugin_test_codex(
    server: &MockServer,
    codex_home: Arc<TempDir>,
) -> Result<Arc<codex_core::CodexThread>> {
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_auth(CodexAuth::from_api_key("Test API Key"));
    Ok(builder
        .build(server)
        .await
        .expect("create new conversation")
        .codex)
}

async fn build_analytics_plugin_test_codex(
    server: &MockServer,
    codex_home: Arc<TempDir>,
) -> Result<Arc<codex_core::CodexThread>> {
    let chatgpt_base_url = server.uri();
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model("gpt-5")
        .with_config(move |config| {
            config.chatgpt_base_url = chatgpt_base_url;
        });
    Ok(builder
        .build(server)
        .await
        .expect("create new conversation")
        .codex)
}

async fn build_apps_enabled_plugin_test_codex(
    server: &MockServer,
    codex_home: Arc<TempDir>,
    chatgpt_base_url: String,
) -> Result<Arc<codex_core::CodexThread>> {
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config
                .features
                .enable(Feature::Apps)
                .expect("test config should allow feature update");
            config.chatgpt_base_url = chatgpt_base_url;
        });
    Ok(builder
        .build(server)
        .await
        .expect("create new conversation")
        .codex)
}

fn tool_names(body: &serde_json::Value) -> Vec<String> {
    body.get("tools")
        .and_then(serde_json::Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("name")
                        .or_else(|| tool.get("type"))
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn tool_description(body: &serde_json::Value, tool_name: &str) -> Option<String> {
    body.get("tools")
        .and_then(serde_json::Value::as_array)
        .and_then(|tools| {
            tools.iter().find_map(|tool| {
                if tool.get("name").and_then(serde_json::Value::as_str) == Some(tool_name) {
                    tool.get("description")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                } else {
                    None
                }
            })
        })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn capability_sections_render_in_developer_message_in_order() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_with_connector_name(&server, "Google Calendar").await?;

    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let codex_home = Arc::new(TempDir::new()?);
    write_plugin_skill_plugin(codex_home.as_ref());
    write_plugin_app_plugin(codex_home.as_ref());
    let codex = build_apps_enabled_plugin_test_codex(
        &server,
        Arc::clone(&codex_home),
        apps_server.chatgpt_base_url,
    )
    .await?;

    codex
        .submit(Op::UserInput {
            items: vec![codex_protocol::user_input::UserInput::Text {
                text: "hello".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = resp_mock.single_request();
    let developer_messages = request.message_input_texts("developer");
    let developer_text = developer_messages.join("\n\n");
    let apps_pos = developer_text
        .find("## Apps")
        .expect("expected apps section in developer message");
    let skills_pos = developer_text
        .find("## Skills")
        .expect("expected skills section in developer message");
    let plugins_pos = developer_text
        .find("## Plugins")
        .expect("expected plugins section in developer message");
    assert!(
        apps_pos < skills_pos && skills_pos < plugins_pos,
        "expected Apps -> Skills -> Plugins order: {developer_messages:?}"
    );
    assert!(
        developer_text.contains("`sample`"),
        "expected enabled plugin name in developer message: {developer_messages:?}"
    );
    assert!(
        developer_text.contains("`sample`: inspect sample data"),
        "expected plugin description in developer message: {developer_messages:?}"
    );
    assert!(
        developer_text.contains("skill entries are prefixed with `plugin_name:`"),
        "expected plugin skill naming guidance in developer message: {developer_messages:?}"
    );
    assert!(
        developer_text.contains("sample:sample-search: inspect sample data"),
        "expected namespaced plugin skill summary in developer message: {developer_messages:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_plugin_mentions_inject_plugin_guidance() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount_with_connector_name(&server, "Google Calendar").await?;
    let mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let codex_home = Arc::new(TempDir::new()?);
    let rmcp_test_server_bin = match stdio_server_bin() {
        Ok(bin) => bin,
        Err(err) => {
            eprintln!("test_stdio_server binary not available, skipping test: {err}");
            return Ok(());
        }
    };
    write_plugin_skill_plugin(codex_home.as_ref());
    write_plugin_mcp_plugin(codex_home.as_ref(), &rmcp_test_server_bin);
    write_plugin_app_plugin(codex_home.as_ref());

    let codex =
        build_apps_enabled_plugin_test_codex(&server, codex_home, apps_server.chatgpt_base_url)
            .await?;

    codex
        .submit(Op::UserInput {
            items: vec![codex_protocol::user_input::UserInput::Mention {
                name: "sample".into(),
                path: format!("plugin://{SAMPLE_PLUGIN_CONFIG_NAME}"),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = mock.single_request();
    let developer_messages = request.message_input_texts("developer");
    assert!(
        developer_messages
            .iter()
            .any(|text| text.contains("Skills from this plugin")),
        "expected plugin skills guidance: {developer_messages:?}"
    );
    assert!(
        developer_messages
            .iter()
            .any(|text| text.contains("MCP servers from this plugin")),
        "expected visible plugin MCP guidance: {developer_messages:?}"
    );
    assert!(
        developer_messages
            .iter()
            .any(|text| text.contains("Apps from this plugin")),
        "expected visible plugin app guidance: {developer_messages:?}"
    );
    let request_body = request.body_json();
    let request_tools = tool_names(&request_body);
    assert!(
        request_tools
            .iter()
            .any(|name| name == "mcp__codex_apps__google_calendar_create_event"),
        "expected plugin app tools to become visible for this turn: {request_tools:?}"
    );
    let echo_description = tool_description(&request_body, "mcp__sample__echo")
        .expect("plugin MCP tool description should be present");
    assert!(
        echo_description.contains("This tool is part of plugin `sample`."),
        "expected plugin MCP provenance in tool description: {echo_description:?}"
    );
    let calendar_description = tool_description(
        &request_body,
        "mcp__codex_apps__google_calendar_create_event",
    )
    .expect("plugin app tool description should be present");
    assert!(
        calendar_description.contains("This tool is part of plugin `sample`."),
        "expected plugin app provenance in tool description: {calendar_description:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_plugin_mentions_track_plugin_used_analytics() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let _resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;

    let codex_home = Arc::new(TempDir::new()?);
    write_plugin_skill_plugin(codex_home.as_ref());
    let codex = build_analytics_plugin_test_codex(&server, codex_home).await?;

    codex
        .submit(Op::UserInput {
            items: vec![codex_protocol::user_input::UserInput::Mention {
                name: "sample".into(),
                path: format!("plugin://{SAMPLE_PLUGIN_CONFIG_NAME}"),
            }],
            final_output_json_schema: None,
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let deadline = Instant::now() + Duration::from_secs(10);
    let plugin_event = loop {
        let requests = server.received_requests().await.unwrap_or_default();
        if let Some(event) = requests
            .into_iter()
            .filter(|request| request.url.path() == "/codex/analytics-events/events")
            .find_map(|request| {
                let payload: serde_json::Value = serde_json::from_slice(&request.body).ok()?;
                payload["events"].as_array().and_then(|events| {
                    events
                        .iter()
                        .find(|event| event["event_type"] == "codex_plugin_used")
                        .cloned()
                })
            })
        {
            break event;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for plugin analytics request");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };

    let event = plugin_event;
    assert_eq!(event["event_params"]["plugin_id"], "sample@test");
    assert_eq!(event["event_params"]["plugin_name"], "sample");
    assert_eq!(event["event_params"]["marketplace_name"], "test");
    assert_eq!(event["event_params"]["has_skills"], true);
    assert_eq!(event["event_params"]["mcp_server_count"], 0);
    assert_eq!(
        event["event_params"]["connector_ids"],
        serde_json::json!([])
    );
    assert_eq!(
        event["event_params"]["product_client_id"],
        serde_json::json!(codex_login::default_client::originator().value)
    );
    assert_eq!(event["event_params"]["model_slug"], "gpt-5");
    assert!(event["event_params"]["thread_id"].as_str().is_some());
    assert!(event["event_params"]["turn_id"].as_str().is_some());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn plugin_mcp_tools_are_listed() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let codex_home = Arc::new(TempDir::new()?);
    let rmcp_test_server_bin = stdio_server_bin()?;
    write_plugin_mcp_plugin(codex_home.as_ref(), &rmcp_test_server_bin);
    let codex = build_plugin_test_codex(&server, codex_home).await?;

    let tools_ready_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        codex.submit(Op::ListMcpTools).await?;
        let list_event = wait_for_event_with_timeout(
            &codex,
            |ev| matches!(ev, EventMsg::McpListToolsResponse(_)),
            Duration::from_secs(10),
        )
        .await;
        let EventMsg::McpListToolsResponse(tool_list) = list_event else {
            unreachable!("event guard guarantees McpListToolsResponse");
        };
        if tool_list.tools.contains_key("mcp__sample__echo")
            && tool_list.tools.contains_key("mcp__sample__image")
        {
            break;
        }

        let available_tools: Vec<&str> = tool_list.tools.keys().map(String::as_str).collect();
        if Instant::now() >= tools_ready_deadline {
            panic!("timed out waiting for plugin MCP tools; discovered tools: {available_tools:?}");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    Ok(())
}
