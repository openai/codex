//! Exercises reviewer ownership through app-server, including concurrent and inline reviews.

use super::*;
use app_test_support::create_final_assistant_message_sse_response as assistant_response;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ReviewDelivery;
use codex_app_server_protocol::ReviewStartParams;
use codex_app_server_protocol::ReviewStartResponse;
use codex_app_server_protocol::ReviewTarget;
use codex_app_server_protocol::SessionSource;
use codex_app_server_protocol::ThreadClosedNotification;
use codex_app_server_protocol::ThreadListResponse;
use codex_app_server_protocol::ThreadReadParams;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadSourceKind;
use codex_app_server_protocol::ThreadUnsubscribeParams;
use codex_app_server_protocol::ThreadUnsubscribeResponse;
use codex_app_server_protocol::TurnInterruptParams;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_protocol::protocol::SubAgentSource;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;
use test_case::test_case;
use tokio::sync::oneshot;

const ALLOW: &str = r#"{"outcome":"allow","rationale":"completed seed assessment"}"#;

fn sync_config(responses_url: &str) -> MockResponsesConfig {
    MockResponsesConfig::new(responses_url)
        .with_provider_config("supports_websockets = false")
        .with_approval_policy("on-request")
        .with_root_config("approvals_reviewer = \"auto_review\"\nthread_unload_delay_secs = 0")
        .with_extra_config("[features.guardianv2]\nenabled = false")
        .enable_feature(Feature::GuardianApproval)
        .disable_feature(Feature::EnableRequestCompression)
}

fn tool_response(server: &str, tool: &str, calls: &[&str]) -> String {
    let mut events = vec![responses::ev_response_created("tools")];
    events.extend(calls.iter().map(|call| {
        responses::ev_function_call_with_namespace(
            call,
            &format!("mcp__{server}"),
            tool,
            &json!({"message": call}).to_string(),
        )
    }));
    events.push(responses::ev_completed("tools"));
    responses::sse(events)
}

#[derive(Clone, Copy)]
enum ReviewEnding {
    Complete,
    Interrupt,
}

