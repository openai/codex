//! Snapshots a worker's retained history and unread mail across idle eviction and follow-up.

use super::*;
use codex_core::ThreadEvictionOutcome;
use codex_protocol::protocol::ThreadSettingsOverrides;
use core_test_support::ThreadIdle;
use core_test_support::responses;
use core_test_support::submit_thread_settings;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_eviction_preserves_worker_context_for_followup() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.thread_lifecycle_contributor(Arc::new(ThreadIdle));
    let mut test = test_codex()
        .with_model("gpt-5.6-sol")
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            configure_scenario_catalog(config);
            config.features.enable(Feature::Collab).unwrap();
            config.features.enable(Feature::MultiAgentV2).unwrap();
            config
                .features
                .enable(Feature::MultiAgentV2DynamicTools)
                .unwrap();
            config.multi_agent_v2.max_concurrent_threads_per_session = 2;
        })
        .build_with_auto_env(&server)
        .await?;
    // Reuse the client-tool flow to hold each worker result until the parent is idle.
    // Otherwise completion mail can race the parent's next model request in the snapshot.
    test.codex.shutdown_and_wait().await?;
    let root = test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools: vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
                name: "client_echo".to_string(),
                description: "Echo the supplied message through the client.".to_string(),
                input_schema: json!({
                    "type": "object", "properties": {"message": {"type": "string"}},
                    "required": ["message"], "additionalProperties": false,
                }),
                defer_loading: false,
            })],
            environments: Some(vec![test.executor_environment().request()]),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    test.codex = root.thread;
    test.session_configured = root.session_configured;
    let parent_id = test.session_configured.session_id.to_string();
    let mut created = test.thread_manager.subscribe_thread_created();
    let mut child_id = None;

    for (tool, arguments, answer) in [
        (
            "spawn_agent",
            json!({
                "task_name": "worker", "fork_turns": "none",
                "message": "Check who owns the API migration."
            }),
            "Mira owns the API migration.",
        ),
        (
            "send_message",
            json!({"target": "worker", "message": "Use the new API for the migration."}),
            "",
        ),
        (
            "followup_task",
            json!({"target": "worker", "message": "Continue the migration with the queued note."}),
            "I will continue with Mira using the new API.",
        ),
    ] {
        let call_id = format!("eviction-{tool}");
        responses::mount_sse_once_match(
            &server,
            wiremock::matchers::header("thread-id", parent_id.clone()),
            sse(vec![
                ev_function_call_with_namespace(
                    &call_id,
                    "collaboration",
                    tool,
                    &arguments.to_string(),
                ),
                ev_completed("parent-call"),
            ]),
        )
        .await;
        let expected_parent = parent_id.clone();
        responses::mount_sse_once_match(
            &server,
            move |request: &wiremock::Request| {
                request
                    .headers
                    .get("thread-id")
                    .is_some_and(|id| id == expected_parent.as_str())
                    && String::from_utf8_lossy(&request.body).contains(&call_id)
            },
            sse(vec![ev_completed("parent-done")]),
        )
        .await;
        if tool != "send_message" {
            let parent_id = parent_id.clone();
            responses::mount_sse_once_match(
                &server,
                move |request: &wiremock::Request| {
                    request
                        .headers
                        .get("thread-id")
                        .is_some_and(|id| id != parent_id.as_str())
                },
                sse(vec![
                    responses::ev_function_call(
                        &format!("worker-echo-{tool}"),
                        "client_echo",
                        &json!({"message": answer}).to_string(),
                    ),
                    ev_completed("worker-echo"),
                ]),
            )
            .await;
        }
        test.submit_text_turn(&format!("Run {tool} for the API migration."))
            .await?;
        ThreadIdle::wait(&test.codex).await;

        if tool == "send_message" {
            let id = child_id.expect("spawned child");
            let child = test.thread_manager.get_thread(id).await?;
            // Flush submission dispatch so the note is queued before idle cleanup starts.
            submit_thread_settings(&child, ThreadSettingsOverrides::default()).await?;
            assert_eq!(
                test.thread_manager.try_evict_v2_thread(child).await?,
                ThreadEvictionOutcome::Evicted,
            );
            assert!(test.thread_manager.get_thread(id).await.is_err());
            assert_eq!(
                responses::received_responses_requests(&server)
                    .await
                    .iter()
                    .filter(|request| request.header("thread-id").as_deref()
                        != Some(parent_id.as_str()))
                    .count(),
                2,
                "queue-only mail and eviction must not start another model turn",
            );
        } else {
            let id = created.recv().await?;
            if let Some(child_id) = child_id {
                assert_eq!(id, child_id, "the follow-up must reload the same child");
            } else {
                child_id = Some(id);
            }
            let child = test.thread_manager.get_thread(id).await?;
            let call = wait_for_event_match(&child, |event| match event {
                EventMsg::DynamicToolCallRequest(call) => Some(call.clone()),
                _ => None,
            })
            .await;
            assert_eq!(call.call_id, format!("worker-echo-{tool}"));
            responses::mount_sse_once_match(
                &server,
                wiremock::matchers::header("thread-id", id.to_string()),
                sse(vec![
                    ev_assistant_message("worker-result", answer),
                    ev_completed("worker-done"),
                ]),
            )
            .await;
            child
                .submit(Op::DynamicToolResponse {
                    id: call.call_id,
                    response: DynamicToolResponse {
                        content_items: vec![DynamicToolCallOutputContentItem::InputText {
                            text: answer.to_string(),
                        }],
                        success: true,
                    },
                })
                .await?;
            wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
            ThreadIdle::wait(&child).await;
            submit_thread_settings(&test.codex, ThreadSettingsOverrides::default()).await?;
        }
    }

    let mut requests = responses::received_responses_requests(&server).await;
    // Preserve each stream's complete history while removing parent/child scheduling order.
    requests
        .sort_by_key(|request| request.header("thread-id").as_deref() != Some(parent_id.as_str()));
    assert_eq!(requests.len(), 10);
    let history = context_snapshot::format_request_history_snapshot(
        "A parent delegates an API migration, queues a note while the worker is idle, and follows up after idle eviction. The same worker resumes with its earlier answer and the unread note. Complete model streams are grouped as parent, then worker.",
        &requests,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    // The shared formatter currently labels agent_message items without rendering their content.
    // Include the resumed worker's complete mailbox so the retained text and order are reviewable.
    let mut mailbox = responses::strip_metadata_from_json(
        responses::strip_response_item_ids_from_json(serde_json::Value::Array(
            requests
                .last()
                .expect("worker continuation")
                .inputs_of_type("agent_message"),
        )),
    );
    mailbox.sort_all_objects();
    let mailbox = serde_json::to_string_pretty(&mailbox)?;
    insta::assert_snapshot!(
        "idle_eviction_followup",
        format!("{history}\n\nResumed worker mailbox:\n{mailbox}")
    );
    test.thread_manager
        .shutdown_all_threads_bounded(Duration::from_secs(5))
        .await;
    Ok(())
}
