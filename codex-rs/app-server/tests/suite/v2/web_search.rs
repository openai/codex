use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_app_server_protocol::WebSearchAction;
use codex_app_server_protocol::WebSearchItem;
use codex_config::types::AuthCredentialsStoreMode;
use codex_features::Feature;
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

// macOS and Windows Bazel CI can spend tens of seconds starting app-server
// subprocesses or processing test RPCs under load.
#[cfg(any(target_os = "macos", windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(60);
#[cfg(not(any(target_os = "macos", windows)))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn standalone_web_search_round_trips_output() -> Result<()> {
    assert_standalone_web_search_round_trips_output(WebSearchProvider::ChatGpt).await
}

#[tokio::test]
async fn standalone_web_search_round_trips_output_for_custom_provider() -> Result<()> {
    assert_standalone_web_search_round_trips_output(WebSearchProvider::CustomResponses).await
}

#[derive(Clone, Copy)]
enum WebSearchProvider {
    ChatGpt,
    CustomResponses,
}

async fn assert_standalone_web_search_round_trips_output(
    provider: WebSearchProvider,
) -> Result<()> {
    let call_id = "web-run-1";
    let expected_model_id = "model-id-from-search-context";
    let search_context = json!({
        "telemetry_attributes": {
            "model_id": expected_model_id,
            "model_slug": "mock-model",
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
    mount_search_response(&server, search_path).await;

    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_function_call_with_namespace(
                    call_id,
                    "web",
                    "run",
                    &json!({
                        "search_query": [{"q": "standalone web search"}],
                    })
                    .to_string(),
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
    assert_eq!(search_body["model"], json!("mock-model"));
    assert_eq!(
        search_body["commands"],
        json!({
            "search_query": [{"q": "standalone web search"}],
        })
    );
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
        started.item,
        ThreadItem::WebSearch(WebSearchItem {
            id: call_id.to_string(),
            query: String::new(),
            action: None,
            results: None,
        })
    );
    let expected_completed_item = ThreadItem::WebSearch(WebSearchItem {
        id: call_id.to_string(),
        query: "standalone web search".to_string(),
        action: Some(WebSearchAction::Search {
            query: Some("standalone web search".to_string()),
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
    assert_eq!(completed.item, expected_completed_item);

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
    let persisted_web_searches: Vec<&ThreadItem> = thread
        .turns
        .iter()
        .flat_map(|turn| &turn.items)
        .filter(|item| matches!(item, ThreadItem::WebSearch(_)))
        .collect();
    assert_eq!(persisted_web_searches, vec![&expected_completed_item]);

    Ok(())
}

async fn wait_for_web_search_started(mcp: &mut TestAppServer) -> Result<ItemStartedNotification> {
    loop {
        let started: ItemStartedNotification = mcp.read_notification("item/started").await?;
        if matches!(&started.item, ThreadItem::WebSearch(_)) {
            return Ok(started);
        }
    }
}

async fn wait_for_web_search_completed(
    mcp: &mut TestAppServer,
) -> Result<ItemCompletedNotification> {
    loop {
        let completed: ItemCompletedNotification = mcp.read_notification("item/completed").await?;
        if matches!(&completed.item, ThreadItem::WebSearch(_)) {
            return Ok(completed);
        }
    }
}

async fn mount_search_response(server: &MockServer, search_path: &str) {
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
        .expect(1)
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
