//! Root communications select worker-specific windows without rewriting worker prompts.

use super::*;
use codex_core::CodexThread;
use pretty_assertions::assert_eq;

async fn root_messages(thread: &CodexThread) -> Vec<GuardianRootMessage> {
    thread
        .guardian_root_snapshot()
        .await
        .expect("worker root snapshot")
        .messages
        .into_iter()
        .filter(|message| {
            matches!(
                message,
                GuardianRootMessage::User(_)
                    | GuardianRootMessage::Assistant(_)
                    | GuardianRootMessage::UserInput(_)
            )
        })
        .collect()
}

async fn handoff(
    test: &TestCodex,
    server: &wiremock::MockServer,
    prompt: &'static str,
    call_id: &'static str,
    tool: &str,
    arguments: Value,
) -> Result<()> {
    let root = test.session_configured.thread_id;
    mount_sse_once_match(
        server,
        move |request: &wiremock::Request| {
            is_root_request(request, root)
                && contains_text(request, prompt)
                && !has_call_output(request, call_id)
        },
        sse(vec![
            ev_function_call_with_namespace(call_id, "collaboration", tool, &arguments.to_string()),
            ev_completed(call_id),
        ]),
    )
    .await;
    mount_completion(server, root, call_id).await;
    test.submit_text_turn(prompt).await?;
    ThreadIdle::wait(&test.codex).await;
    Ok(())
}

