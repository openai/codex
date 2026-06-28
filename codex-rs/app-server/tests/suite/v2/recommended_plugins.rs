use anyhow::Result;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::TestAppServer;
use app_test_support::encode_id_token;
use app_test_support::to_response;
use app_test_support::write_mock_responses_config_toml_with_chatgpt_base_url;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(20);
const WORKSPACE_ID: &str = "123e4567-e89b-42d3-a456-426614174010";

#[tokio::test]
async fn first_turn_after_external_login_waits_for_recommended_plugins() -> Result<()> {
    let server = responses::start_mock_server().await;
    let apps_server = AppsTestServer::mount(&server).await?;
    Mock::given(method("GET"))
        .and(path("/ps/plugins/suggested"))
        .and(query_param("scope", "GLOBAL"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(250))
                .set_body_json(json!({
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
        .expect(1)
        .mount(&server)
        .await;
    let response = responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "done"),
        responses::ev_completed("resp-1"),
    ]);
    let responses_mock = responses::mount_sse_once(&server, response).await;

    let codex_home = TempDir::new()?;
    write_mock_responses_config_toml_with_chatgpt_base_url(
        codex_home.path(),
        &server.uri(),
        &apps_server.chatgpt_base_url,
    )?;
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        config_path,
        format!("{config}\n[features]\napps = true\nplugins = true\ntool_suggest = true\n"),
    )?;

    let sqlite_home = codex_home.path().to_string_lossy();
    let mut app_server = TestAppServer::new_without_managed_config_with_env(
        codex_home.path(),
        &[("CODEX_SQLITE_HOME", Some(sqlite_home.as_ref()))],
    )
    .await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.initialize()).await??;

    let access_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("embedded@example.com")
            .plan_type("pro")
            .chatgpt_account_id(WORKSPACE_ID),
    )?;
    let login_id = app_server
        .send_chatgpt_auth_tokens_login_request(
            access_token,
            WORKSPACE_ID.to_string(),
            Some("pro".to_string()),
        )
        .await?;
    let login_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(login_id)),
    )
    .await??;
    assert_eq!(
        to_response::<LoginAccountResponse>(login_response)?,
        LoginAccountResponse::ChatgptAuthTokens {}
    );

    let thread_id = app_server
        .send_thread_start_request(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;
    let thread_response: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(thread_id)),
    )
    .await??;
    let ThreadStartResponse { thread, .. } = to_response(thread_response)?;

    let turn_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.id,
            input: vec![UserInput::Text {
                text: "suggest a plugin".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: JSONRPCResponse = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_response_message(RequestId::Integer(turn_id)),
    )
    .await??;
    timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let requests = responses_mock.requests();
    let request = requests
        .iter()
        .find(|request| {
            request
                .message_input_texts("user")
                .iter()
                .any(|text| text.contains("suggest a plugin"))
        })
        .expect("turn request");
    let contextual_user_message = request.message_input_texts("user").join("\n");
    assert!(contextual_user_message.contains("<recommended_plugins>"));
    assert!(contextual_user_message.contains("- GitHub (github@openai-curated-remote)"));
    let body = request.body_json();
    let tool_names = body
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| tool.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert!(tool_names.contains(&"request_plugin_install"));
    assert!(!tool_names.contains(&"list_available_plugins_to_install"));
    Ok(())
}
