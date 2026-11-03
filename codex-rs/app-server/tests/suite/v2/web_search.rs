use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadSettingsUpdateParams;
use codex_app_server_protocol::ThreadSettingsUpdateResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_app_server_protocol::WebSearchAction;
use codex_app_server_protocol::WebSearchItem;
use codex_config::types::AuthCredentialsStoreMode;
use codex_features::Feature;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS;
use core_test_support::responses;
use core_test_support::responses::strip_response_item_ids_from_json;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[path = "web_search_routing.rs"]
mod routing;

// macOS and Windows Bazel CI can spend tens of seconds starting app-server
// subprocesses or processing test RPCs under load.
#[cfg(any(target_os = "macos", windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(not(any(target_os = "macos", windows)))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn standalone_web_search_round_trips_output() -> Result<()> {
    assert_standalone_web_search_round_trips_output(
        WebSearchProvider::ChatGpt,
        /*literature_search*/ false,
    )
    .await
}

#[tokio::test]
async fn standalone_web_search_round_trips_output_for_custom_provider() -> Result<()> {
    assert_standalone_web_search_round_trips_output(
        WebSearchProvider::CustomResponses,
        /*literature_search*/ false,
    )
    .await
}

#[tokio::test]
async fn standalone_literature_search_round_trips_for_an_opted_in_turn_model() -> Result<()> {
    assert_standalone_web_search_round_trips_output(
        WebSearchProvider::ChatGpt,
        /*literature_search*/ true,
    )
    .await
}

#[derive(Clone, Copy)]
enum WebSearchProvider {
    ChatGpt,
    CustomResponses,
}

