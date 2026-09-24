//! Accepted mail pins a worker through dispatch; cancelled eviction still owns teardown.

use super::*;
use crate::suite::settings_commits::COMMITTED_MODEL;
use crate::suite::settings_commits::PauseAfterCommit;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStopInput;
use codex_extension_api::ToolLifecycleContributor;
use codex_extension_api::ToolLifecycleFuture;
use codex_extension_api::ToolStartInput;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::ThreadIdle;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use tokio::sync::Notify;
use tokio::sync::oneshot;
use tokio::time::timeout;

#[derive(Default)]
struct ShutdownGate {
    entered: Notify,
    release: Notify,
}

#[derive(Default)]
struct PauseShutdown {
    delivery_started: Notify,
}

impl ToolLifecycleContributor for PauseShutdown {
    fn on_tool_start<'a>(&'a self, input: ToolStartInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            if input.call_id == "late-mail" {
                self.delivery_started.notify_one();
            }
        })
    }
}

impl ThreadLifecycleContributor<Config> for PauseShutdown {
    fn on_thread_stop<'a>(&'a self, input: ThreadStopInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if let Some(gate) = input.thread_store.get::<Arc<ShutdownGate>>() {
                gate.entered.notify_one();
                gate.release.notified().await;
            }
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_mail_and_cancelled_eviction_keep_worker_ownership() -> Result<()> {
    const QUEUED_TASK: &str = "handle this accepted task after dispatch resumes";
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
    let delivery = mount_root_collaboration_call(
        &server,
        "queue a followup",
        "queued-mail",
        "followup_task",
        json!({ "target": "first", "message": QUEUED_TASK }),
    )
    .await;
    let worker = mount_completed_worker(&server, QUEUED_TASK, "queued-mail").await;
    let queued_message_count = || {
        worker
            .requests()
            .iter()
            .flat_map(|request| request.inputs_of_type("agent_message"))
            .filter(|item| item.to_string().contains(QUEUED_TASK))
            .count()
    };
    let blocked_spawn = mount_root_collaboration_call(
        &server,
        "reclaim the queued worker",
        "blocked-spawn",
        "spawn_agent",
        json!({ "message": SECOND_TASK, "task_name": "replacement", "fork_turns": "none" }),
    )
    .await;

    let (entered_tx, entered_rx) = oneshot::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.config_contributor(Arc::new(PauseAfterCommit {
        gate: Mutex::new(Some((entered_tx, release_rx))),
    }));
    extensions.thread_lifecycle_contributor(Arc::new(ThreadIdle));
    let lifecycle = Arc::new(PauseShutdown::default());
    extensions.thread_lifecycle_contributor(lifecycle.clone());
    extensions.tool_lifecycle_contributor(lifecycle.clone());
    let test = test_codex()
        .with_model("gpt-5.6-sol")
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.features.enable(Feature::Collab).unwrap();
            config.features.enable(Feature::MultiAgentV2).unwrap();
            config.multi_agent_v2.max_concurrent_threads_per_session = 2;
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

    // Reuse the settings fixture to pause the submission loop without starting a turn.
    first
        .submit(Op::ThreadSettings {
            thread_settings: ThreadSettingsOverrides {
                model: Some(COMMITTED_MODEL.to_string()),
                ..Default::default()
            },
            reply: None,
        })
        .await?;
    timeout(Duration::from_secs(10), entered_rx).await??;
    test.submit_turn("queue a followup").await?;
    ThreadIdle::wait(&test.codex).await;
    assert_eq!(
        delivery.function_call_output_text("queued-mail"),
        Some(String::new()),
    );
    assert_eq!(queued_message_count(), 0);

    // The sender has finished, but accepted mail still owns its place in the queue.
    timeout(
        Duration::from_secs(10),
        test.submit_turn("reclaim the queued worker"),
    )
    .await
    .expect("queued mail must prevent eviction without waiting for dispatch")?;
    ThreadIdle::wait(&test.codex).await;
    assert_eq!(
        blocked_spawn.function_call_output_text("blocked-spawn"),
        Some("collab spawn failed: agent thread limit reached".to_string()),
    );
    assert!(test.thread_manager.get_thread(first_id).await.is_ok());

    release_tx.send(())?;
    wait_for_event(&first, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    ThreadIdle::wait(&first).await;
    assert_eq!(queued_message_count(), 1);

    // Reaching shutdown proves dispatch released the mail's residency guard.
    let gate = Arc::new(ShutdownGate::default());
    first.thread_extension_data().insert(Arc::clone(&gate));

    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, "start eviction"),
        sse(vec![
            ev_response_created("eviction-response"),
            ev_function_call_with_namespace(
                "cancelled-spawn",
                MULTI_AGENT_V2_NAMESPACE,
                "spawn_agent",
                &json!({ "message": SECOND_TASK, "task_name": "replacement", "fork_turns": "none" }).to_string(),
            ),
            ev_completed("eviction-response"),
        ]),
    )
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "start eviction".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    timeout(Duration::from_secs(10), gate.entered.notified()).await?;
    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;

    let delivery = mount_root_collaboration_call(
        &server,
        "send during eviction",
        "late-mail",
        "send_message",
        json!({ "target": "first", "message": "must not disappear behind shutdown" }),
    )
    .await;
    let send = test.submit_turn("send during eviction");
    tokio::pin!(send);
    tokio::select! {
        result = &mut send => panic!("delivery finished before shutdown was released: {result:?}"),
        started = timeout(Duration::from_secs(10), lifecycle.delivery_started.notified()) => started?,
    }
    assert!(
        timeout(Duration::from_secs(1), &mut send).await.is_err(),
        "mail must not be accepted by the closing runtime after its eviction caller is cancelled",
    );
    gate.release.notify_one();
    timeout(Duration::from_secs(10), send).await??;
    assert_eq!(
        delivery.function_call_output_text("late-mail"),
        Some(format!("agent with id {first_id} not found")),
    );
    assert!(test.thread_manager.get_thread(first_id).await.is_err());

    // The cancelled caller leaves no reservation behind once teardown finishes.
    mount_root_collaboration_call(
        &server,
        "retry replacement",
        "replacement-call",
        "spawn_agent",
        json!({ "message": SECOND_TASK, "task_name": "replacement", "fork_turns": "none" }),
    )
    .await;
    mount_completed_worker(&server, SECOND_TASK, "replacement-call").await;
    test.submit_turn("retry replacement").await?;
    let replacement = test
        .thread_manager
        .get_thread(created.recv().await?)
        .await?;
    wait_for_event(&replacement, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.thread_manager
        .shutdown_all_threads_bounded(Duration::from_secs(5))
        .await;
    Ok(())
}