#[test_case(ReviewEnding::Complete; "completed reviews")]
#[test_case(ReviewEnding::Interrupt; "cancelled concurrent reviews")]
#[tokio::test]
async fn managed_reviewers_reuse_fork_and_resume_after_parent_shutdown(
    ending: ReviewEnding,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let (continue_parent, parent_gate) = oneshot::channel();
    let (finish_first, first_gate) = oneshot::channel();
    let (finish_second, second_gate) = oneshot::channel();
    let mut replies = vec![
        (
            tool_response(TEST_SERVER_NAME, TEST_TOOL_NAME, &["seed"]),
            None,
        ),
        (assistant_response(ALLOW)?, None),
        (
            tool_response(TEST_SERVER_NAME, TEST_TOOL_NAME, &["first", "second"]),
            Some(parent_gate),
        ),
        (assistant_response(ALLOW)?, Some(first_gate)),
        (assistant_response(ALLOW)?, Some(second_gate)),
    ];
    if matches!(ending, ReviewEnding::Complete) {
        replies.push((assistant_response("Done.")?, None));
    }
    replies.push((assistant_response("The user authorized the checks.")?, None));
    let (server, _completions) = start_streaming_sse_server(
        replies
            .into_iter()
            .map(|(body, gate)| vec![StreamingSseChunk { gate, body }])
            .collect(),
    )
    .await;
    let (mcp_url, mcp_server) = start_mcp_server(/*sensitive_action*/ None).await?;
    let codex_home = TempDir::new()?;
    std::fs::write(
        codex_home.path().join("AGENTS.md"),
        "Keep this project private.",
    )?;
    sync_config(server.uri())
        .with_extra_config(&format!(
            "[mcp_servers.{TEST_SERVER_NAME}]\nurl = \"{mcp_url}/mcp\"\ndefault_tools_approval_mode = \"prompt\""
        ))
        .write(codex_home.path())?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    let parent = app.start_thread(ThreadStartParams::default()).await?.thread;
    let started: TurnStartResponse = app
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: parent.id.clone(),
                input: vec![UserInput::Text {
                    text: "Run the three checks.".to_owned(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

    // The seed review finished; pause the parent before it requests concurrent approvals.
    timeout(TIMEOUT, server.wait_for_request_count(/*count*/ 3)).await?;
    let seed: Value = serde_json::from_slice(&server.requests().await[1])?;
    assert!(
        !seed["tools"]
            .to_string()
            .contains(&format!("mcp__{TEST_SERVER_NAME}")),
        "reviewers must not inherit the parent's configured MCP tools"
    );
    let reviewer_id = seed["client_metadata"]["thread_id"]
        .as_str()
        .expect("reviewer ID")
        .to_owned();
    let id = app
        .send_thread_resume_request(ThreadResumeParams {
            thread_id: reviewer_id.clone(),
            ..Default::default()
        })
        .await?;
    let error = timeout(
        TIMEOUT,
        app.read_stream_until_error_message(RequestId::Integer(id)),
    )
    .await??;
    assert_eq!(error.error, JSONRPCErrorError {
        code: -32600,
        message: "cannot resume a live Guardian reviewer; use thread/read to inspect it, or resume after its parent is unloaded".to_owned(),
        data: None,
    });
    // Client removal must leave the cached reviewer available for the next approvals.
    for method in ["thread/archive", "thread/delete"] {
        let id = app
            .send_request(method, Some(json!({"threadId": reviewer_id})))
            .await?;
        let error = timeout(
            TIMEOUT,
            app.read_stream_until_error_message(RequestId::Integer(id)),
        )
        .await??;
        assert_eq!(
            error.error,
            JSONRPCErrorError {
                code: -32600,
                message: "live internal threads can only be removed by their owner".to_owned(),
                data: None,
            }
        );
    }
    continue_parent.send(()).expect("continue parent");

    // Neither review can finish until both requests arrive: prove reuse and an independent fork.
    timeout(TIMEOUT, server.wait_for_request_count(/*count*/ 5)).await?;
    let requests = server.requests().await;
    let first: Value = serde_json::from_slice(&requests[3])?;
    let second: Value = serde_json::from_slice(&requests[4])?;
    let concurrent_ids = [
        &first["client_metadata"]["thread_id"],
        &second["client_metadata"]["thread_id"],
    ];
    assert_ne!(concurrent_ids[0], concurrent_ids[1]);
    assert!(concurrent_ids.contains(&&seed["client_metadata"]["thread_id"]));
    for review in [&seed, &first, &second] {
        assert_eq!(
            review["prompt_cache_key"],
            format!("guardian:{}", parent.id)
        );
        let metadata: Value = serde_json::from_str(
            review["client_metadata"]["x-codex-turn-metadata"]
                .as_str()
                .expect("turn metadata"),
        )?;
        assert_eq!(metadata["subagent_kind"], "guardian");
        assert!(
            review["input"]
                .to_string()
                .contains("Keep this project private.")
        );
    }
    for review in [&first, &second] {
        assert!(
            review["input"]
                .to_string()
                .contains("completed seed assessment")
        );
    }
    if matches!(ending, ReviewEnding::Interrupt) {
        let _: TurnInterruptResponse = app
            .request(|request_id| ClientRequest::TurnInterrupt {
                request_id,
                params: TurnInterruptParams {
                    thread_id: parent.id.clone(),
                    turn_id: started.turn.id.clone(),
                },
            })
            .await?;
    } else {
        finish_first.send(()).expect("finish first review");
        finish_second.send(()).expect("finish second review");
    }
    let completed: TurnCompletedNotification =
        timeout(TIMEOUT, app.read_notification("turn/completed")).await??;
    let expected_status = match ending {
        ReviewEnding::Complete => TurnStatus::Completed,
        ReviewEnding::Interrupt => TurnStatus::Interrupted,
    };
    assert_eq!(completed.turn.status, expected_status);

    let read: ThreadReadResponse = app
        .request(|request_id| ClientRequest::ThreadRead {
            request_id,
            params: ThreadReadParams {
                thread_id: reviewer_id.clone(),
                include_turns: true,
            },
        })
        .await?;
    assert_eq!(
        read.thread.source,
        SessionSource::SubAgent(SubAgentSource::Other("guardian".to_owned()))
    );
    let _: ThreadUnsubscribeResponse = app
        .request(|request_id| ClientRequest::ThreadUnsubscribe {
            request_id,
            params: ThreadUnsubscribeParams {
                thread_id: parent.id.clone(),
            },
        })
        .await?;
    let closed: ThreadClosedNotification =
        timeout(TIMEOUT, app.read_notification("thread/closed")).await??;
    assert_eq!(closed.thread_id, parent.id);
    // Saved reviewers remain discoverable through the existing subagent filters.
    for kind in [ThreadSourceKind::SubAgent, ThreadSourceKind::SubAgentOther] {
        let params = serde_json::from_value(json!({"sourceKinds": [kind]}))?;
        let listed: ThreadListResponse = app
            .request(|request_id| ClientRequest::ThreadList { request_id, params })
            .await?;
        assert!(listed.data.iter().any(|thread| thread.id == reviewer_id));
    }
    let resumed: ThreadResumeResponse = app
        .request(|request_id| ClientRequest::ThreadResume {
            request_id,
            params: ThreadResumeParams {
                thread_id: reviewer_id.clone(),
                ..Default::default()
            },
        })
        .await?;
    assert_eq!(resumed.thread.id, reviewer_id);
    assert_eq!(resumed.thread.source, read.thread.source);
    let completed = timeout(
        TIMEOUT,
        app.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: reviewer_id.clone(),
            input: vec![UserInput::Text {
                text: "Explain your assessment.".to_owned(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    assert_eq!(
        server.requests().await.len(),
        match ending {
            ReviewEnding::Complete => 7,
            ReviewEnding::Interrupt => 6,
        }
    );
    // Once the owner releases it, the resumed reviewer follows normal client removal.
    for method in ["thread/archive", "thread/delete"] {
        let id = app
            .send_request(method, Some(json!({"threadId": reviewer_id})))
            .await?;
        let response: Value = timeout(TIMEOUT, app.read_response(id)).await??;
        assert_eq!(response, json!({}));
    }
    app.shutdown_gracefully().await?;
    mcp_server.abort();
    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn inline_review_delegate_runs_strict_guardian_assessment() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let requests = responses::mount_sse_sequence(
        &server,
        vec![
            tool_response("node_repl", "js", &["inline-tool"]),
            assistant_response(ALLOW)?,
            assistant_response(r#"{"findings":[]}"#)?,
        ],
    )
    .await;
    let (mcp_url, mcp_server) =
        start_mcp_server_with_tools(&["js"], /*sensitive_action*/ None).await?;
    let codex_home = TempDir::new()?;
    sync_config(&server.uri())
        .with_extra_config(&format!("[mcp_servers.node_repl]\nurl = \"{mcp_url}/mcp\"\ndefault_tools_approval_mode = \"auto\""))
        .write(codex_home.path())?;
    let config = load_default_config_for_test(&codex_home).await;
    let mut model = codex_core::test_support::construct_model_info_offline(MODEL, &config);
    // Strict MCP approvals must work under the inline delegate's `never` policy.
    model.node_repl_auto_review_required = true;
    write_models_cache_with_models(codex_home.path(), vec![model]).await?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized_with_timeout(TIMEOUT)
        .await?;
    let parent = app.start_thread(ThreadStartParams::default()).await?.thread;
    let _: ReviewStartResponse = app
        .request(|request_id| ClientRequest::ReviewStart {
            request_id,
            params: ReviewStartParams {
                thread_id: parent.id,
                delivery: Some(ReviewDelivery::Inline),
                target: ReviewTarget::Custom {
                    instructions: "Run the configured tool to check this change.".to_owned(),
                },
            },
        })
        .await?;
    let completed: TurnCompletedNotification =
        timeout(TIMEOUT, app.read_notification("turn/completed")).await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    let output = requests
        .function_call_output_text("inline-tool")
        .expect("MCP tool output");
    assert!(output.contains(r#""echoed":"inline-tool""#), "{output}");
    assert_eq!(
        requests
            .requests()
            .iter()
            .map(|request| request.body_json()["client_metadata"]["x-openai-subagent"].clone())
            .collect::<Vec<_>>(),
        vec![json!("review"), json!("guardian"), json!("review")]
    );
    app.shutdown_gracefully().await?;
    mcp_server.abort();
    Ok(())
}
