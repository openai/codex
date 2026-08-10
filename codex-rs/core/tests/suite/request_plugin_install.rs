#![allow(clippy::unwrap_used)]

use anyhow::Context;
use anyhow::Result;
use codex_config::types::ToolSuggestDisabledTool;
use codex_config::types::ToolSuggestDiscoverable;
use codex_config::types::ToolSuggestDiscoverableType;
use codex_core::config::Config;
use codex_core_plugins::startup_sync::curated_plugins_repo_path;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use codex_protocol::approvals::ElicitationAction;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use codex_utils_path_uri::PathUri;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_wine_exec;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use tempfile::TempDir;
use test_case::test_case;
use wiremock::Mock;
use wiremock::MockGuard;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

use super::rmcp_client::remote_aware_environment_id;
use super::rmcp_client::remote_aware_stdio_server_bin;

const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
const LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME: &str = "list_available_plugins_to_install";
const REQUEST_PLUGIN_INSTALL_TOOL_NAME: &str = "request_plugin_install";
const DISCOVERABLE_GMAIL_ID: &str = "connector_68df038e0ba48191908c8434991bbac2";
const REMOTE_CALENDAR_PLUGIN_CONFIG_ID: &str = "calendar@openai-curated-remote";
const REMOTE_CALENDAR_PLUGIN_ID: &str = "plugin_calendar";
const CALENDAR_CONNECTOR_ID: &str = "calendar";
const CALENDAR_NAMESPACE: &str = "mcp__codex_apps__calendar";
const CALENDAR_CREATE_EVENT_TOOL: &str = "_create_event";
const STEP_PREPARATION_MCP_SERVER: &str = "step_preparation";

fn tool_names(body: &Value) -> Vec<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| {
                    tool.get("name")
                        .or_else(|| tool.get("type"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn configure_apps_without_search_tool(config: &mut Config, apps_base_url: &str) {
    for feature in [
        Feature::Apps,
        Feature::Plugins,
        Feature::RemotePlugin,
        Feature::ToolSuggest,
    ] {
        config
            .features
            .enable(feature)
            .expect("test config should allow feature update");
    }
    let mut model_catalog = bundled_models_response()
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"));
    let model = model_catalog
        .models
        .iter_mut()
        .find(|model| model.slug == "gpt-5.4")
        .expect("gpt-5.4 exists in bundled models.json");
    config.chatgpt_base_url = apps_base_url.to_string();
    config.model = Some("gpt-5.4".to_string());
    config.tool_suggest.discoverables = vec![ToolSuggestDiscoverable {
        kind: ToolSuggestDiscoverableType::Connector,
        id: DISCOVERABLE_GMAIL_ID.to_string(),
    }];
    model.supports_search_tool = false;
    config.model_catalog = Some(model_catalog);
}

async fn mount_recommendations(server: &wiremock::MockServer, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/ps/plugins/suggested"))
        .and(query_param("scope", "GLOBAL"))
        .respond_with(response)
        .mount(server)
        .await;
}

fn assert_legacy_tools(body: &Value) {
    let tools = tool_names(body);
    assert!(!tools.iter().any(|name| name == TOOL_SEARCH_TOOL_NAME));
    assert!(
        tools
            .iter()
            .any(|name| name == LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME),
        "legacy mode should expose {LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME}: {tools:?}"
    );
    assert!(
        tools
            .iter()
            .any(|name| name == REQUEST_PLUGIN_INSTALL_TOOL_NAME),
        "legacy mode should expose {REQUEST_PLUGIN_INSTALL_TOOL_NAME}: {tools:?}"
    );
}

async fn build_test(
    server: &wiremock::MockServer,
    apps_server: &AppsTestServer,
) -> Result<TestCodex> {
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config({
            let apps_base_url = apps_server.chatgpt_base_url.clone();
            move |config| {
                config
                    .permissions
                    .set_permission_profile(PermissionProfile::Disabled)
                    .expect("test config should allow disabled permissions");
                configure_apps_without_search_tool(config, apps_base_url.as_str());
            }
        });
    builder.build_with_auto_env(server).await
}

async fn build_gated_step_preparation_test(
    server: &MockServer,
    apps_server: &AppsTestServer,
) -> Result<TestCodex> {
    let command = remote_aware_stdio_server_bin()?;
    let environment_id = remote_aware_environment_id();
    let apps_base_url = apps_server.chatgpt_base_url.clone();
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| {
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .expect("test config should allow disabled permissions");
            configure_apps_without_search_tool(config, apps_base_url.as_str());

            let barrier_file = config.cwd.join("allow-step-preparation-initialize");
            let pid_file = config.cwd.join("step-preparation.pid");
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                STEP_PREPARATION_MCP_SERVER.to_string(),
                serde_json::from_value(json!({
                    "command": command.clone(),
                    "environment_id": environment_id.clone(),
                    "env": {
                        "MCP_TEST_INITIALIZE_BARRIER_FILE": barrier_file,
                        "MCP_TEST_PID_FILE": pid_file,
                    },
                    "enabled_tools": ["echo"],
                    "startup_timeout_sec": 10,
                }))
                .expect("test MCP server configuration"),
            );
            config
                .mcp_servers
                .set(servers)
                .expect("test config should allow MCP servers");
        });
    builder.build_with_auto_env(server).await
}

async fn start_gated_step_preparation(test: &TestCodex, server: &MockServer) -> Result<PathUri> {
    let prior_recommendation_count = server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|request| request.url.path() == "/ps/plugins/suggested")
        .count();
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "prepare MCP and plugin recommendations".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                ..Default::default()
            },
        })
        .await?;

    let fs = test.fs();
    let pid_file = PathUri::from_host_native_path(test.config.cwd.join("step-preparation.pid"))?;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let mcp_started = fs
                .read_file_text(&pid_file, /*sandbox*/ None)
                .await
                .is_ok_and(|pid| !pid.trim().is_empty());
            let recommendation_count = server
                .received_requests()
                .await
                .unwrap_or_default()
                .into_iter()
                .filter(|request| request.url.path() == "/ps/plugins/suggested")
                .count();
            if mcp_started && recommendation_count > prior_recommendation_count {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .context("MCP startup and plugin recommendations should begin before MCP is released")?;

    PathUri::from_host_native_path(test.config.cwd.join("allow-step-preparation-initialize"))
        .map_err(Into::into)
}

