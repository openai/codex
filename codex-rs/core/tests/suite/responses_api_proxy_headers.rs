//! Verifies that parent and spawned subagent Responses API requests carry the expected window,
//! parent-thread, and subagent identity headers.

use anyhow::Result;
use anyhow::anyhow;
use codex_features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;

const PARENT_PROMPT: &str = "spawn a subagent and report when it is started";
const CHILD_PROMPT: &str = "child: say done";
const SPAWN_CALL_ID: &str = "spawn-call-1";
const REQUEST_POLL_INTERVAL: Duration = Duration::from_millis(/*millis*/ 20);
const TURN_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 60);
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn responses_api_parent_and_subagent_requests_include_identity_headers() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let spawn_args = serde_json::to_string(&json!({ "message": CHILD_PROMPT }))?;
    let parent_mock = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            request_body_contains(req, PARENT_PROMPT)
                && request_header(req, "x-openai-subagent").is_none()
        },
        sse(vec![
            ev_response_created("resp-parent-1"),
            ev_function_call(SPAWN_CALL_ID, "spawn_agent", &spawn_args),
            ev_completed("resp-parent-1"),
        ]),
    )
    .await;
    let child_mock = mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            request_body_contains(req, CHILD_PROMPT)
                && !request_body_contains(req, SPAWN_CALL_ID)
                && request_header(req, "x-openai-subagent") == Some("collab_spawn")
        },
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_assistant_message("msg-child-1", "child done"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |req: &wiremock::Request| {
            request_body_contains(req, SPAWN_CALL_ID)
                && request_header(req, "x-openai-subagent").is_none()
        },
        sse(vec![
            ev_response_created("resp-parent-2"),
            ev_assistant_message("msg-parent-2", "parent done"),
            ev_completed("resp-parent-2"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .disable(Feature::EnableRequestCompression)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await?;
    submit_turn_with_timeout(&test, PARENT_PROMPT).await?;

    let parent = wait_for_matching_request(&parent_mock, "parent request", |request| {
        request.body_contains_text(PARENT_PROMPT) && request.header("x-openai-subagent").is_none()
    })
    .await?;
    let child = wait_for_matching_request(&child_mock, "child request", |request| {
        request.body_contains_text(CHILD_PROMPT)
            && !request.body_contains_text(SPAWN_CALL_ID)
            && request.header("x-openai-subagent").as_deref() == Some("collab_spawn")
    })
    .await?;

    let parent_window_id = parent
        .header("x-codex-window-id")
        .ok_or_else(|| anyhow!("parent request missing x-codex-window-id"))?;
    let child_window_id = child
        .header("x-codex-window-id")
        .ok_or_else(|| anyhow!("child request missing x-codex-window-id"))?;
    let (parent_thread_id, parent_generation) = split_window_id(&parent_window_id)?;
    let (child_thread_id, child_generation) = split_window_id(&child_window_id)?;

    assert_eq!(parent_generation, 0);
    assert_eq!(child_generation, 0);
    assert!(child_thread_id != parent_thread_id);
    assert_eq!(parent.header("x-openai-subagent"), None);
    assert_eq!(
        child.header("x-openai-subagent").as_deref(),
        Some("collab_spawn")
    );
    assert_eq!(
        child.header("x-codex-parent-thread-id").as_deref(),
        Some(parent_thread_id)
    );

    Ok(())
}

async fn submit_turn_with_timeout(test: &TestCodex, prompt: &str) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    test.codex
        .submit(Op::UserTurn {
            environments: None,
            items: vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.config.cwd.to_path_buf(),
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: None,
            sandbox_policy: SandboxPolicy::WorkspaceWrite {
                writable_roots: Vec::new(),
                read_only_access: Default::default(),
                network_access: false,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            },
            permission_profile: None,
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let turn_started = wait_for_event_result(test, "turn started", |event| {
        matches!(event, EventMsg::TurnStarted(_))
    })
    .await?;
    let EventMsg::TurnStarted(turn_started) = turn_started else {
        unreachable!("event predicate only matches turn started events");
    };
    wait_for_event_result(test, "turn complete", |event| match event {
        EventMsg::TurnComplete(event) => event.turn_id == turn_started.turn_id,
        _ => false,
    })
    .await?;

    Ok(())
}

async fn wait_for_matching_request<F>(
    mock: &ResponseMock,
    label: &str,
    mut predicate: F,
) -> Result<ResponsesRequest>
where
    F: FnMut(&ResponsesRequest) -> bool,
{
    tokio::time::timeout(TURN_TIMEOUT, async {
        loop {
            if let Some(request) = mock
                .requests()
                .into_iter()
                .find(|request| predicate(request))
            {
                return request;
            }
            tokio::time::sleep(REQUEST_POLL_INTERVAL).await;
        }
    })
    .await
    .map_err(|_| anyhow!("timed out waiting for {label}"))
}

async fn wait_for_event_result<F>(
    test: &TestCodex,
    stage: &str,
    mut predicate: F,
) -> Result<EventMsg>
where
    F: FnMut(&EventMsg) -> bool,
{
    let mut seen_events = Vec::new();
    tokio::time::timeout(TURN_TIMEOUT, async {
        loop {
            let event = test.codex.next_event().await?;
            seen_events.push(event_summary(&event.msg));
            if predicate(&event.msg) {
                return Ok::<EventMsg, anyhow::Error>(event.msg);
            }
        }
    })
    .await
    .map_err(|_| {
        anyhow!(
            "timed out waiting for {stage}; saw events: {}",
            seen_events.join(" | ")
        )
    })?
}

fn event_summary(event: &EventMsg) -> String {
    let mut summary = format!("{event:?}");
    summary.truncate(240);
    summary
}

fn request_body_contains(req: &wiremock::Request, text: &str) -> bool {
    std::str::from_utf8(&req.body).is_ok_and(|body| body.contains(text))
}

fn request_header<'a>(req: &'a wiremock::Request, name: &str) -> Option<&'a str> {
    req.headers.get(name).and_then(|value| value.to_str().ok())
}

fn split_window_id(window_id: &str) -> Result<(&str, u64)> {
    let (thread_id, generation) = window_id
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("invalid window id header: {window_id}"))?;
    Ok((thread_id, generation.parse::<u64>()?))
}
