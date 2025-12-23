#![cfg(not(target_os = "windows"))]

use std::sync::Arc;

use anyhow::Result;
use codex_core::CodexAuth;
use codex_core::features::Feature;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_local_shell_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use tokio::time::Duration;
use wiremock::Match;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;

#[derive(Clone)]
struct ToolOutputWithEtagMatcher {
    expected_etag: &'static str,
    call_id: &'static str,
}

impl Match for ToolOutputWithEtagMatcher {
    fn matches(&self, request: &Request) -> bool {
        let header_matches = request
            .headers
            .get("x-if-models-match")
            .and_then(|v| v.to_str().ok())
            == Some(self.expected_etag);

        let body = String::from_utf8_lossy(&request.body);
        header_matches && body.contains("\"function_call_output\"") && body.contains(self.call_id)
    }
}

async fn wait_for_task_complete_without_errors(codex: &codex_core::CodexConversation) {
    loop {
        // Allow a bit more time to accommodate tool execution + retries.
        let ev = tokio::time::timeout(Duration::from_secs(10), codex.next_event())
            .await
            .expect("timeout waiting for event")
            .expect("stream ended unexpectedly");

        if matches!(ev.msg, EventMsg::Error(_)) {
            panic!("unexpected error event: {:?}", ev.msg);
        }

        if matches!(ev.msg, EventMsg::TaskComplete(_)) {
            break;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refresh_models_etag_on_412_and_retry_tool_output_request() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const ETAG_1: &str = "\"models-etag-1\"";
    const ETAG_2: &str = "\"models-etag-2\"";
    const CALL_ID: &str = "local-shell-call-1";

    let server = MockServer::start().await;

    // 1) On spawn, Codex fetches /models and stores the ETag.
    let spawn_models_mock = responses::mount_models_once_with_etag(
        &server,
        ModelsResponse { models: Vec::new() },
        ETAG_1,
    )
    .await;

    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let mut builder = test_codex()
        .with_auth(auth)
        .with_model("gpt-5")
        .with_config(|config| {
            config.features.enable(Feature::RemoteModels);
            // Keep this test deterministic: no request retries, and a small stream retry budget.
            config.model_provider.request_max_retries = Some(0);
            config.model_provider.stream_max_retries = Some(1);
        });

    let test = builder.build(&server).await?;
    let codex = Arc::clone(&test.codex);
    let cwd = Arc::clone(&test.cwd);
    let session_model = test.session_configured.model.clone();

    assert_eq!(spawn_models_mock.requests().len(), 1);
    assert_eq!(spawn_models_mock.single_request_path(), "/v1/models");

    // 2) If a /responses follow-up is rejected with 412, Codex refreshes /models to get a new tag.
    let refresh_models_mock = responses::mount_models_once_with_etag(
        &server,
        ModelsResponse { models: Vec::new() },
        ETAG_2,
    )
    .await;

    // First /responses request (user message) succeeds and returns a tool call.
    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_local_shell_call(CALL_ID, "completed", vec!["/bin/echo", "etag ok"]),
        ev_completed("resp-1"),
    ]);
    let user_turn_mock = responses::mount_sse_once_match(
        &server,
        header("X-If-Models-Match", ETAG_1),
        first_response,
    )
    .await;

    // Second /responses request (tool output) is rejected with 412 due to stale models catalog.
    let precondition_failed_mock = responses::mount_response_once_match(
        &server,
        ToolOutputWithEtagMatcher {
            expected_etag: ETAG_1,
            call_id: CALL_ID,
        },
        ResponseTemplate::new(412)
            .insert_header("content-type", "application/json")
            .set_body_string(
                r#"{"error":{"message":"models changed","type":"precondition_failed"}}"#,
            ),
    )
    .await;

    // Third /responses request retries the same tool output and now includes the new ETag.
    let completion_response = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let retried_tool_output_mock = responses::mount_sse_once_match(
        &server,
        ToolOutputWithEtagMatcher {
            expected_etag: ETAG_2,
            call_id: CALL_ID,
        },
        completion_response,
    )
    .await;

    codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "please run a tool".into(),
            }],
            final_output_json_schema: None,
            cwd: cwd.path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    wait_for_task_complete_without_errors(&codex).await;

    // Assert initial /responses includes ETag from spawn /models call.
    let user_req = user_turn_mock.single_request();
    assert_eq!(
        user_req.header("X-If-Models-Match"),
        Some(ETAG_1.to_string())
    );

    // Assert the tool output request was rejected with 412 and used the old ETag.
    let failed_req = precondition_failed_mock.single_request();
    assert_eq!(
        failed_req.header("X-If-Models-Match"),
        Some(ETAG_1.to_string())
    );
    let _ = failed_req.function_call_output(CALL_ID);

    // Assert /models was refreshed exactly once after the 412.
    assert_eq!(refresh_models_mock.requests().len(), 1);
    assert_eq!(refresh_models_mock.single_request_path(), "/v1/models");
    let refresh_req = refresh_models_mock
        .requests()
        .into_iter()
        .next()
        .expect("one request");
    // Ensure Codex includes client_version on refresh. (This is a stable signal that we're using the /models client.)
    assert!(
        refresh_req
            .url
            .query_pairs()
            .any(|(k, _)| k == "client_version"),
        "expected /models refresh to include client_version query param"
    );

    // Assert the retried tool output /responses request used the new ETag.
    let retried_req = retried_tool_output_mock.single_request();
    assert_eq!(
        retried_req.header("X-If-Models-Match"),
        Some(ETAG_2.to_string())
    );
    let _ = retried_req.function_call_output(CALL_ID);

    Ok(())
}
