//! Verifies that confirmed Code Mode messaging invalidates cached approvals in both root and worker.

use super::*;
use codex_core::TurnInputRequest;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::wait_for_mcp_server;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;

fn request_body(request: &wiremock::Request) -> Option<serde_json::Value> {
    let compressed = request
        .headers
        .get("content-encoding")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("zstd"));
    let bytes = if compressed {
        zstd::stream::decode_all(std::io::Cursor::new(&request.body)).ok()?
    } else {
        request.body.clone()
    };
    serde_json::from_slice(&bytes).ok()
}

async fn allow_messaging_guardian_review(server: &wiremock::MockServer) {
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path_regex(".*/responses$"))
        .and(|request: &wiremock::Request| {
            request_body(request).is_some_and(|body| {
                body["client_metadata"]["x-openai-subagent"] == "guardian"
                    && body["input"].as_array().is_some_and(|items| {
                        items
                            .iter()
                            .filter_map(|item| item["content"].as_array())
                            .flatten()
                            .any(|item| {
                                item["text"].as_str().is_some_and(|text| {
                                    text.contains("\"tool\": \"mcp_tool_call\"")
                                        && text.contains("\"server\": \"codex_apps\"")
                                })
                            })
                    })
            })
        })
        .respond_with(responses::sse_response(responses::sse(vec![
            ev_assistant_message(
                "messaging-review-allow",
                &json!({
                    "risk_level": "low",
                    "user_authorization": "high",
                    "outcome": "allow",
                    "rationale": "The assistant is asking a question.",
                })
                .to_string(),
            ),
            ev_completed("messaging-review-allow"),
        ])))
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn confirmed_root_delivery_invalidates_root_and_worker_cached_approvals() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const TOOL: &str = "user_message_send_message";
    let messaging_server = responses::start_mock_server().await;
    let pending_delivery = Arc::new(Notify::new());
    let (release_delivery, receive_release) = std::sync::mpsc::channel();
    let pending_delivery_for_server = Arc::clone(&pending_delivery);
    let receive_release = Mutex::new(receive_release);
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/user-messaging"))
        .respond_with(move |request: &wiremock::Request| {
            let request: serde_json::Value =
                serde_json::from_slice(&request.body).expect("MCP request");
            let Some(id) = request.get("id") else {
                return wiremock::ResponseTemplate::new(/*s*/ 202);
            };
            let result = match request["method"].as_str().unwrap_or_default() {
                "initialize" => json!({
                    "protocolVersion": request["params"]["protocolVersion"],
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "user-messaging", "version": "1"}
                }),
                "tools/list" => json!({"tools": [{"name": TOOL, "inputSchema": {
                    "type": "object", "properties": {"text": {"type": "string"}}
                }, "_meta": {"connector_id": "user_message", "connector_name": "user_message"}}]}),
                "tools/call" => {
                    assert_eq!(request["params"]["name"], TOOL);
                    assert_eq!(
                        request["params"]["arguments"],
                        json!({"text": "May I deploy?"})
                    );
                    pending_delivery_for_server.notify_one();
                    receive_release
                        .lock()
                        .expect("delivery gate")
                        .recv_timeout(Duration::from_secs(/*secs*/ 20))
                        .expect("release confirmed delivery");
                    json!({"content": [{"type": "text", "text": "Message sent."}]})
                }
                "resources/list" => json!({"resources": []}),
                "resources/templates/list" => json!({"resourceTemplates": []}),
                _ => json!({}),
            };
            wiremock::ResponseTemplate::new(/*s*/ 200)
                .set_body_json(json!({"jsonrpc": "2.0", "id": id, "result": result}))
        })
        .mount(&messaging_server)
        .await;

    let (_, test, registry, thread_server) = sample_configured_conversation_history_with_delivery(
        Vec::new(),
        r#"{"path":"README.md"}"#,
        Some(TEST_GUARDIAN_POLICY),
        "",
        /*model_defaults*/ None,
        ToolCallSource::Direct,
        MessagingSetup::CodeMode(format!("{}/user-messaging", messaging_server.uri())),
    )
    .await?;
    wait_for_mcp_server(&test.codex, "codex_apps").await?;
    allow_messaging_guardian_review(&thread_server).await;
    let root_store = test.codex.thread_extension_data();
    let root_progress = root_store.get::<GuardianV2ScoreProgress>().unwrap();
    tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        while cached_score(root_store).is_none() || root_progress.inspect(/*call_id*/ None).lag > 0
        {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    let root_id = test.session_configured.thread_id;
    let mut created = test.thread_manager.subscribe_thread_created();
    let root_request = move |marker: &'static str| {
        move |request: &wiremock::Request| {
            request_body(request).is_some_and(|body| {
                body["client_metadata"]["thread_id"] == json!(root_id)
                    && body.to_string().contains(marker)
            })
        }
    };
    mount_sse_once_match(
        &thread_server,
        root_request("Start a worker"),
        sse(vec![
            ev_function_call_with_namespace(
                "spawn-worker",
                "collaboration",
                "spawn_agent",
                &json!({"message": "Inspect the deployment.", "task_name": "worker"}).to_string(),
            ),
            ev_completed("spawn-worker"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &thread_server,
        root_request("spawn-worker"),
        sse(vec![
            ev_custom_tool_call(
                "send-message",
                "exec",
                "text(await tools.mcp__codex_apps__user_message_send_message({text: 'May I deploy?'}));",
            ),
            ev_completed("send-message"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &thread_server,
        root_request("send-message"),
        sse(vec![ev_completed("root-complete")]),
    )
    .await;
    mount_sse_once_match(
        &thread_server,
        move |request: &wiremock::Request| {
            request_body(request).is_some_and(|body| {
                body["client_metadata"]["x-codex-parent-thread-id"] == json!(root_id)
                    && body["client_metadata"]["x-openai-subagent"] != "guardian"
            })
        },
        sse(vec![ev_completed("worker-complete")]),
    )
    .await;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Start a worker and ask before deployment.".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    tokio::time::timeout(ASYNC_TEST_TIMEOUT, pending_delivery.notified())
        .await
        .map_err(|_| anyhow::anyhow!("messaging delivery did not reach the MCP server"))?;
    let worker = test
        .thread_manager
        .get_thread(created.recv().await?)
        .await?;
    let mut config = test.config.clone();
    config.features.enable(Feature::GuardianV2)?;
    let session_store = ExtensionData::new("worker-cache-session");
    registry.thread_lifecycle_contributors()[0]
        .on_thread_start(ThreadStartInput {
            config: &config,
            session_source: &SessionSource::Exec,
            persistent_thread_state_available: false,
            environments: &[],
            mcp_resource_client: None,
            extension_metrics: None,
            session_store: &session_store,
            thread_store: worker.thread_extension_data(),
        })
        .await;
    let root_before = ScoreAuthorization::current(&test.codex, &Default::default()).await;
    let worker_before = ScoreAuthorization::current(&worker, &Default::default()).await;
    assert_eq!(
        worker_before.root_review_context_revision,
        Some(root_before.review_context_revision)
    );
    let mut score = cached_score(root_store).expect("initial Guardian score");
    score.scores.insert("action_risk".to_owned(), 0.25);
    set_cached_score(root_store, score);
    // Hold tool lag at zero so only the delivery can invalidate either approval.
    for (thread, before) in [(&test.codex, &root_before), (&worker, &worker_before)] {
        let store = thread.thread_extension_data();
        let progress = store.get::<GuardianV2ScoreProgress>().unwrap();
        seed_cached_score(&progress, store, /*index*/ 1_000, before.clone());
        assert_eq!(
            cached_approval(&registry, store, "review action", /*metrics*/ None).await,
            Some(ReviewDecision::Approved)
        );
    }

    release_delivery.send(())?;
    let (root_after, worker_after) = tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        loop {
            let root = ScoreAuthorization::current(&test.codex, &Default::default()).await;
            let worker = ScoreAuthorization::current(&worker, &Default::default()).await;
            if root.review_context_revision != root_before.review_context_revision
                && worker.root_review_context_revision != worker_before.root_review_context_revision
            {
                break (root, worker);
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(
        root_after,
        ScoreAuthorization {
            review_context_revision: root_after.review_context_revision,
            ..root_before
        }
    );
    assert_eq!(
        worker_after,
        ScoreAuthorization {
            root_review_context_revision: Some(root_after.review_context_revision),
            ..worker_before
        }
    );
    for thread in [&test.codex, &worker] {
        assert_eq!(
            cached_approval(
                &registry,
                thread.thread_extension_data(),
                "review action",
                /*metrics*/ None
            )
            .await,
            None
        );
    }
    // Missing retained root instructions must not permanently veto a matching LOW score.
    ThreadIdle::wait(&test.codex).await;
    ThreadIdle::wait(&worker).await;
    test.codex
        .inject_response_items(
            (0..20)
                .map(|index| user_instruction(&format!("Root instruction {index}.")))
                .collect(),
        )
        .await?;
    let snapshot = worker
        .guardian_root_snapshot()
        .await
        .expect("root snapshot");
    assert!(!snapshot.authorization_version.retained_context_complete);
    assert!(
        snapshot
            .messages
            .contains(&codex_core::GuardianRootMessage::IncompleteRootInstructions)
    );
    let store = worker.thread_extension_data();
    let progress = store.get::<GuardianV2ScoreProgress>().unwrap();
    let authorization = ScoreAuthorization::current(&worker, &Default::default()).await;
    assert!(authorization.local.retained_context_complete);
    seed_cached_score(&progress, store, /*index*/ 1_000, authorization);
    assert_eq!(
        cached_approval(&registry, store, "review action", /*metrics*/ None).await,
        Some(ReviewDecision::Approved)
    );
    let shutdown = test
        .thread_manager
        .shutdown_all_threads_bounded(ASYNC_TEST_TIMEOUT)
        .await;
    assert!(shutdown.timed_out.is_empty());
    Ok(())
}
