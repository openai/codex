//! Spawn reports captured child settings even when the runtime leaves the manager during startup.

use super::*;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_protocol::protocol::CollabAgentRef;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::Weak;
use tokio::sync::oneshot;

type RemovedChild = (ThreadId, Arc<CodexThread>);

struct RemoveChildOnTurnStart {
    context: OnceLock<(Weak<ThreadManager>, ThreadId)>,
    removed: Mutex<Option<oneshot::Sender<RemovedChild>>>,
}

impl TurnLifecycleContributor for RemoveChildOnTurnStart {
    fn on_turn_start<'a>(&'a self, input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let (manager, root_thread_id) = self.context.get().expect("test context initialized");
            let thread_id = ThreadId::from_string(input.thread_store.level_id())
                .expect("thread store has a thread ID");
            if thread_id == *root_thread_id {
                return;
            }

            // V1 spawn awaits this callback before accepting the child's initial input, so
            // the manager lookup is guaranteed to fail before the spawn tool reports settings.
            let child = manager
                .upgrade()
                .expect("test manager is alive")
                .remove_thread(&thread_id)
                .await
                .expect("spawn registered the child");
            assert!(
                self.removed
                    .lock()
                    .expect("removed child sender lock")
                    .take()
                    .expect("only one child starts")
                    .send((thread_id, child))
                    .is_ok()
            );
        })
    }
}

#[tokio::test]
async fn spawn_reports_effective_settings_after_child_runtime_is_removed() -> Result<()> {
    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("parent-spawn"),
                ev_function_call_with_namespace(
                    SPAWN_CALL_ID,
                    MULTI_AGENT_V1_NAMESPACE,
                    "spawn_agent",
                    &json!({
                        "message": CHILD_PROMPT,
                        "agent_type": "custom",
                        "model": REQUESTED_MODEL,
                        "reasoning_effort": REQUESTED_REASONING_EFFORT,
                    })
                    .to_string(),
                ),
                ev_completed("parent-spawn"),
            ]),
            sse(vec![ev_response_created("done-1"), ev_completed("done-1")]),
            sse(vec![ev_response_created("done-2"), ev_completed("done-2")]),
        ],
    )
    .await;
    let (removed_tx, removed_rx) = oneshot::channel();
    let remover = Arc::new(RemoveChildOnTurnStart {
        context: OnceLock::new(),
        removed: Mutex::new(Some(removed_tx)),
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.turn_lifecycle_contributor(remover.clone());
    let test = test_codex()
        .with_model(INHERITED_MODEL)
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.features.enable(Feature::Collab).expect("enable collab");
            config
                .features
                .disable(Feature::MultiAgentV2)
                .expect("use V1 spawn");
            let role_path = config.codex_home.join("custom-role.toml");
            fs::write(
                &role_path,
                format!(
                    "model = \"{ROLE_MODEL}\"\nmodel_reasoning_effort = \"{ROLE_REASONING_EFFORT}\"\n"
                ),
            )
            .expect("write role config");
            config.agent_roles.insert(
                "custom".to_string(),
                AgentRoleConfig {
                    description: Some("Custom role".to_string()),
                    config_file: Some(role_path.to_path_buf()),
                    nickname_candidates: Some(vec!["Captured".to_string()]),
                },
            );
        })
        .build_with_auto_env(&server)
        .await?;
    assert!(
        remover
            .context
            .set((
                Arc::downgrade(&test.thread_manager),
                test.session_configured.thread_id,
            ))
            .is_ok()
    );

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: TURN_1_PROMPT.to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let (child_id, child) = timeout(Duration::from_secs(/*secs*/ 10), removed_rx).await??;
    assert!(matches!(
        test.thread_manager.get_thread(child_id).await,
        Err(error) if matches!(error.details(), codex_protocol::error::CodexErrorDetails::ThreadNotFound(id) if *id == child_id)
    ));
    let spawn = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::ItemCompleted(completed) => match &completed.item {
            TurnItem::CollabAgentToolCall(item) if item.id == SPAWN_CALL_ID => Some(item.clone()),
            _ => None,
        },
        _ => None,
    })
    .await;
    for thread in [test.codex.as_ref(), child.as_ref()] {
        wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        thread.shutdown_and_wait().await?;
    }

    assert_eq!(
        (spawn.model, spawn.reasoning_effort, spawn.receiver_agents),
        (
            Some(ROLE_MODEL.to_string()),
            Some(ROLE_REASONING_EFFORT),
            vec![CollabAgentRef {
                thread_id: child_id,
                agent_nickname: Some("Captured".to_string()),
                agent_role: Some("custom".to_string()),
            }],
        )
    );
    let requests = responses.requests();
    let child_request = requests
        .iter()
        .find(|request| request.header("thread-id") == Some(child_id.to_string()))
        .expect("removed child still receives its initial input");
    let child_body = child_request.body_json();
    assert_eq!(
        (&child_body["model"], &child_body["reasoning"]["effort"]),
        (&json!(ROLE_MODEL), &json!(ROLE_REASONING_EFFORT)),
    );
    let output = requests
        .iter()
        .find_map(|request| request.function_call_output_text(SPAWN_CALL_ID))
        .expect("parent receives successful spawn output");
    assert_eq!(
        serde_json::from_str::<Value>(&output)?,
        json!({ "agent_id": child_id.to_string(), "nickname": "Captured" }),
    );
    Ok(())
}
