//! Verifies host controller selection and routing through existing runtime entry points.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use codex_agent_message_board_extension::PostMetadata;
use codex_core::AgentConfigUpdate;
use codex_core::AgentControl;
use codex_core::AgentExecutionGuard;
use codex_core::AgentInfo;
use codex_core::AgentTarget;
use codex_core::AgentTurnOutcome;
use codex_core::DeliveryReceipt;
use codex_core::ForkSnapshot;
use codex_core::GuardianRootSnapshot;
use codex_core::LiveAgent;
use codex_core::RolloutBudgetReminder;
use codex_core::SendRequest;
use codex_core::SpawnRequest;
use codex_core::StartThreadOptions;
use codex_core::ThreadConfigSnapshot;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_history::InitialHistory;
use codex_history::ResumedHistory;
use codex_history::RolloutItem;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::MultiAgentVersion;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use codex_protocol::user_input::UserInput;
use codex_rollout_trace::ThreadTraceContext;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use wiremock::MockServer;

struct TestAgentControl {
    thread_id: ThreadId,
    service_tier: Mutex<Option<String>>,
    agents: Mutex<HashMap<String, ThreadId>>,
}

impl TestAgentControl {
    fn new(thread_id: ThreadId) -> Self {
        Self {
            thread_id,
            service_tier: Mutex::default(),
            agents: Mutex::new(HashMap::from([("/root".into(), thread_id)])),
        }
    }
}

impl AgentControl for TestAgentControl {
    fn identity(&self) -> SessionId {
        SessionId::from(self.thread_id)
    }

    fn resolve<'a>(
        &'a self,
        _caller: ThreadId,
        _parent: Option<ThreadId>,
        _source: &'a SessionSource,
        target: &'a str,
    ) -> BoxFuture<'a, CodexResult<ThreadId>> {
        Box::pin(async move {
            if let Some(id) = self.agents.lock().expect("agents lock").get(target) {
                return Ok(*id);
            }
            ThreadId::from_string(target)
                .map_err(|error| CodexErr::InvalidRequest(error.to_string()))
        })
    }

    fn spawn(
        &self,
        request: SpawnRequest,
    ) -> BoxFuture<'_, CodexResult<(LiveAgent, ThreadConfigSnapshot)>> {
        assert_eq!(request.caller, self.thread_id);
        Box::pin(async { Err(CodexErr::InvalidRequest("host spawn rejection".to_string())) })
    }

    fn send(&self, request: SendRequest) -> BoxFuture<'_, CodexResult<DeliveryReceipt>> {
        assert_eq!(request.caller, self.thread_id);
        Box::pin(async { Err(CodexErr::InvalidRequest("host send rejection".to_string())) })
    }

    fn ensure_child_loaded(
        &self,
        parent: ThreadId,
        child: ThreadId,
    ) -> BoxFuture<'_, CodexResult<()>> {
        assert_eq!(parent, self.thread_id);
        Box::pin(async move {
            Err(CodexErr::InvalidRequest(format!(
                "host resume rejection for {child}"
            )))
        })
    }

    fn interrupt(
        &self,
        _caller: ThreadId,
        _target: AgentTarget,
        _version: MultiAgentVersion,
    ) -> BoxFuture<'_, CodexResult<AgentInfo>> {
        panic!("unexpected agent interrupt")
    }

    fn list<'a>(
        &'a self,
        caller: ThreadId,
        _parent: Option<ThreadId>,
        _source: &'a SessionSource,
        _path_prefix: Option<&'a str>,
    ) -> BoxFuture<'a, CodexResult<Vec<LiveAgent>>> {
        assert_eq!(caller, self.thread_id);
        Box::pin(async { Err(CodexErr::InvalidRequest("host list rejection".to_string())) })
    }

    fn child_agent_paths(&self, _parent: ThreadId) -> BoxFuture<'_, Vec<AgentPath>> {
        Box::pin(async { Vec::new() })
    }

    fn check_turn_admission(
        &self,
        _version: MultiAgentVersion,
        _source: &SessionSource,
    ) -> CodexResult<()> {
        Ok(())
    }

    fn admit_turn(
        &self,
        _version: MultiAgentVersion,
        _source: &SessionSource,
    ) -> Option<AgentExecutionGuard> {
        None
    }

    fn record_usage(&self, _usage: TokenUsage) -> BoxFuture<'_, CodexResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn turn_finished<'a>(
        &'a self,
        _outcome: AgentTurnOutcome,
        _trace: &'a ThreadTraceContext,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }

    fn service_tier(&self) -> Option<String> {
        self.service_tier.lock().expect("service tier lock").clone()
    }

    fn propagate_config_update(&self, update: AgentConfigUpdate) {
        let AgentConfigUpdate::ServiceTier(tier) = update;
        *self.service_tier.lock().expect("service tier lock") = tier;
    }

    fn get_guardian_package(
        &self,
        _agent: ThreadId,
    ) -> BoxFuture<'_, Option<GuardianRootSnapshot>> {
        Box::pin(async { None })
    }

    fn pending_budget_reminder<'a>(
        &'a self,
        _agent: ThreadId,
        _window: &'a str,
    ) -> BoxFuture<'a, Option<RolloutBudgetReminder>> {
        Box::pin(async { None })
    }

    fn mark_budget_reminder_delivered<'a>(
        &'a self,
        _agent: ThreadId,
        _window: &'a str,
        _reminder: RolloutBudgetReminder,
    ) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }
}