pub(crate) async fn handoff_scenario() -> Result<Vec<ResponsesRequest>> {
    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(Arc::new(ThreadIdle));
    let mut test = test_codex()
        .with_model("gpt-5.5")
        .with_model_info_override("gpt-5.5", |model| {
            model.multi_agent_version = Some(codex_protocol::protocol::MultiAgentVersion::V2);
        })
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            for feature in [
                Feature::Collab,
                Feature::MultiAgentV2,
                Feature::GuardianRootHandoffContext,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable multi-agent feature");
            }
            config.multi_agent_v2.max_concurrent_threads_per_session = 4;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("set read-only permissions");
        })
        .build_with_auto_env(&server)
        .await?;
    let root = test.session_configured.thread_id;
    let mut created = test.thread_manager.subscribe_thread_created();
    // Ordinary root/worker continuations idle; the reviewed action has its own mock.
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/v1/responses"))
        .and(|request: &wiremock::Request| {
            request_body(request)
                .is_some_and(|body| body["client_metadata"]["x-openai-subagent"] != "guardian")
        })
        .respond_with(sse_response(sse(vec![ev_completed("idle")])))
        .with_priority(/*p*/ 20)
        .mount(&server)
        .await;
    test.codex
        .inject_response_items(vec![serde_json::from_value(
            ev_assistant_message("question", "Inspect the private deployment?")["item"].clone(),
        )?])
        .await?;
    mount_sse_once_match(
        &server,
        move |request: &wiremock::Request| {
            is_worker_request(request, root)
                && contains_text(request, "Delegate to leaf.")
                && !has_call_output(request, "spawn-leaf")
        },
        sse(vec![
            ev_function_call_with_namespace(
                "spawn-leaf",
                "collaboration",
                "spawn_agent",
                &json!({"fork_turns":"none", "message":"Inspect.", "task_name":"leaf"}).to_string(),
            ),
            ev_completed("spawn-leaf"),
        ]),
    )
    .await;
    handoff(
        &test,
        &server,
        "Inspect the deployment.",
        "spawn-alpha",
        "spawn_agent",
        json!({"task_name":"alpha", "message":"Delegate to leaf.", "fork_turns":"none"}),
    )
    .await?;
    let alpha = test
        .thread_manager
        .get_thread(created.recv().await?)
        .await?;
    let leaf = test
        .thread_manager
        .get_thread(created.recv().await?)
        .await?;
    for worker in [&alpha, &leaf] {
        wait_for_event(worker, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        ThreadIdle::wait(worker).await;
    }
    let mut expected = vec![
        GuardianRootMessage::Assistant("Inspect the private deployment?".to_owned()),
        GuardianRootMessage::User("Inspect the deployment.".to_owned()),
    ];
    assert_eq!(root_messages(&leaf).await, expected);
    test.codex
        .inject_response_items(vec![serde_json::from_value(
            ev_assistant_message("metrics", "Inspect the metrics too?")["item"].clone(),
        )?])
        .await?;
    handoff(
        &test,
        &server,
        "Inspect the metrics.",
        "spawn-beta",
        "spawn_agent",
        json!({"task_name":"beta", "message":"Inspect.", "fork_turns":"none"}),
    )
    .await?;
    let beta = test
        .thread_manager
        .get_thread(created.recv().await?)
        .await?;
    wait_for_event(&beta, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    ThreadIdle::wait(&beta).await;
    let mut with_latest = expected.clone();
    with_latest.extend([
        GuardianRootMessage::Assistant("Inspect the metrics too?".to_owned()),
        GuardianRootMessage::User("Inspect the metrics.".to_owned()),
    ]);
    assert_eq!(root_messages(&alpha).await, with_latest);
    let mut beta_messages = root_messages(&beta).await;

    // Root-to-ancestor communication updates the descendant immediately, without a relay.
    test.codex
        .inject_response_items(vec![serde_json::from_value(
            ev_assistant_message("private", "Keep the deployment private?")["item"].clone(),
        )?])
        .await?;
    handoff(
        &test,
        &server,
        "Keep the deployment private.",
        "send-alpha",
        "send_message",
        json!({"target":"alpha", "message":"Keep it private."}),
    )
    .await?;
    expected.extend([
        GuardianRootMessage::User("Inspect the metrics.".to_owned()),
        GuardianRootMessage::Assistant("Keep the deployment private?".to_owned()),
        GuardianRootMessage::User("Keep the deployment private.".to_owned()),
    ]);
    assert_eq!(root_messages(&leaf).await, expected);
    beta_messages.extend([
        GuardianRootMessage::Assistant("Keep the deployment private?".to_owned()),
        GuardianRootMessage::User("Keep the deployment private.".to_owned()),
    ]);
    assert_eq!(root_messages(&beta).await, beta_messages);

    // The latest window includes a cancellation before root communicates with this branch again.
    test.codex.inject_response_items(vec![serde_json::from_value(json!({
        "type":"message", "id":"cancel", "role":"user", "content":[{"type":"input_text", "text":"Cancel the deployment."}]
    }))?]).await?;
    expected.push(GuardianRootMessage::User(
        "Cancel the deployment.".to_owned(),
    ));
    assert_eq!(root_messages(&leaf).await, expected);
    // Later status questions do not revoke the cancellation, even outside the recent window.
    for prompt in [
        "How many deployment checks are pending?",
        "How long have they been running?",
        "What is the current throughput?",
    ] {
        test.submit_text_turn(prompt).await?;
        expected.push(GuardianRootMessage::User(prompt.to_owned()));
    }
    assert_eq!(root_messages(&leaf).await, expected);
    let alpha_id = alpha.startup_metadata().thread_id;
    mount_sse_once_match(&server, move |request: &wiremock::Request| {
        is_root_request(request, alpha_id) && contains_text(request, "Check.") && !has_call_output(request, "check")
    }, sse(vec![ev_function_call("check", "exec_command", &json!({"cmd":"exit 0", "justification":"Check the deployment.", "sandbox_permissions":"require_escalated"}).to_string()), ev_completed("check")])).await;
    let review_server = start_mock_server().await;
    mount_response_once_match(
        &server,
        |request: &wiremock::Request| {
            request_body(request)
                .is_some_and(|body| body["client_metadata"]["x-openai-subagent"] == "guardian")
        },
        wiremock::ResponseTemplate::new(/*s*/ 307)
            .insert_header("location", format!("{}/v1/responses", review_server.uri())),
    )
    .await;
    let review = core_test_support::responses::mount_sse_once(&review_server, sse(vec![
        ev_assistant_message("assessment", &json!({"risk_level":"high","user_authorization":"low","outcome":"deny","rationale":"The user cancelled the deployment."}).to_string()), ev_completed("review-done")
    ])).await;
    handoff(
        &test,
        &server,
        "Check the deployment status.",
        "followup-alpha",
        "followup_task",
        json!({"target":"alpha", "message":"Check."}),
    )
    .await?;
    wait_for_event(&alpha, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    ThreadIdle::wait(&alpha).await;
    expected.push(GuardianRootMessage::User(
        "Check the deployment status.".to_owned(),
    ));
    assert_eq!(root_messages(&leaf).await, expected);
    // Beta's latest window advances even though this follow-up only targets alpha.
    beta_messages.remove(/*index*/ 3);
    beta_messages.extend([
        GuardianRootMessage::User("Cancel the deployment.".to_owned()),
        GuardianRootMessage::User("How many deployment checks are pending?".to_owned()),
        GuardianRootMessage::User("How long have they been running?".to_owned()),
        GuardianRootMessage::User("What is the current throughput?".to_owned()),
        GuardianRootMessage::User("Check the deployment status.".to_owned()),
    ]);
    assert_eq!(root_messages(&beta).await, beta_messages);
    assert!(
        review
            .single_request()
            .body_json()
            .to_string()
            .contains("user: Cancel the deployment.")
    );

    let requests = core_test_support::responses::received_responses_requests(&review_server).await;
    // Compaction can remove every recognizable handoff. Fall back to the original root reader.
    let history = test.codex.conversation_history_snapshot().await;
    let checkpoint: CompactedItem = serde_json::from_value(json!({
        "message":"Root checkpoint.", "replacement_history":[], "retained_context":history.retained_context(),
    }))?;
    test.codex
        .append_rollout_items(&[RolloutItem::Compacted(checkpoint)])
        .await?;
    test.codex.flush_rollout().await?;
    let rollout = test
        .codex
        .load_history(/*include_archived*/ false)
        .await?
        .items;
    test.codex =
        crate::suite::guardian_checkpoint_migration::resume(&test, &test.codex, rollout).await?;
    expected.insert(
        /*index*/ 2,
        GuardianRootMessage::Assistant("Inspect the metrics too?".to_owned()),
    );
    assert_eq!(root_messages(&leaf).await, expected);

    let shutdown = test
        .thread_manager
        .shutdown_all_threads_bounded(Duration::from_secs(/*secs*/ 10))
        .await;
    assert!(shutdown.timed_out.is_empty());
    Ok(requests)
}
