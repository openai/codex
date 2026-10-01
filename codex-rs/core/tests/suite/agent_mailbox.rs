//! Pending mail survives eviction without loading idle recipients.

use super::*;
use crate::suite::settings_commits::COMMITTED_MODEL;
use crate::suite::settings_commits::PauseAfterCommit;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_protocol::protocol::AgentStatus;
use codex_protocol::protocol::Op;
use core_test_support::ThreadIdle;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use tokio::sync::oneshot;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_mail_survives_eviction_and_preserves_followup_order() -> Result<()> {
    const MAIL: &str = "Keep this note for later";
    const LATE_MAIL: &str = "Another note while unloaded";
    const FOLLOWUP: &str = "Read the pending note";
    const AFTER_FOLLOWUP: &str = "This note must follow the new task";
    let server = start_mock_server().await;
    mount_root_collaboration_call(
        &server,
        FIRST_PROMPT,
        "first-call",
        "spawn_agent",
        json!({ "message": FIRST_TASK, "task_name": "first", "fork_turns": "none" }),
    )
    .await;
    mount_completed_worker(&server, FIRST_TASK, "first-call").await;
    let (entered_tx, entered_rx) = oneshot::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.config_contributor(Arc::new(PauseAfterCommit {
        gate: Mutex::new(Some((entered_tx, release_rx))),
    }));
    extensions.thread_lifecycle_contributor(Arc::new(ThreadIdle));
    let test = test_codex()
        .with_model("gpt-5.6-sol")
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.features.enable(Feature::Collab).unwrap();
            config.features.enable(Feature::MultiAgentV2).unwrap();
            config.multi_agent_v2.max_concurrent_threads_per_session = 2;
            config.multi_agent_v2.hide_spawn_agent_metadata = true;
        })
        .build_with_auto_env(&server)
        .await?;
    let mut created = test.thread_manager.subscribe_thread_created();
    test.submit_turn(FIRST_PROMPT).await?;
    ThreadIdle::wait(&test.codex).await;
    let first_id = created.recv().await?;
    let first = test.thread_manager.get_thread(first_id).await?;
    wait_for_event(&first, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    ThreadIdle::wait(&first).await;

    mount_root_collaboration_call(
        &server,
        "send a note",
        "queued-mail",
        "send_message",
        json!({ "target": "first", "message": MAIL }),
    )
    .await;
    test.submit_turn("send a note").await?;
    ThreadIdle::wait(&test.codex).await;
    // Wait for delivery through the submission queue before attempting eviction.
    submit_thread_settings(&first, ThreadSettingsOverrides::default()).await?;
    assert_eq!(
        first.agent_status().await,
        AgentStatus::Completed(Some("worker completed".into()))
    );
    drop(first);

    mount_root_collaboration_call(
        &server,
        "spawn while child is idle",
        "replacement-spawn",
        "spawn_agent",
        json!({ "message": SECOND_TASK, "task_name": "replacement", "fork_turns": "none" }),
    )
    .await;
    mount_completed_worker(&server, SECOND_TASK, "replacement-spawn").await;
    test.submit_turn("spawn while child is idle").await?;
    ThreadIdle::wait(&test.codex).await;
    let replacement_id = created.recv().await?;
    let replacement = test.thread_manager.get_thread(replacement_id).await?;
    wait_for_event(&replacement, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    ThreadIdle::wait(&replacement).await;
    assert!(test.thread_manager.get_thread(first_id).await.is_err());
    assert_eq!(test.thread_manager.list_thread_ids().await.len(), 2);
    mount_root_collaboration_call(
        &server,
        "send while unloaded",
        "late-mail",
        "send_message",
        json!({ "target": "first", "message": LATE_MAIL }),
    )
    .await;
    test.submit_turn("send while unloaded").await?;
    ThreadIdle::wait(&test.codex).await;
    assert!(test.thread_manager.get_thread(first_id).await.is_err());

    // Reload without starting a turn, then hold dispatch while both tools submit.
    // The older queued notes must precede the follow-up, and the newer note must follow it.
    test.thread_manager
        .ensure_multi_agent_v2_child_loaded(first_id)
        .await?;
    let resumed = test.thread_manager.get_thread(first_id).await?;
    resumed
        .submit(Op::ThreadSettings {
            thread_settings: ThreadSettingsOverrides {
                model: Some(COMMITTED_MODEL.to_string()),
                ..Default::default()
            },
            reply: None,
        })
        .await?;
    entered_rx.await?;
    mount_root_collaboration_call(
        &server,
        "drain the note",
        "followup",
        "followup_task",
        json!({ "target": "first", "message": FOLLOWUP }),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, FOLLOWUP) && !has_function_call_output(request, "followup")
        },
        sse(vec![
            ev_response_created("worker-wait"),
            ev_function_call_with_namespace(
                "wait-note",
                MULTI_AGENT_V2_NAMESPACE,
                "wait_agent",
                r#"{"timeout_ms":1}"#,
            ),
            ev_completed("worker-wait"),
        ]),
    )
    .await;
    let delivered = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| has_function_call_output(request, "wait-note"),
        sse(vec![
            ev_assistant_message("done", "Read all notes"),
            ev_completed("done"),
        ]),
    )
    .await;
    test.submit_turn("drain the note").await?;
    ThreadIdle::wait(&test.codex).await;
    mount_root_collaboration_call(
        &server,
        "send after followup",
        "after-followup",
        "send_message",
        json!({ "target": "first", "message": AFTER_FOLLOWUP }),
    )
    .await;
    test.submit_turn("send after followup").await?;
    ThreadIdle::wait(&test.codex).await;
    release_tx.send(())?;
    wait_for_event(&resumed, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    ThreadIdle::wait(&resumed).await;
    let request = delivered
        .requests()
        .into_iter()
        .find(|request| request.function_call_output_text("wait-note").is_some())
        .expect("worker continuation after waiting for mail");
    let messages = request.inputs_of_type("agent_message");
    let expected = [MAIL, LATE_MAIL, FOLLOWUP, AFTER_FOLLOWUP];
    let received = messages
        .iter()
        .filter_map(|item| {
            expected
                .iter()
                .find(|text| item.to_string().contains(**text))
                .copied()
        })
        .collect::<Vec<_>>();
    assert_eq!(received, expected);
    assert_eq!(test.thread_manager.list_thread_ids().await.len(), 2);

    test.thread_manager
        .shutdown_all_threads_bounded(Duration::from_secs(5))
        .await;
    Ok(())
}
