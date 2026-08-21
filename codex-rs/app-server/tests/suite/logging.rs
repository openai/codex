use anyhow::Context;
use anyhow::Result;
use app_test_support::ChatGptIdTokenClaims;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::app_server_json_shutdown_event;
use app_test_support::create_exec_command_sse_response;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_mock_responses_server_sequence;
use app_test_support::encode_id_token;
use app_test_support::write_models_cache;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::LoginAccountResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use codex_state::LogQuery;
use codex_state::SqliteConfig;
use codex_state::StateRuntime;
use codex_utils_absolute_path::test_support::PathExt;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

const READ_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn credentials_stay_out_of_persisted_and_feedback_logs() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let bearer = "synthetic-provider-bearer";
    let header = "synthetic-provider-header";
    let attestation = "synthetic-attestation-token";
    let account_id = "123e4567-e89b-42d3-a456-426614174011";
    let initial_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("initial@example.com")
            .chatgpt_account_id(account_id),
    )?;
    let refreshed_token = encode_id_token(
        &ChatGptIdTokenClaims::new()
            .email("refreshed@example.com")
            .chatgpt_account_id(account_id),
    )?;
    let server = MockServer::start().await;
    let success = responses::sse_response(create_final_assistant_message_sse_response("done")?);
    let responses = responses::mount_response_sequence(
        &server,
        vec![success.clone(), ResponseTemplate::new(401), success],
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/wham/settings/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "commit_attribution_enabled": false,
        })))
        .mount(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_uri = server.uri();
    MockResponsesConfig::new(&server_uri)
        .with_root_config(&format!("chatgpt_base_url = \"{server_uri}/backend-api\""))
        .with_provider_config("requires_openai_auth = true\nsupports_websockets = false")
        .with_extra_config(&format!(
            r#"
[model_providers.bearer_provider]
name = "Bearer provider"
base_url = "{server_uri}/v1"
experimental_bearer_token = "{bearer}"
http_headers = {{ X-Credential = "{header}" }}
supports_websockets = false
"#
        ))
        .write(codex_home.path())?;
    write_models_cache(codex_home.path())?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None)])
        .build()
        .await?;
    let initialized = app_server
        .initialize_with_capabilities(
            ClientInfo {
                name: "codex_desktop".into(),
                title: None,
                version: "0.1.0".into(),
            },
            Some(InitializeCapabilities {
                experimental_api: true,
                request_attestation: true,
                ..Default::default()
            }),
        )
        .await?;
    anyhow::ensure!(
        matches!(initialized, JSONRPCMessage::Response(_)),
        "initialization failed"
    );
    let login_id = app_server
        .send_chatgpt_auth_tokens_login_request(
            initial_token.clone(),
            account_id.into(),
            Some("pro".into()),
        )
        .await?;
    let _: LoginAccountResponse = app_server.read_response(login_id).await?;

    let mut thread_ids = Vec::new();
    for provider in ["bearer_provider", "mock_provider"] {
        let thread = app_server
            .start_thread(ThreadStartParams {
                model_provider: Some(provider.into()),
                ..Default::default()
            })
            .await?
            .thread;
        app_server
            .send_turn_start_request(TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "hello".into(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            })
            .await?;
        timeout(Duration::from_secs(/*secs*/ 60), async {
            loop {
                match app_server.read_next_message().await? {
                    JSONRPCMessage::Request(request) => {
                        let (request_id, result) = match ServerRequest::try_from(request)? {
                            ServerRequest::AttestationGenerate { request_id, .. } => {
                                (request_id, json!({ "token": attestation }))
                            }
                            ServerRequest::ChatgptAuthTokensRefresh { request_id, .. } => (
                                request_id,
                                json!({
                                    "accessToken": refreshed_token,
                                    "chatgptAccountId": account_id,
                                    "chatgptPlanType": "pro",
                                }),
                            ),
                            request => anyhow::bail!("unexpected request: {request:?}"),
                        };
                        app_server.send_response(request_id, result).await?;
                    }
                    JSONRPCMessage::Notification(notification)
                        if notification.method == "turn/completed" =>
                    {
                        let params = notification
                            .params
                            .context("missing turn/completed params")?;
                        assert_eq!(params["turn"]["status"], "completed");
                        break Ok::<_, anyhow::Error>(());
                    }
                    JSONRPCMessage::Error(error) => anyhow::bail!("unexpected error: {error:?}"),
                    JSONRPCMessage::Response(_) | JSONRPCMessage::Notification(_) => {}
                }
            }
        })
        .await??;
        thread_ids.push(thread.id);
    }
    let requests = responses.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.header("authorization"))
            .collect::<Vec<_>>(),
        vec![
            Some(format!("Bearer {bearer}")),
            Some(format!("Bearer {initial_token}")),
            Some(format!("Bearer {refreshed_token}")),
        ]
    );
    assert_eq!(requests[0].header("x-credential").as_deref(), Some(header));
    assert_eq!(
        requests[2].header("x-oai-attestation"),
        Some(format!(r#"{{"v":1,"s":0,"t":"{attestation}"}}"#))
    );

    // Wait for a later event so buffered logs cannot hide a leak.
    let barrier = "credential-log-barrier";
    app_server
        .send_response(RequestId::String(barrier.into()), json!({}))
        .await?;
    let state = StateRuntime::init(
        SqliteConfig::new_for_testing(codex_home.path().abs()),
        "mock_provider".into(),
    )
    .await?;
    let thread_ids = thread_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let feedback = timeout(Duration::from_secs(/*secs*/ 60), async {
        loop {
            let logs =
                String::from_utf8(state.query_feedback_logs_for_threads(&thread_ids).await?)?;
            if logs.contains(barrier) {
                break Ok::<_, anyhow::Error>(logs);
            }
            tokio::time::sleep(Duration::from_millis(/*millis*/ 50)).await;
        }
    })
    .await??;
    let persisted = format!("{:?}", state.query_logs(&LogQuery::default()).await?);
    state.close().await;
    // The HTTP assertions above prove the credentials were used. The barrier
    // confirms earlier queued logs were persisted before we check for leaks.
    for (sink, logs) in [("SQLite", persisted), ("feedback", feedback)] {
        anyhow::ensure!(logs.contains(barrier), "missing log barrier in {sink} logs");
        for secret in [
            bearer,
            header,
            &initial_token,
            &refreshed_token,
            attestation,
        ] {
            anyhow::ensure!(
                !logs.contains(secret),
                "credential leaked into {sink} logs: {secret}"
            );
        }
    }
    Ok(())
}

#[test]
fn standalone_app_server_emits_json_info_events() -> Result<()> {
    let codex_home = TempDir::new()?;
    let event = app_server_json_shutdown_event("codex-app-server", &[], codex_home.path())?;

    assert_eq!(
        event,
        json!({
            "level": "INFO",
            "fields": {
                "message": "processor task exited",
                "exit_reason": "stdio_connection_closed",
                "remaining_connection_count": 0,
                "shutdown_forced": false,
            },
            "target": "codex_app_server",
        })
    );

    Ok(())
}

#[tokio::test]
async fn app_server_emits_structured_tool_call_timing_event() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = create_mock_responses_server_sequence(vec![
        create_exec_command_sse_response("exec-call-1")?,
        create_final_assistant_message_sse_response("done")?,
    ])
    .await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .enable_feature(Feature::UnifiedExec)
        .with_root_config("compact_prompt = \"compact\"\nmodel_auto_compact_token_limit = 100000")
        .with_provider_config("supports_websockets = false")
        .write(codex_home.path())?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_json_logging("warn,codex_core::tools::parallel=info")
        .build_initialized()
        .await?;

    let thread = app_server
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?
        .thread;

    let TurnStartResponse { turn } = app_server
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "run a command".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

    timeout(
        READ_TIMEOUT,
        app_server.read_stream_until_notification_message("turn/completed"),
    )
    .await??;

    let mut tool_call = app_server
        .wait_for_json_log_event("codex.tool_call")
        .await?;
    let tool_call_object = tool_call
        .as_object_mut()
        .context("tool call log event must be an object")?;
    // JsonLogCapture already validates the timestamp as RFC 3339.
    tool_call_object
        .remove("timestamp")
        .context("tool call log event must include a timestamp")?;
    let fields = tool_call_object
        .get_mut("fields")
        .and_then(Value::as_object_mut)
        .context("tool call log event fields must be an object")?;
    let trace_id = fields
        .remove("trace_id")
        .context("tool call log event must include trace_id")?;
    anyhow::ensure!(trace_id.is_string(), "trace_id must be a string");
    let dispatch_duration_ms = fields
        .remove("dispatch_duration_ms")
        .and_then(|duration| duration.as_u64())
        .context("dispatch_duration_ms must be a nonnegative integer")?;
    let handler_duration_ms = fields
        .remove("handler_duration_ms")
        .and_then(|duration| duration.as_u64())
        .context("handler_duration_ms must be a nonnegative integer")?;
    let total_duration_ms = fields
        .remove("total_duration_ms")
        .and_then(|duration| duration.as_u64())
        .context("total_duration_ms must be a nonnegative integer")?;
    let accounted_duration_ms = dispatch_duration_ms
        .checked_add(handler_duration_ms)
        .context("dispatch and handler durations must not overflow")?;
    anyhow::ensure!(
        total_duration_ms >= accounted_duration_ms
            && total_duration_ms - accounted_duration_ms <= 1,
        "dispatch and handler durations must account for total duration within integer truncation"
    );

    assert_eq!(
        tool_call,
        json!({
            "level": "INFO",
            "fields": {
                "message": "tool call completed",
                "event.name": "codex.tool_call",
                "conversation.id": thread.id,
                "turn_id": turn.id,
                "tool_name": "exec_command",
                "call_id": "exec-call-1",
                "tool_source": "direct",
                "execution_started": true,
            },
            "target": "codex_core::tools::parallel",
        })
    );

    Ok(())
}