#[tokio::test]
async fn standalone_web_search_uses_new_permissions_only_on_the_next_turn() -> Result<()> {
    let server = responses::start_mock_server().await;
    let search_path = "/v1/alpha/search";
    mount_search_response(&server, search_path, /*expected_calls*/ 3).await;
    let search = |id| {
        responses::sse(vec![
            responses::ev_function_call_with_namespace(
                id,
                "web",
                "run",
                r#"{"search_query":[{"q":"weather"}]}"#,
            ),
            responses::ev_completed(id),
        ])
    };
    responses::mount_sse_sequence(
        &server,
        vec![
            search("before-update"),
            responses::sse(vec![
                responses::ev_function_call(
                    "pause",
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "continue", "header": "Continue", "question": "Search again?",
                            "options": [
                                {"label": "Yes", "description": "Search again"},
                                {"label": "No", "description": "Finish"}
                            ]
                        }]
                    })
                    .to_string(),
                ),
                responses::ev_completed("paused"),
            ]),
            search("after-update"),
            responses::sse_completed("first-turn-done"),
            search("next-turn"),
            responses::sse_completed("second-turn-done"),
        ],
    )
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_root_config(r#"web_search = "cached""#)
        .enable_feature(Feature::StandaloneWebSearch)
        .enable_feature(Feature::DefaultModeRequestUserInput)
        .with_provider_config("supports_websockets = false")
        .with_provider_config("supports_standalone_web_search = true")
        .write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let thread_id = app
        .start_thread(ThreadStartParams::default())
        .await?
        .thread
        .id;
    let turn = TurnStartParams {
        thread_id: thread_id.clone(),
        input: vec![V2UserInput::Text {
            text: "Search the web".to_string(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    };
    let start = app.send_turn_start_request(turn.clone()).await?;
    let _: TurnStartResponse = app.read_response(start).await?;
    let ServerRequest::ToolRequestUserInput { request_id, .. } = timeout(
        DEFAULT_READ_TIMEOUT,
        app.read_stream_until_request_message(),
    )
    .await??
    else {
        anyhow::bail!("expected the first turn to pause for input");
    };
    let _: ThreadSettingsUpdateResponse = app
        .request(|request_id| ClientRequest::ThreadSettingsUpdate {
            request_id,
            params: ThreadSettingsUpdateParams {
                thread_id,
                permissions: Some(BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS.to_string()),
                ..Default::default()
            },
        })
        .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app.read_stream_until_matching_notification(
            "thread settings updated to Full Access",
            |notification| {
                notification.method == "thread/settings/updated"
                    && notification
                        .params
                        .as_ref()
                        .and_then(|params| params.pointer("/threadSettings/sandboxPolicy/type"))
                        .and_then(Value::as_str)
                        == Some("dangerFullAccess")
            },
        ),
    )
    .await??;
    app.send_response(
        request_id,
        json!({"answers":{"continue":{"answers":["Yes"]}}}),
    )
    .await?;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app.read_stream_until_notification_message("turn/completed"),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app.start_turn_and_wait_for_completion(turn),
    )
    .await??;

    let mut observed = Vec::new();
    for request in server
        .received_requests()
        .await
        .context("expected HTTP requests")?
    {
        if request.url.path() == search_path {
            let body = request.body_json::<Value>()?;
            observed.push(body["settings"]["external_web_access"].clone());
        }
    }
    assert_eq!(observed, vec![json!(false), json!(false), json!(true)]);
    Ok(())
}

async fn assert_standalone_web_search_round_trips_output(
    provider: WebSearchProvider,
    literature_search: bool,
) -> Result<()> {
    let call_id = "web-run-1";
    let (model, query, commands) = if literature_search {
        (
            "paper-model",
            "plasma biomarkers",
            json!({"literature_search_query": [{"q": "plasma biomarkers", "recency": 30, "domains": ["pubmed.ncbi.nlm.nih.gov"]}]}),
        )
    } else {
        (
            "mock-model",
            "standalone web search",
            json!({"search_query": [{"q": "standalone web search"}]}),
        )
    };
    let expected_model_id = "model-id-from-search-context";
    let search_context = json!({
        "telemetry_attributes": {
            "model_id": expected_model_id,
            "model_slug": model,
        }
    })
    .to_string();
    let client_metadata = HashMap::from([(
        "mcp_request_meta".to_string(),
        json!({ "openai/search_context": search_context }).to_string(),
    )]);
    let server = responses::start_mock_server().await;
    let search_path = match provider {
        WebSearchProvider::ChatGpt => "/api/codex/alpha/search",
        WebSearchProvider::CustomResponses => "/v1/alpha/search",
    };
    mount_search_response(&server, search_path, /*expected_calls*/ 1).await;

    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    call_id,
                    "web",
                    "run",
                    &commands.to_string(),
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "Done"),
                responses::ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let codex_home = TempDir::new()?;
    let config = MockResponsesConfig::new(&server.uri())
        .with_root_config(&format!("chatgpt_base_url = \"{}\"", server.uri()))
        .enable_feature(Feature::StandaloneWebSearch)
        .with_provider_config("supports_websockets = false");
    let config = if literature_search {
        let mut paper_model = model_info_from_slug(model);
        paper_model.experimental_supported_tools = vec!["literature_search_query".to_string()];
        let catalog_path = codex_home.path().join("models.json");
        std::fs::write(
            &catalog_path,
            serde_json::to_vec(
                &json!({"models": [model_info_from_slug("mock-model"), paper_model]}),
            )?,
        )?;
        config.with_root_config(&format!(
            "model_catalog_json = {}",
            serde_json::to_string(&catalog_path)?
        ))
    } else {
        config
    };
    let config = match provider {
        WebSearchProvider::ChatGpt => config
            .with_model_provider("openai-custom")
            .with_provider_name("OpenAI")
            .with_provider_base_url(&format!("{}/api/codex", server.uri()))
            .with_provider_config("requires_openai_auth = true"),
        WebSearchProvider::CustomResponses => config
            .with_model_provider("custom-responses")
            .with_provider_name("Custom Responses")
            .with_provider_base_url(&format!("{}/v1", server.uri()))
            .with_provider_config("env_key = \"CUSTOM_RESPONSES_API_KEY\"")
            .with_provider_config("supports_standalone_web_search = true")
            .with_provider_config("requires_openai_auth = false"),
    };
    config.write(codex_home.path())?;

    if matches!(provider, WebSearchProvider::ChatGpt) {
        write_chatgpt_auth(
            codex_home.path(),
            ChatGptAuthFixture::new("access-chatgpt"),
            AuthCredentialsStoreMode::File,
        )?;
    }

    let env_overrides = match provider {
        WebSearchProvider::ChatGpt => {
            vec![("OPENAI_API_KEY", None), ("CUSTOM_RESPONSES_API_KEY", None)]
        }
        WebSearchProvider::CustomResponses => vec![
            ("OPENAI_API_KEY", None),
            ("CUSTOM_RESPONSES_API_KEY", Some("test-api-key")),
        ],
    };

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&env_overrides)
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    let thread_req = mcp
        .send_thread_start_request_with_auto_env(ThreadStartParams {
            service_name: Some("chatgpt_cca".to_string()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(thread_req)).await??;
    let thread_id = thread.id.clone();

    let turn_req = mcp
        .send_turn_start_request(TurnStartParams {
            thread_id: thread_id.clone(),
            client_user_message_id: None,
            model: literature_search.then(|| model.to_string()),
            input: vec![V2UserInput::Text {
                text: "Search the web".to_string(),
                text_elements: Vec::new(),
            }],
            responsesapi_client_metadata: Some(client_metadata.clone()),
            ..Default::default()
        })
        .await?;
    let _turn: TurnStartResponse =
        timeout(DEFAULT_READ_TIMEOUT, mcp.read_response(turn_req)).await??;

    let started = timeout(DEFAULT_READ_TIMEOUT, wait_for_web_search_started(&mut mcp)).await??;
    let completed = timeout(
        DEFAULT_READ_TIMEOUT,
        wait_for_web_search_completed(&mut mcp),
    )
    .await??;

    timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);

    let first_response = requests[0].body_json();
    let web_run = requests[0]
        .tool_by_name("web", "run")
        .context("web.run should be sent to the model")?;
    assert_eq!(
        web_run.pointer("/parameters/properties/time/description"),
        Some(&json!("Get time for the given UTC offsets."))
    );
    assert_eq!(
        web_run
            .pointer("/parameters/properties/literature_search_query")
            .is_some(),
        literature_search
    );
    assert!(
        !has_hosted_web_search(&first_response),
        "standalone web search should replace hosted web search"
    );

    let search_request = search_request(&server, search_path).await?;
    let expected_authorization = match provider {
        WebSearchProvider::ChatGpt => "Bearer access-chatgpt",
        WebSearchProvider::CustomResponses => "Bearer test-api-key",
    };
    assert_eq!(
        search_request
            .headers
            .get("authorization")
            .context("standalone search should include provider authorization")?
            .to_str()
            .context("standalone search authorization should be valid ASCII")?,
        expected_authorization
    );
    if matches!(provider, WebSearchProvider::CustomResponses) {
        assert!(
            search_request
                .headers
                .get("x-openai-actor-authorization")
                .is_none()
        );
    }
    assert_eq!(
        search_request
            .headers
            .get("originator")
            .context("standalone search should include the thread originator")?
            .to_str()
            .context("standalone search originator should be valid ASCII")?,
        "chatgpt_cca"
    );
    let search_body = search_request
        .body_json::<Value>()
        .context("search request body should be JSON")?;
    assert!(
        search_body.get("result_fields").is_none(),
        "standalone search should use the endpoint's default result projection"
    );
    assert_eq!(search_body["model"], json!(model));
    assert_eq!(search_body["commands"], commands);
    assert_eq!(
        search_body["settings"]["allowed_callers"],
        json!(["direct"])
    );
    assert_eq!(
        search_body["input"]
            .as_array()
            .context("search input should be an array")?
            .last()
            .cloned()
            .map(responses::strip_metadata_from_json),
        Some(json!({
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": "Search the web"}],
        }))
    );
    let turn_metadata_header = search_request
        .headers
        .get("x-codex-turn-metadata")
        .context("standalone search should include x-codex-turn-metadata")?
        .to_str()
        .context("x-codex-turn-metadata should be valid ASCII")?;
    let turn_metadata: Value = serde_json::from_str(turn_metadata_header)
        .context("x-codex-turn-metadata should be valid JSON")?;
    let mcp_request_meta = turn_metadata["mcp_request_meta"]
        .as_str()
        .context("mcp_request_meta should be a JSON string")?;
    let mcp_request_meta: Value = serde_json::from_str(mcp_request_meta)
        .context("mcp_request_meta should contain valid JSON")?;
    let search_context = mcp_request_meta["openai/search_context"]
        .as_str()
        .context("openai/search_context should be a JSON string")?;
    let search_context: Value = serde_json::from_str(search_context)
        .context("openai/search_context should contain valid JSON")?;
    assert_eq!(
        search_context
            .pointer("/telemetry_attributes/model_id")
            .and_then(Value::as_str),
        Some(expected_model_id)
    );

    assert_eq!(
        strip_response_item_ids_from_json(responses::strip_metadata_from_json(
            requests[1].function_call_output(call_id),
        )),
        json!({
            "type": "function_call_output",
            "call_id": call_id,
            "output": [{
                "type": "input_text",
                "text": "Search result",
            }],
        })
    );
    assert_eq!(
        started.item.payload,
        ThreadItemPayload::WebSearch(WebSearchItem {
            id: call_id.to_string(),
            query: String::new(),
            action: None,
            results: None,
        })
    );
    let expected_completed_item = ThreadItemPayload::WebSearch(WebSearchItem {
        id: call_id.to_string(),
        query: query.to_string(),
        action: Some(WebSearchAction::Search {
            query: Some(query.to_string()),
            queries: None,
        }),
        results: Some(vec![json!({
            "type": "text_result",
            "ref_id": "turn0search0",
            "url": "https://example.com/search-result",
            "title": "Search Result",
            "snippet": "A result snippet",
            "future_field": {"preserved": true},
        })]),
    });
    assert_eq!(completed.item.payload, expected_completed_item);

    drop(mcp);
    let mut reloaded_mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&env_overrides)
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let read_req = reloaded_mcp
        .send_thread_read_request(ThreadReadParams {
            thread_id,
            include_turns: true,
        })
        .await?;
    let ThreadReadResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, reloaded_mcp.read_response(read_req)).await??;
    let persisted_web_searches: Vec<&ThreadItemPayload> = thread
        .turns
        .iter()
        .flat_map(Turn::item_payloads)
        .filter(|item| matches!(item, ThreadItemPayload::WebSearch(_)))
        .collect();
    assert_eq!(persisted_web_searches, vec![&expected_completed_item]);

    Ok(())
}