async fn start_install_turn(test: &TestCodex, prompt: &str) -> Result<ElicitationRequestEvent> {
    let (sandbox_policy, permission_profile) =
        turn_permission_fields(PermissionProfile::Disabled, test.config.cwd.as_path());
    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: test.session_configured.model.clone(),
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            },
        })
        .await?;

    Ok(wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ElicitationRequest(request) => Some(request.clone()),
        _ => None,
    })
    .await)
}

async fn resolve_install_elicitation(
    test: &TestCodex,
    elicitation: ElicitationRequestEvent,
    decision: ElicitationAction,
) -> Result<()> {
    test.codex
        .submit(Op::ResolveElicitation {
            server_name: elicitation.server_name,
            request_id: elicitation.id,
            decision,
            content: None,
            meta: None,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    Ok(())
}

async fn mount_remote_calendar_recommendation(server: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/ps/plugins/suggested"))
        .and(query_param("scope", "GLOBAL"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "enabled": true,
            "plugins": [{
                "id": REMOTE_CALENDAR_PLUGIN_ID,
                "name": "calendar",
                "status": "ENABLED",
                "installation_policy": "AVAILABLE",
                "release": {
                    "display_name": "Calendar",
                    "app_ids": [CALENDAR_CONNECTOR_ID]
                }
            }]
        })))
        .expect(1)
        .mount(server)
        .await;
}

fn remote_installed_plugins_response(plugins: Vec<Value>) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "plugins": plugins,
        "pagination": {
            "next_page_token": null
        }
    }))
}

async fn mount_empty_remote_installed_plugins(server: &wiremock::MockServer) -> MockGuard {
    Mock::given(method("GET"))
        .and(path("/ps/plugins/installed"))
        .respond_with(remote_installed_plugins_response(Vec::new()))
        .mount_as_scoped(server)
        .await
}