async fn test_with_host_control(
    server: &MockServer,
) -> anyhow::Result<(TestCodex, Arc<TestAgentControl>)> {
    let thread_id = ThreadId::new();
    let controller = Arc::new(TestAgentControl::new(thread_id));
    let test = test_codex()
        .with_config(|config| {
            for feature in [
                Feature::Collab,
                Feature::MultiAgentV2,
                Feature::AgentMessageBoard,
            ] {
                config
                    .features
                    .enable(feature)
                    .expect("enable collaboration feature");
            }
            config.service_tier = Some("priority".to_string());
        })
        .with_thread_manager({
            let controller = Arc::clone(&controller);
            move |manager| {
                manager
                    .with_thread_id_generator(move || thread_id)
                    .with_agent_control_factory(move |_| {
                        let control: Arc<dyn AgentControl> = controller.clone();
                        async move { Ok(control) }
                    })
            }
        })
        .build_with_auto_env(server)
        .await?;
    Ok((test, controller))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_threads_preserve_lineage_settings_and_resume_routing() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let (test, controller) = test_with_host_control(&server).await?;
    let root_id = test.session_configured.thread_id;
    let mut child_config = test.config.clone();
    child_config.service_tier = None;
    let child = test
        .thread_manager
        .start_thread(StartThreadOptions {
            reserved_thread_id: Some(ThreadId::new()),
            environments: Some(vec![test.executor_environment().selection().clone()]),
            session_source: Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                parent_thread_id: root_id,
                depth: 1,
                agent_path: Some(AgentPath::root().join("worker").expect("valid child path")),
                agent_nickname: None,
                agent_role: None,
            })),
            ..StartThreadOptions::new(child_config.clone())
        })
        .await?;
    assert_eq!(
        (
            child.session_configured.session_id,
            child.session_configured.parent_thread_id
        ),
        (controller.identity(), Some(root_id))
    );
    assert_eq!(controller.service_tier(), Some("priority".to_string()));
    child.thread.ensure_rollout_materialized().await;
    child.thread.flush_rollout().await?;
    let saved = test
        .thread_store
        .load_latest_model_context(codex_thread_store::LoadThreadHistoryParams {
            thread_id: child.thread_id,
            include_archived: false,
        })
        .await?;
    assert!(saved.items.iter().any(|item| matches!(item,
        RolloutItem::SessionMeta(meta) if meta.meta.parent_thread_id == Some(root_id)
    )));

    controller
        .agents
        .lock()
        .expect("agents lock")
        .insert("/root/worker".into(), child.thread_id);
    for (thread, author, recipient, destination) in [
        (&test.codex, "/root", "/root/worker", "new_channel_name"),
        (&child.thread, "/root/worker", "/root", "channel_name"),
    ] {
        let arguments = serde_json::json!({
            (destination): "design",
            "text": "A shared decision.",
            "agents_to_notify": [recipient],
        });
        let mock = responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse(vec![
                    responses::ev_function_call_with_namespace(
                        "post",
                        "collaboration",
                        "post",
                        &arguments.to_string(),
                    ),
                    responses::ev_completed("post"),
                ]),
                responses::sse(vec![responses::ev_completed("done")]),
            ],
        )
        .await;
        thread
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Post a decision.".into(),
                text_elements: Vec::new(),
            }]))
            .await?;
        wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
        let post: PostMetadata = serde_json::from_str(
            &mock.requests()[1]
                .function_call_output_text("post")
                .expect("post result"),
        )?;
        assert_eq!(post.author.as_str(), author);
    }

    // Rejected starts must leave the live tree's settings alone, whether the
    // requested root already exists or the factory returns the wrong identity.
    for reserved_thread_id in [root_id, ThreadId::new()] {
        let rejected = test
            .thread_manager
            .start_thread(StartThreadOptions {
                reserved_thread_id: Some(reserved_thread_id),
                environments: Some(vec![test.executor_environment().selection().clone()]),
                ..StartThreadOptions::new(child_config.clone())
            })
            .await;
        let error = rejected
            .err()
            .expect("startup must reject the requested root");
        if reserved_thread_id != root_id {
            assert!(
                matches!(error.details(), CodexErrorDetails::InvalidRequest(_)),
                "{error:?}"
            );
        }
        assert_eq!(controller.service_tier(), Some("priority".to_string()));
    }
    child.thread.shutdown_and_wait().await?;
    test.thread_manager.remove_thread(&child.thread_id).await;
    let error = test
        .thread_manager
        .ensure_multi_agent_v2_child_loaded(child.thread_id)
        .await
        .expect_err("child reload must reach the host controller");
    assert!(
        matches!(error.details(), CodexErrorDetails::InvalidRequest(message)
            if message == &format!("host resume rejection for {}", child.thread_id)
        ),
        "{error:?}"
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn host_factory_follows_thread_lifecycle() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let test = test_codex()
        .with_config(|config| {
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("enable multi-agent v2");
        })
        .with_thread_manager({
            let calls = Arc::clone(&calls);
            move |manager| {
                manager.with_agent_control_factory(move |thread_id| {
                    calls.lock().expect("factory calls lock").push(thread_id);
                    let control: Arc<dyn AgentControl> = Arc::new(TestAgentControl::new(thread_id));
                    async move { Ok(control) }
                })
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let root_id = test.session_configured.thread_id;
    let manager = &test.thread_manager;
    let options = || StartThreadOptions {
        environments: Some(vec![test.executor_environment().selection().clone()]),
        ..StartThreadOptions::new(test.config.clone())
    };
    let internal = manager
        .spawn_internal_session(
            root_id,
            StartThreadOptions {
                session_source: Some(SessionSource::Internal(InternalSessionSource::Guardian)),
                ..options()
            },
        )
        .await?;
    assert_eq!(
        internal.session_configured.session_id,
        test.session_configured.session_id
    );
    test.codex.ensure_rollout_materialized().await;
    test.codex.flush_rollout().await?;
    let saved = test.codex.load_history(/*include_archived*/ false).await?;
    let history = InitialHistory::Resumed(ResumedHistory {
        conversation_id: root_id,
        history: Arc::new(saved.items),
        rollout_path: test.codex.rollout_path(),
    });
    let fork = manager
        .fork_thread_from_history(ForkSnapshot::Interrupted, options(), history.clone())
        .await?;
    assert_eq!(
        fork.session_configured.session_id,
        SessionId::from(fork.thread_id)
    );
    let warm = manager
        .start_thread(StartThreadOptions {
            initial_history: history.clone(),
            ..options()
        })
        .await?;
    assert!(Arc::ptr_eq(&warm.thread, &test.codex));
    assert_eq!(
        *calls.lock().expect("factory calls lock"),
        vec![root_id, fork.thread_id]
    );
    internal.thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;
    let cold = manager
        .start_thread(StartThreadOptions {
            initial_history: history,
            ..options()
        })
        .await?;
    assert!(!Arc::ptr_eq(&cold.thread, &test.codex));
    assert_eq!(cold.thread_id, root_id);
    assert_eq!(
        *calls.lock().expect("factory calls lock"),
        vec![root_id, fork.thread_id, root_id]
    );
    cold.thread.shutdown_and_wait().await?;
    fork.thread.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collaboration_tools_dispatch_to_the_host_controller() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let (test, _controller) = test_with_host_control(&server).await?;
    let mut events = vec![responses::ev_response_created("tools")];
    for (call, name, arguments) in [
        (
            "spawn",
            "spawn_agent",
            serde_json::json!({"task_name": "worker", "message": "hello", "fork_turns": "none"}),
        ),
        (
            "send",
            "send_message",
            serde_json::json!({"target": ThreadId::new().to_string(), "message": "hello"}),
        ),
        ("list", "list_agents", serde_json::json!({})),
    ] {
        events.push(responses::ev_function_call_with_namespace(
            call,
            "collaboration",
            name,
            &arguments.to_string(),
        ));
    }
    events.push(responses::ev_completed("tools"));
    let calls = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(events),
            responses::sse(vec![responses::ev_completed("done")]),
        ],
    )
    .await;
    test.submit_text_turn("use collaboration").await?;
    let requests = calls.requests();
    assert_eq!(requests.len(), 2);
    for call in ["spawn", "send", "list"] {
        assert!(
            requests[1]
                .function_call_output(call)
                .to_string()
                .contains(&format!("host {call} rejection"))
        );
    }
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InternalSessionSource;
