use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::CapabilityRootLocation;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::McpServerToolCallParams;
use codex_app_server_protocol::McpServerToolCallResponse;
use codex_app_server_protocol::SelectedCapabilityRoot;
use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use codex_features::Feature;
use codex_utils_path_uri::PathUri;
use core_test_support::responses;
use core_test_support::stdio_server_bin;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use test_case::test_case;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[derive(Clone, Copy)]
enum AccessCall {
    Installed,
    Selected,
    Unowned,
    ApiKey,
    Unlisted,
}

#[test_case(AccessCall::Installed; "installed direct call")]
#[test_case(AccessCall::Selected; "selected model and direct calls")]
#[test_case(AccessCall::Unowned; "unowned direct call")]
#[test_case(AccessCall::ApiKey; "api key direct call")]
#[test_case(AccessCall::Unlisted; "unlisted direct call")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daybreak_access_respects_plugin_provenance(case: AccessCall) -> Result<()> {
    let server = responses::start_mock_server().await;
    let home = TempDir::new()?;
    let selected_package = TempDir::new()?;
    let selected = matches!(case, AccessCall::Selected);
    let plugin_root = if selected {
        selected_package.path().to_path_buf()
    } else {
        home.path().join("plugins/cache/test/sample/local")
    };
    std::fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
    std::fs::write(
        plugin_root.join(".codex-plugin/plugin.json"),
        r#"{"name":"sample"}"#,
    )?;
    let mut mcp_config = json!({
        "command":stdio_server_bin()?, "cwd":plugin_root, "environment_id":"local",
        "default_tools_approval_mode":"approve", "env":{"MCP_TEST_DAYBREAK_READ_ONLY":"true"}
    });
    if matches!(case, AccessCall::Unlisted) {
        mcp_config["env"] = json!({});
    }
    std::fs::write(
        plugin_root.join(".mcp.json"),
        serde_json::to_vec(&json!({"mcpServers":{"sample":mcp_config}}))?,
    )?;
    let extra_config = match case {
        AccessCall::Installed | AccessCall::ApiKey | AccessCall::Unlisted => {
            "[plugins.\"sample@test\"]\nenabled = true\n".to_string()
        }
        AccessCall::Selected => String::new(),
        AccessCall::Unowned => toml::to_string(&json!({"mcp_servers":{"sample":mcp_config}}))?,
    };
    MockResponsesConfig::new(&server.uri())
        .enable_feature(Feature::Plugins)
        .disable_feature(Feature::Apps)
        .with_root_config(&format!(
            "chatgpt_base_url = \"{}\"\ncli_auth_credentials_store = \"file\"",
            server.uri()
        ))
        .with_extra_config(&extra_config)
        .write(home.path())?;
    let api_key = matches!(case, AccessCall::ApiKey).then_some("daybreak-test-api-key");
    if api_key.is_none() {
        write_chatgpt_auth(
            home.path(),
            ChatGptAuthFixture::new("daybreak-test-token").account_id("daybreak-account"),
            AuthCredentialsStoreMode::File,
        )?;
    }
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", api_key)])
        // Daybreak metadata is local-host-only, even when the test runner provides a remote executor.
        .without_auto_env()
        .build_initialized_with_timeout(std::time::Duration::from_secs(/*secs*/ 30))
        .await?;
    let selected_capability_roots = if selected {
        Some(vec![SelectedCapabilityRoot {
            id: "selected-daybreak".to_string(),
            location: CapabilityRootLocation::Environment {
                environment_id: "local".to_string(),
                path: PathUri::from_host_native_path(&plugin_root)?,
            },
        }])
    } else {
        None
    };
    let response: ThreadStartResponse = app
        .request(|request_id| ClientRequest::ThreadStart {
            request_id,
            params: ThreadStartParams {
                selected_capability_roots,
                ..Default::default()
            },
        })
        .await?;
    let thread = response.thread;
    Mock::given(method("GET"))
        .and(path("/accounts/verified_access"))
        .and(header("authorization", "Bearer daybreak-test-token"))
        .and(header("chatgpt-account-id", "daybreak-account"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "programs": [{"program":"cyber", "state":"active", "grants":[
                {"level":"tac1", "source":"individual"}
            ]}]
        })))
        .expect(match case {
            AccessCall::Selected => 1,
            AccessCall::Installed
            | AccessCall::Unowned
            | AccessCall::ApiKey
            | AccessCall::Unlisted => 0,
        })
        .mount(&server)
        .await;
    let expected = json!({
        "schemaVersion":1, "entitlements":{"cyber_trusted_access":{
            "schemaVersion":1,"status":"granted","grants":[{"level":"tac1","source":"user"}],"stale":false
        }}
    });
    let mut results = Vec::new();
    // Selected capabilities are materialized by the first turn, as in the executor MCP tests.
    if selected {
        let _responses = responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse(vec![
                    responses::ev_tool_search_call(
                        "search",
                        &json!({"query":"get_codex_security_daybreak_access"}),
                    ),
                    responses::ev_completed("resp-1"),
                ]),
                responses::sse(vec![
                    responses::ev_function_call_with_namespace(
                        "daybreak-call",
                        "mcp__sample",
                        "get_codex_security_daybreak_access",
                        "{}",
                    ),
                    responses::ev_completed("resp-2"),
                ]),
                responses::sse(vec![
                    responses::ev_assistant_message("done", "done"),
                    responses::ev_completed("resp-3"),
                ]),
            ],
        )
        .await;
        app.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread.id.clone(),
            input: vec![UserInput::Text {
                text: "Check Daybreak access".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
        loop {
            let event: ItemCompletedNotification = app.read_notification("item/completed").await?;
            if let ThreadItemPayload::McpToolCall {
                result, plugin_id, ..
            } = event.item.payload
            {
                assert_eq!(plugin_id.as_deref(), Some("selected-daybreak"));
                results.push((
                    result
                        .expect("successful model call")
                        .structured_content
                        .expect("echoed metadata"),
                    Some(expected),
                ));
                break;
            }
        }
    }
    let response: McpServerToolCallResponse = app
        .request(|request_id| ClientRequest::McpServerToolCall {
            request_id,
            params: McpServerToolCallParams {
                thread_id: thread.id.clone(),
                server: "sample".to_string(),
                tool: "get_daybreak_access".to_string(),
                arguments: Some(json!({})),
                meta: Some(json!({
                    "source":"direct-test",
                    "openai/entitlementContext":{"untrusted":"caller-supplied"}
                })),
            },
        })
        .await?;
    let direct_metadata = response.structured_content.expect("echoed metadata");
    assert_eq!(direct_metadata["source"], "direct-test");
    results.push((direct_metadata, None));
    for (metadata, expected) in results {
        assert_eq!(metadata.get("openai/entitlementContext"), expected.as_ref());
        assert_eq!(metadata["threadId"], thread.id);
        assert!(!metadata.to_string().contains("daybreak-test-token"));
    }
    server.verify().await;
    Ok(())
}