async fn mount_remote_calendar_installed_plugins(server: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/ps/plugins/installed"))
        .respond_with(remote_installed_plugins_response(vec![json!({
            "id": REMOTE_CALENDAR_PLUGIN_ID,
            "name": "calendar",
            "scope": "GLOBAL",
            "status": "ENABLED",
            "installation_policy": "AVAILABLE",
            "authentication_policy": "ON_USE",
            "release": {
                "display_name": "Calendar",
                "description": "Manage calendar events.",
                "interface": {}
            },
            "enabled": true
        })]))
        .with_priority(1)
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_discovery_overlaps_endpoint_plugin_recommendations() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    mount_recommendations(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "enabled": true,
            "plugins": [{
                "id": "plugin_github",
                "name": "github",
                "status": "ENABLED",
                "installation_policy": "AVAILABLE",
                "release": {"display_name": "GitHub"}
            }]
        })),
    )
    .await;
    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("concurrent-endpoint"),
            ev_assistant_message("endpoint-message", "done"),
            ev_completed("concurrent-endpoint"),
        ]),
    )
    .await;
    let test = build_gated_step_preparation_test(&server, &apps_server).await?;

    let barrier = start_gated_step_preparation(&test, &server).await?;
    assert!(
        response.requests().is_empty(),
        "sampling should wait for the complete MCP catalog"
    );
    test.fs()
        .write_file(&barrier, b"ready".to_vec(), /*sandbox*/ None)
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = response.single_request();
    assert!(
        request
            .message_input_texts("user")
            .join("\n")
            .contains("github@openai-curated-remote"),
        "the completed request should preserve endpoint recommendations"
    );
    assert!(
        request
            .tool_by_name("mcp__step_preparation", "echo")
            .is_some(),
        "the completed request should expose the live gated MCP tool"
    );

    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupting_concurrent_step_preparation_prevents_sampling() -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "requires a Windows test_stdio_server in the Wine-exec environment"
    );
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    mount_recommendations(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({"enabled": true, "plugins": []})),
    )
    .await;
    let response = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("cancelled-step-preparation"),
            ev_assistant_message("cancelled-message", "unexpected"),
            ev_completed("cancelled-step-preparation"),
        ]),
    )
    .await;
    let test = build_gated_step_preparation_test(&server, &apps_server).await?;

    let barrier = start_gated_step_preparation(&test, &server).await?;
    assert!(response.requests().is_empty());
    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;
    assert!(
        response.requests().is_empty(),
        "cancelling concurrent step preparation must prevent model sampling"
    );

    test.fs()
        .write_file(&barrier, b"ready".to_vec(), /*sandbox*/ None)
        .await?;
    test.codex.shutdown_and_wait().await?;
    assert!(
        response.requests().is_empty(),
        "releasing the cancelled MCP startup must not revive the aborted turn"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_false_preserves_legacy_workflow() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    mount_recommendations(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({"enabled": false, "plugins": []})),
    )
    .await;
    let call_id = "list-installable-tools";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME, "{}"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let test = build_test(&server, &apps_server).await?;
    test.submit_turn_with_approval_and_permission_profile(
        "list tools",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let request = &requests[0];
    assert!(
        !request
            .message_input_texts("user")
            .join("\n")
            .contains("<recommended_plugins>")
    );
    assert_legacy_tools(&request.body_json());
    let output = requests[1]
        .function_call_output_text(call_id)
        .expect("list tool output");
    let output: Value = serde_json::from_str(&output)?;
    assert!(output["tools"].as_array().is_some_and(|tools| {
        tools
            .iter()
            .any(|tool| tool["id"] == DISCOVERABLE_GMAIL_ID && tool["tool_type"] == "connector")
    }));
    Ok(())
}