async fn wait_for_web_search_started(mcp: &mut TestAppServer) -> Result<ItemStartedNotification> {
    loop {
        let started: ItemStartedNotification = mcp.read_notification("item/started").await?;
        if matches!(&started.item.payload, ThreadItemPayload::WebSearch(_)) {
            return Ok(started);
        }
    }
}

async fn wait_for_web_search_completed(
    mcp: &mut TestAppServer,
) -> Result<ItemCompletedNotification> {
    loop {
        let completed: ItemCompletedNotification = mcp.read_notification("item/completed").await?;
        if matches!(&completed.item.payload, ThreadItemPayload::WebSearch(_)) {
            return Ok(completed);
        }
    }
}

async fn mount_search_response(server: &MockServer, search_path: &str, expected_calls: u64) {
    Mock::given(method("POST"))
        .and(path(search_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "encrypted_output": "ciphertext",
            "output": "Search result",
            "results": [{
                "type": "text_result",
                "ref_id": "turn0search0",
                "url": "https://example.com/search-result",
                "title": "Search Result",
                "snippet": "A result snippet",
                "future_field": {"preserved": true},
            }],
        })))
        .expect(expected_calls)
        .mount(server)
        .await;
}

fn has_hosted_web_search(body: &Value) -> bool {
    body.get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| {
            tools
                .iter()
                .any(|tool| tool.get("type").and_then(Value::as_str) == Some("web_search"))
        })
}

async fn search_request(server: &MockServer, search_path: &str) -> Result<wiremock::Request> {
    let requests = server
        .received_requests()
        .await
        .context("failed to fetch received requests")?;
    requests
        .into_iter()
        .find(|request| request.url.path() == search_path)
        .context("expected standalone search request")
}

const XATU_V99: &str = "SonicBrowserToolV99";
const PAPER_MODEL_TOOLS: &[&str] = &["literature_search_query", "other_model_tool"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum SearchPolicy {
    Standard,
    HostedOnly,
    RequireNative,
    RequireNativeClientMcp,
    RequireNativeCached,
    RequireNativeTelemetry,
    RequireNativeDomainPolicy,
    HostAllowedDomains(&'static [&'static str]),
    Standalone(WebSearchMode),
    Disabled,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiscoveryFailure {
    Rpc(i32),
    Http(u16),
}

/// Model Lab selection is resolved for the active turn on the same app-server used by both clients.
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), None, SearchPolicy::Standard, false; "xatu_literature_treatment")]
#[test_case::test_case(XATU_V99, (&[], false), None, SearchPolicy::Standard, false; "xatu_control")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), Some(DiscoveryFailure::Rpc(-32603)), SearchPolicy::Standard, false; "critical_discovery_error")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), Some(DiscoveryFailure::Rpc(-32602)), SearchPolicy::Standard, false; "invalid_discovery_request")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), Some(DiscoveryFailure::Http(503)), SearchPolicy::Standard, false; "service_unavailable_after_retries_allows_fallback")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), Some(DiscoveryFailure::Http(403)), SearchPolicy::Standard, false; "forbidden_discovery_disables_search")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), None, SearchPolicy::HostedOnly, false; "work_hosted_only")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), Some(DiscoveryFailure::Rpc(-32603)), SearchPolicy::HostedOnly, false; "work_prohibits_native_fallback")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), None, SearchPolicy::Standard, true; "failed_invocation_never_replays_on_native")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, false), None, SearchPolicy::Standard, false; "host_disables_literature_only")]
#[test_case::test_case(XATU_V99, (&["other_model_tool"], false), None, SearchPolicy::Standard, false; "model_with_other_supported_tools")]
#[test_case::test_case("SonicBrowserToolV66", (PAPER_MODEL_TOOLS, false), None, SearchPolicy::Standard, false; "inherit_connector_version")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), None, SearchPolicy::RequireNative, false; "managed_native_requirement")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), None, SearchPolicy::RequireNativeClientMcp, false; "native_requirement_overrides_client_mcp_without_model_preference")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), None, SearchPolicy::RequireNativeCached, false; "native_requirement_enforces_cached_turn")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), None, SearchPolicy::RequireNativeTelemetry, false; "native_requirement_with_telemetry_and_no_search_connector")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), None, SearchPolicy::RequireNativeDomainPolicy, false; "native_requirement_cannot_bypass_connector_domain_policy")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), None, SearchPolicy::Standalone(WebSearchMode::Cached), false; "standalone_model_honors_cached_turn")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), None, SearchPolicy::Standalone(WebSearchMode::Live), false; "desktop_installation_id_preserves_standalone_search")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), Some(DiscoveryFailure::Rpc(-32603)), SearchPolicy::HostAllowedDomains(&[]), false; "empty_host_and_native_domain_lists_deny_fallback")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), Some(DiscoveryFailure::Rpc(-32603)), SearchPolicy::HostAllowedDomains(&["example.com"]), false; "matching_query_filter_cannot_authorize_cached_navigation")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), None, SearchPolicy::Standalone(WebSearchMode::Disabled), false; "standalone_model_honors_disabled_turn")]
#[test_case::test_case(XATU_V99, (PAPER_MODEL_TOOLS, true), None, SearchPolicy::Disabled, false; "metadata_cannot_authorize_search")]
#[tokio::test]
async fn model_lab_search_provider(
    pinned_type: &str,
    (supported_tools, pinned_literature): (&[&str], bool),
    error: Option<DiscoveryFailure>,
    policy: SearchPolicy,
    invocation_error: bool,
) -> Result<()> {
    use core_test_support::apps_test_server::AppsTestServer;
    use std::sync::Arc;
    use std::sync::Mutex;
    let server = responses::start_mock_server().await;
    let tool = json!({
        "name": "search_service.web_run", "description": "Search", "annotations": {"readOnlyHint": true},
        "inputSchema": {"type":"object","properties":{"system2_search_query":{"type":"array"},"open":{"type":"array"}}},
        "_meta": {"connector_id": codex_mcp::SEARCH_CONNECTOR, "connector_name":"search_service"}
    });
    let catalog_tools = if policy == SearchPolicy::RequireNativeTelemetry {
        vec![]
    } else {
        vec![tool.clone()]
    };
    let apps =
        AppsTestServer::mount_with_tools(&server, Arc::new(Mutex::new(catalog_tools))).await?;
    let mut pinned = tool;
    let search_query = if pinned_type == "SonicBrowserToolV66" {
        let properties = pinned["inputSchema"]["properties"]
            .as_object_mut()
            .context("search schema")?;
        let query = properties
            .remove("system2_search_query")
            .context("query schema")?;
        properties.insert("system1_search_query".into(), query);
        "system1_search_query"
    } else {
        "system2_search_query"
    };
    if pinned_literature {
        pinned["inputSchema"]["properties"]["literature_search_query"] =
            json!({"type":"array","items":{"type":"object"}});
    }
    let profile =
        json!({"config":{"tool_type":pinned_type,"allow_literature_search":pinned_literature}});
    pinned["_meta"]["_codex_apps"] = json!({"invocation_context":profile});
    Mock::given(method("POST"))
        .and(wiremock::matchers::body_partial_json(json!({
            "method":"tools/list","params":{"_meta":{"x-codex-turn-metadata":{"mcp_request_meta":{
                "openai/search_context":{"model_supported_tools":supported_tools}
            }}}}
        })))
        .respond_with(move |r: &wiremock::Request| {
            let req: Value = r.body_json().expect("JSON-RPC request");
            let mut response = json!({"jsonrpc":"2.0","id":req["id"]});
            match error {
                Some(DiscoveryFailure::Http(status)) => return ResponseTemplate::new(status),
                Some(DiscoveryFailure::Rpc(code)) => {
                    response["error"] = json!({"code":code,"message":"discovery failed"});
                }
                None => response["result"] = json!({"tools":[pinned]}),
            }
            ResponseTemplate::new(200).set_body_json(response)
        })
        .with_priority(1)
        .mount(&server)
        .await;
    let namespace = "mcp__codex_apps__search_service";
    let key = if pinned_literature {
        "literature_search_query"
    } else {
        search_query
    };
    let commands = json!({(key): [{"q":"plasma biomarkers"}]});
    let mut steps = Vec::new();
    let native = matches!(
        policy,
        SearchPolicy::RequireNative
            | SearchPolicy::RequireNativeClientMcp
            | SearchPolicy::RequireNativeCached
            | SearchPolicy::RequireNativeTelemetry
            | SearchPolicy::Standalone(WebSearchMode::Cached | WebSearchMode::Live)
    ) || (matches!(
        error,
        Some(DiscoveryFailure::Rpc(-32603) | DiscoveryFailure::Http(503))
    ) && policy == SearchPolicy::Standard);
    if let SearchPolicy::HostAllowedDomains(domains) = policy
        && !domains.is_empty()
    {
        // A stale model call must not reach native, even for a URL already in its cache.
        steps.push(responses::sse(vec![
            responses::ev_function_call_with_namespace(
                "outside",
                "web",
                "run",
                &json!({"open":[{"ref_id":"https://outside.example/paper"}]}).to_string(),
            ),
            responses::ev_completed("outside"),
        ]));
    }
    if native {
        mount_search_response(&server, "/v1/alpha/search", /*expected_calls*/ 1).await;
        steps.push(responses::sse(vec![
            responses::ev_function_call_with_namespace(
                "native-search",
                "web",
                "run",
                &commands.to_string(),
            ),
            responses::ev_completed("native"),
        ]));
    }
    let allow_hosted = matches!(
        policy,
        SearchPolicy::Standard | SearchPolicy::HostedOnly | SearchPolicy::HostAllowedDomains(_)
    );
    let hosted = allow_hosted && error.is_none();
    if hosted {
        steps.push(responses::sse(vec![
            responses::ev_tool_search_call(
                "find-search",
                &json!({"query": "search_service web_run"}),
            ),
            responses::ev_completed("discovered"),
        ]));
        steps.push(responses::sse(vec![
            responses::ev_function_call_with_namespace(
                "search-1",
                namespace,
                "_web_run",
                &commands.to_string(),
            ),
            responses::ev_completed("resp-1"),
        ]));
        Mock::given(method("POST"))
            .and(wiremock::matchers::body_partial_json(json!({
                "method": "tools/call",
                "params": {"name": "search_service.web_run"}
            })))
            .respond_with(move |r: &wiremock::Request| {
                let body: Value = r.body_json().expect("JSON-RPC request");
                ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body["id"],
                    "result": {
                        "isError": invocation_error,
                        "content": [{"type": "text", "text": "search result"}]
                    }
                }))
            })
            .with_priority(1)
            .mount(&server)
            .await;
    }
    steps.push(responses::sse(vec![
        responses::ev_assistant_message("msg", "Done"),
        responses::ev_completed("resp-final"),
    ]));
    let responses = responses::mount_sse_sequence(&server, steps).await;
    let home = TempDir::new()?;
    let mut model = model_info_from_slug("mock-model");
    model.web_search_provider = match policy {
        SearchPolicy::RequireNativeClientMcp => None,
        SearchPolicy::Standalone(_) => {
            Some(codex_protocol::openai_models::WebSearchProvider::Standalone)
        }
        _ => Some(codex_protocol::openai_models::WebSearchProvider::SearchMcp),
    };
    model.supports_search_tool = true;
    model.use_responses_lite = true;
    model.experimental_supported_tools = supported_tools
        .iter()
        .map(|tool| (*tool).to_owned())
        .collect();
    let catalog = home.path().join("models.json");
    std::fs::write(&catalog, serde_json::to_vec(&json!({"models":[model]}))?)?;
    let mode = if matches!(policy, SearchPolicy::HostedOnly | SearchPolicy::Disabled) {
        "disabled"
    } else {
        "live"
    };
    if matches!(
        policy,
        SearchPolicy::RequireNative
            | SearchPolicy::RequireNativeClientMcp
            | SearchPolicy::RequireNativeCached
            | SearchPolicy::RequireNativeTelemetry
            | SearchPolicy::RequireNativeDomainPolicy
    ) {
        std::fs::write(
            home.path().join("requirements.toml"),
            "[feature_requirements]\nstandalone_web_search = true\n",
        )?;
    }
    let client_provider = if policy == SearchPolicy::RequireNativeClientMcp {
        "search_mcp"
    } else {
        "standalone"
    };
    let domain_config = match policy {
        SearchPolicy::HostAllowedDomains(domains) => format!(
            "\ntools.web_search.allowed_domains = {}",
            serde_json::to_string(domains)?
        ),
        _ => String::new(),
    };
    MockResponsesConfig::new(&server.uri())
        .with_root_config(&format!(
            "chatgpt_base_url = {}\nmodel_catalog_json = {}\nweb_search = \"{mode}\"\ntools.client_search_provider = \"{client_provider}\"{domain_config}",
            serde_json::to_string(&apps.chatgpt_base_url)?,
            serde_json::to_string(&catalog)?,
        ))
        .with_model_provider("openai-custom")
        .with_provider_name("OpenAI")
        .with_provider_config("requires_openai_auth = true")
        .with_features(&[
            (Feature::Apps, !matches!(policy, SearchPolicy::Standalone(_))),
            (Feature::StandaloneWebSearch, true),
            (Feature::HostedSearchPolicy, policy == SearchPolicy::HostedOnly),
        ].into())
        .with_provider_config("supports_websockets = false")
        .with_provider_config("supports_standalone_web_search = true")
        .write(home.path())?;
    write_chatgpt_auth(
        home.path(),
        ChatGptAuthFixture::new("test-account-token").account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    let mut app = TestAppServer::builder()
        .with_codex_home(home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build_initialized()
        .await?;
    let thread = app.start_thread(ThreadStartParams::default()).await?.thread;
    app.start_turn_and_wait_for_completion(TurnStartParams {
        thread_id: thread.id,
        responsesapi_client_metadata: match policy {
            SearchPolicy::HostedOnly | SearchPolicy::Disabled => Some(json!({"mode":"live"})),
            SearchPolicy::RequireNativeCached => Some(json!({"mode":"cached"})),
            SearchPolicy::RequireNativeTelemetry => {
                Some(json!({"telemetry_attributes":{"source":"test"}}))
            }
            SearchPolicy::RequireNativeDomainPolicy => {
                Some(json!({"allowed_domains":["example.org"]}))
            }
            SearchPolicy::HostAllowedDomains([]) => Some(json!({"allowed_domains":[]})),
            SearchPolicy::HostAllowedDomains(domains) => {
                Some(json!({"mode":"cached","allowed_domains":domains}))
            }
            SearchPolicy::Standalone(mode) => Some(json!({"mode":mode})),
            _ => None,
        }
        .map(|context| {
            let mut meta = json!({"openai/search_context":context});
            if matches!(
                policy,
                SearchPolicy::Standalone(_) | SearchPolicy::RequireNativeTelemetry
            ) {
                meta["openai/codex_installation_id"] = json!("desktop-installation");
            }
            [("mcp_request_meta".to_string(), meta.to_string())].into()
        }),
        input: vec![V2UserInput::Text {
            text: "Search papers".into(),
            text_elements: vec![],
        }],
        ..Default::default()
    })
    .await?;
    let requests = responses.requests();
    let received = server
        .received_requests()
        .await
        .context("recorded search requests")?;
    let rpc: Vec<Value> = received
        .iter()
        .filter_map(|r| r.body_json::<Value>().ok())
        .filter(|r| r.get("method").is_some())
        .collect();
    let scoped_lists = rpc
        .iter()
        .filter(|r| r["method"] == "tools/list")
        .filter_map(|r| {
            r.pointer("/params/_meta/x-codex-turn-metadata/mcp_request_meta/openai~1search_context")
        })
        .filter(|ctx| ctx["model_supported_tools"] == json!(supported_tools))
        .count();
    if error == Some(DiscoveryFailure::Http(503)) {
        assert!(scoped_lists > 1, "fallback should follow a discovery retry");
    } else {
        assert_eq!(scoped_lists, usize::from(allow_hosted));
    }
    let request = requests[0].body_json();
    let tools = request["input"]
        .as_array()
        .context("Lite input")?
        .iter()
        .find(|item| item["type"] == "additional_tools")
        .context("Lite additional_tools")?;
    let find_tool = |ns: &str, name: &str| responses::namespace_child_tool(tools, ns, name);
    assert_eq!(find_tool("web", "run").is_some(), native);
    assert!(!has_hosted_web_search(&request));
    assert!(find_tool(namespace, "_web_run").is_none());
    if hosted {
        let found = requests[1].tool_search_output("find-search");
        let tool = responses::namespace_child_tool(&found, namespace, "_web_run")
            .context("profiled search tool in the lookup result")?;
        assert_eq!(
            tool.pointer("/parameters/properties/literature_search_query")
                .is_some(),
            pinned_literature
        );
        assert!(
            requests[2]
                .function_call_output("search-1")
                .to_string()
                .contains("search result")
        );
    }
    assert_eq!(
        received
            .iter()
            .filter(|r| r.url.path().ends_with("/alpha/search"))
            .count(),
        usize::from(native)
    );
    if matches!(
        policy,
        SearchPolicy::Standalone(WebSearchMode::Cached)
            | SearchPolicy::RequireNativeCached
            | SearchPolicy::RequireNativeTelemetry
    ) {
        let body = search_request(&server, "/v1/alpha/search")
            .await?
            .body_json::<Value>()?;
        assert_eq!(
            body["settings"]["external_web_access"],
            policy == SearchPolicy::RequireNativeTelemetry
        );
    }
    let calls = core_test_support::apps_test_server::recorded_apps_tool_calls(&server).await;
    assert_eq!(calls.len(), usize::from(hosted));
    if let Some(call) = calls.first() {
        assert_eq!(call["params"]["arguments"], commands);
        assert_eq!(
            call.pointer("/params/_meta/x-codex-turn-metadata/mcp_request_meta/openai~1search_context/model_supported_tools"),
            Some(&json!(supported_tools))
        );
        assert_eq!(
            call.pointer("/params/_meta/_codex_apps/invocation_context"),
            Some(&profile)
        );
    }
    Ok(())
}