#[test_case(true; "enabled plugin skill")]
#[test_case(false; "disabled plugin skill")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_plugin_skill_availability_reaches_tool_suggestion_candidates(
    skill_enabled: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    let codex_home = Arc::new(TempDir::new()?);
    let curated_root = curated_plugins_repo_path(codex_home.path());
    let plugin_root = curated_root.join("plugins/sample");
    std::fs::create_dir_all(curated_root.join(".agents/plugins"))?;
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::create_dir_all(plugin_root.join("skills/search"))?;
    std::fs::write(
        curated_root.join(".agents/plugins/marketplace.json"),
        r#"{
  "name": "openai-curated",
  "plugins": [{
    "name": "sample",
    "source": {"source": "local", "path": "./plugins/sample"}
  }]
}"#,
    )?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample","description":"Search sample data"}"#,
    )?;
    std::fs::write(
        plugin_root.join("skills/search/SKILL.md"),
        "---\nname: search\ndescription: Search sample data\n---\n",
    )?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            "[features]\nplugins = true\n\n[[skills.config]]\nname = \"sample:search\"\nenabled = {skill_enabled}\n"
        ),
    )?;

    let call_id = "list-skill-backed-plugin-candidates";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME, "{}"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex()
        .with_home(codex_home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config({
            let apps_base_url = apps_server.chatgpt_base_url.clone();
            move |config| {
                config
                    .permissions
                    .set_permission_profile(PermissionProfile::Disabled)
                    .expect("test config should allow disabled permissions");
                configure_apps_without_search_tool(config, apps_base_url.as_str());
                config
                    .features
                    .disable(Feature::RemotePlugin)
                    .expect("test config should allow local plugin suggestions");
                config.tool_suggest.discoverables = vec![ToolSuggestDiscoverable {
                    kind: ToolSuggestDiscoverableType::Plugin,
                    id: "sample@openai-curated".to_string(),
                }];
            }
        });
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_turn_with_approval_and_permission_profile(
        "list available plugins",
        AskForApproval::Never,
        PermissionProfile::Disabled,
    )
    .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert_legacy_tools(&requests[0].body_json());
    let output: Value = serde_json::from_str(
        &requests[1]
            .function_call_output_text(call_id)
            .expect("list tool output"),
    )?;
    assert_eq!(
        output,
        json!({
            "tools": [{
                "id": "sample@openai-curated",
                "name": "sample",
                "description": "Search sample data",
                "tool_type": "plugin",
                "has_skills": skill_enabled,
                "mcp_server_names": [],
                "app_connector_ids": []
            }]
        })
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn endpoint_mode_injects_candidates_hides_list_and_rejects_invented_ids() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    mount_recommendations(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "enabled": true,
            "plugins": [
                {
                    "id": "plugin_google_calendar",
                    "name": "google-calendar",
                    "status": "ENABLED",
                    "installation_policy": "AVAILABLE",
                    "release": {"display_name": "Google Calendar"}
                },
                {
                    "id": "plugin_github",
                    "name": "github",
                    "status": "ENABLED",
                    "installation_policy": "AVAILABLE",
                    "release": {"display_name": "GitHub"}
                }
            ]
        })),
    )
    .await;
    let call_id = "invented-plugin";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    REQUEST_PLUGIN_INSTALL_TOOL_NAME,
                    &serde_json::to_string(&json!({
                        "plugin_id": "invented@openai-curated-remote",
                        "suggest_reason": "Try this"
                    }))?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let test = build_test(&server, &apps_server).await?;

    test.submit_turn("suggest a plugin").await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let contextual_user_message = requests[0].message_input_texts("user").join("\n");
    assert!(contextual_user_message.contains("<recommended_plugins>"));
    assert!(contextual_user_message.contains("github@openai-curated-remote"));
    assert!(contextual_user_message.contains("google-calendar@openai-curated-remote"));
    let body = requests[0].body_json();
    let tools = tool_names(&body);
    assert!(
        !tools
            .iter()
            .any(|name| name == LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME)
    );
    assert!(
        tools
            .iter()
            .any(|name| name == REQUEST_PLUGIN_INSTALL_TOOL_NAME)
    );
    let output = requests[1]
        .function_call_output_text(call_id)
        .expect("request tool output");
    assert!(output.contains("<recommended_plugins> list"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn endpoint_recommendation_adds_install_identity_only_to_elicitation_metadata() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    run_remote_plugin_install_metadata_case().await
}

async fn run_remote_plugin_install_metadata_case() -> Result<()> {
    const REMOTE_PLUGIN_ID: &str = "plugin_connector_github";
    const APP_CONNECTOR_ID: &str = "connector_github";

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    mount_recommendations(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "enabled": true,
            "plugins": [{
                "id": REMOTE_PLUGIN_ID,
                "name": "github",
                "status": "ENABLED",
                "installation_policy": "AVAILABLE",
                "release": {
                    "display_name": "GitHub",
                    "app_ids": [APP_CONNECTOR_ID]
                }
            }]
        })),
    )
    .await;
    let call_id = "install-github";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    REQUEST_PLUGIN_INSTALL_TOOL_NAME,
                    &serde_json::to_string(&json!({
                        "plugin_id": "github@openai-curated-remote",
                        "suggest_reason": "Use GitHub for this request"
                    }))?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let test = build_test(&server, &apps_server).await?;
    let elicitation = start_install_turn(&test, "use GitHub").await?;
    let ElicitationRequest::Form {
        meta: Some(meta), ..
    } = &elicitation.request
    else {
        panic!("expected form elicitation metadata");
    };
    assert_eq!(meta["remote_plugin_id"], REMOTE_PLUGIN_ID);
    assert_eq!(meta["app_connector_ids"], json!([APP_CONNECTOR_ID]));

    let deadline = Instant::now() + Duration::from_secs(10);
    let analytics_event = loop {
        let requests = server.received_requests().await.unwrap_or_default();
        if let Some(event) = requests
            .into_iter()
            .filter(|request| request.url.path() == "/codex/analytics-events/events")
            .find_map(|request| {
                let payload: Value = serde_json::from_slice(&request.body).ok()?;
                payload["events"].as_array().and_then(|events| {
                    events
                        .iter()
                        .find(|event| event["event_type"] == "codex_plugin_install_requested")
                        .cloned()
                })
            })
        {
            break event;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for plugin install request analytics");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    let thread_id = analytics_event["event_params"]["thread_id"].clone();
    let turn_id = analytics_event["event_params"]["turn_id"].clone();
    assert_eq!(
        analytics_event,
        json!({
            "event_type": "codex_plugin_install_requested",
            "event_params": {
                "suggestion_id": "request_plugin_install_install-github",
                "plugins": [{
                    "plugin_id": "github@openai-curated-remote",
                    "remote_plugin_id": REMOTE_PLUGIN_ID,
                    "plugin_name": "GitHub",
                    "connector_ids": [APP_CONNECTOR_ID],
                }],
                "source": "endpoint_recommendation",
                "thread_id": thread_id,
                "turn_id": turn_id,
                "model_slug": "gpt-5.4",
                "product_client_id": codex_login::default_client::originator().value,
            }
        })
    );

    resolve_install_elicitation(&test, elicitation, ElicitationAction::Decline).await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    for request in requests {
        let body = request.body_json().to_string();
        assert!(!body.contains(REMOTE_PLUGIN_ID));
        assert!(!body.contains(APP_CONNECTOR_ID));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum RefreshedAppsTools {
    Available,
    Missing,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_plugin_install_refreshes_plugin_and_apps_tool_caches() -> Result<()> {
    skip_if_no_network!(Ok(()));

    run_remote_plugin_install_refresh_case(RefreshedAppsTools::Available).await?;
    run_remote_plugin_install_refresh_case(RefreshedAppsTools::Missing).await
}

async fn run_remote_plugin_install_refresh_case(refreshed_tools: RefreshedAppsTools) -> Result<()> {
    let server = start_mock_server().await;
    let tools_available = Arc::new(AtomicBool::new(false));
    let apps_server = match refreshed_tools {
        RefreshedAppsTools::Available => {
            AppsTestServer::mount_with_tools_available_when(&server, Arc::clone(&tools_available))
                .await?
        }
        RefreshedAppsTools::Missing => AppsTestServer::mount_without_tools(&server).await?,
    };
    mount_remote_calendar_recommendation(&server).await;
    let initial_remote_installed_plugins = mount_empty_remote_installed_plugins(&server).await;

    let install_call_id = "install-calendar";
    let suggest_reason = "Use Calendar for this request";
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    install_call_id,
                    REQUEST_PLUGIN_INSTALL_TOOL_NAME,
                    &serde_json::to_string(&json!({
                        "plugin_id": REMOTE_CALENDAR_PLUGIN_CONFIG_ID,
                        "suggest_reason": suggest_reason
                    }))?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-2", "catalog still current"),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;
    let test = build_test(&server, &apps_server).await?;

    let elicitation = start_install_turn(&test, "use Calendar").await?;
    mount_remote_calendar_installed_plugins(&server).await;
    drop(initial_remote_installed_plugins);
    tools_available.store(
        matches!(refreshed_tools, RefreshedAppsTools::Available),
        Ordering::SeqCst,
    );
    resolve_install_elicitation(&test, elicitation, ElicitationAction::Accept).await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0]
            .tool_by_name(CALENDAR_NAMESPACE, CALENDAR_CREATE_EVENT_TOOL)
            .is_none(),
        "calendar tool should be absent before the remote install"
    );
    let completed = matches!(refreshed_tools, RefreshedAppsTools::Available);
    assert_eq!(
        serde_json::from_str::<Value>(
            &requests[1]
                .function_call_output_text(install_call_id)
                .expect("install tool output")
        )?,
        json!({
            "completed": completed,
            "user_confirmed": true,
            "tool_type": "plugin",
            "action_type": "install",
            "tool_id": REMOTE_CALENDAR_PLUGIN_CONFIG_ID,
            "tool_name": "Calendar",
            "suggest_reason": suggest_reason
        })
    );
    assert_eq!(
        requests[1]
            .tool_by_name(CALENDAR_NAMESPACE, CALENDAR_CREATE_EVENT_TOOL)
            .is_some(),
        completed,
        "the resumed router should reflect the refreshed Apps tools"
    );
    assert!(
        !tool_names(&requests[1].body_json())
            .iter()
            .any(|name| name == REQUEST_PLUGIN_INSTALL_TOOL_NAME),
        "the refreshed installed-plugin cache should filter the cached recommendation"
    );
    drop(requests);
    test.codex.refresh_runtime_config(test.config.clone()).await;
    test.submit_turn("check whether Calendar is still installed")
        .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[2]
            .tool_by_name(CALENDAR_NAMESPACE, CALENDAR_CREATE_EVENT_TOOL)
            .is_some(),
        completed,
        "an unrelated runtime publication must retain the refreshed Apps catalog"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn endpoint_mode_with_no_eligible_candidates_exposes_no_suggestion_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    mount_recommendations(
        &server,
        ResponseTemplate::new(200).set_body_json(json!({
            "enabled": true,
            "plugins": [{
                "id": "plugin_google_calendar",
                "name": "google-calendar",
                "release": {"display_name": "Google Calendar"}
            }]
        })),
    )
    .await;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let mut builder = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config({
            let apps_base_url = apps_server.chatgpt_base_url.clone();
            move |config| {
                configure_apps_without_search_tool(config, apps_base_url.as_str());
                config.tool_suggest.disabled_tools = vec![ToolSuggestDisabledTool::plugin(
                    "google-calendar@openai-curated-remote",
                )];
            }
        });
    let test = builder.build_with_auto_env(&server).await?;

    test.submit_turn("list tools").await?;

    let request = mock.single_request();
    assert!(
        !request
            .message_input_texts("user")
            .join("\n")
            .contains("<recommended_plugins>")
    );
    let tools = tool_names(&request.body_json());
    assert!(
        !tools
            .iter()
            .any(|name| name == LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME)
    );
    assert!(
        !tools
            .iter()
            .any(|name| name == REQUEST_PLUGIN_INSTALL_TOOL_NAME)
    );
    Ok(())
}
