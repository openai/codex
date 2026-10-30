//! Exercises host-reserved child startup with fresh and inherited history.

use super::super::agent_control::TestAgentControl;
use super::super::agents_md::RecordingThreadInstructionsProvider;
use super::*;
use anyhow::Context;
use codex_core::AgentControl;
use codex_core::CodexThread;
use codex_core::HostThreadSpawnReservation;
use codex_core::HostToolInvocationLookup;
use codex_core::SpawnAgentForkMode;
use codex_core::StartIfIdleSubmission;
use codex_core::ThreadCreateRequest;
use codex_core::ThreadSpawnError;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::Instructions;
use codex_extension_api::ThreadInstructionsProvider;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadReadyInput;
use codex_http_client::DestinationPolicy;
use codex_http_client::NetworkPolicy;
use codex_http_client::NetworkPolicyController;
use codex_protocol::ThreadId;
use codex_protocol::dynamic_tools::DynamicToolCallRequest;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_thread_store::AddThreadAttachmentParams;
use codex_thread_store::ListThreadAttachmentsParams;
use core_test_support::responses;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::wait_for_successful_turn;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use wiremock::MockServer;

async fn host_spawn_fixture(
    server: &MockServer,
    builder: TestCodexBuilder,
    application_network_policy: NetworkPolicy,
    thread_instructions_provider: Option<Arc<dyn ThreadInstructionsProvider>>,
) -> Result<(TestCodex, Arc<CodexThread>, Arc<TestAgentControl>)> {
    let root_id = ThreadId::new();
    // This controller rejects every AgentControl::spawn call. Successful startup therefore
    // demonstrates the direct host hook, while runtime calls still use the host controller.
    let controller = Arc::new(TestAgentControl::new(root_id));
    let mut test = builder
        .with_model("gpt-6-astra")
        .with_config(move |config| {
            configure_scenario_catalog(config);
            config.application_network_policy = application_network_policy;
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("enable multi-agent V2");
            config
                .features
                .enable(Feature::MultiAgentV2DynamicTools)
                .expect("enable dynamic tool inheritance");
            config.multi_agent_v2.disable_native_tools = true;
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .expect("configure read-only permissions");
        })
        .with_thread_manager({
            let controller = Arc::clone(&controller);
            move |manager| {
                manager.with_agent_control_factory(move |thread_id| {
                    let control: Arc<dyn AgentControl> = if thread_id == root_id {
                        controller.clone()
                    } else {
                        Arc::new(TestAgentControl::new(thread_id))
                    };
                    async move { Ok(control) }
                })
            }
        })
        .build_with_auto_env(server)
        .await?;
    let mut options = test.start_thread_options().await;
    options
        .thread_extension_init
        .insert(HostToolInvocationLookup);
    options.history_mode = Some(ThreadHistoryMode::Legacy);
    options.thread_instructions_provider = thread_instructions_provider;
    options.reserved_thread_id = Some(root_id);
    options.disabled_plugin_ids = Some(vec!["captured-plugin@local".into()]);
    options.dynamic_tools = vec![DynamicToolSpec::Function(DynamicToolFunctionSpec {
        name: "host_spawn".into(),
        description: "Ask the host to start a child from this invocation.".into(),
        input_schema: json!({"type": "object", "properties": {}}),
        defer_loading: false,
    })];
    test.codex.shutdown_and_wait().await?;
    test.thread_manager
        .remove_thread(&test.session_configured.thread_id)
        .await;
    let root = test.thread_manager.start_thread(options).await?;
    test.codex = Arc::clone(&root.thread);
    test.session_configured = root.session_configured;
    let parent = root.thread;
    Ok((test, parent, controller))
}

async fn host_spawn_invocation(
    server: &MockServer,
    parent: &Arc<CodexThread>,
    call_id: &str,
    message: &str,
) -> Result<DynamicToolCallRequest> {
    responses::mount_sse_once(
        server,
        sse(vec![
            responses::ev_function_call(call_id, "host_spawn", "{}"),
            ev_completed(call_id),
        ]),
    )
    .await;
    parent
        .start_or_steer_turn(TurnInputRequest::user_input(vec![text(message)]))
        .await?;
    Ok(wait_for_event_match(parent, |event| match event {
        EventMsg::DynamicToolCallRequest(call) => Some(call.clone()),
        _ => None,
    })
    .await)
}

async fn finish_spawn_tool(
    server: &MockServer,
    parent: &CodexThread,
    invocation: &DynamicToolCallRequest,
) -> Result<()> {
    responses::mount_sse_once(server, sse(vec![ev_completed("parent-done")])).await;
    parent
        .submit(Op::DynamicToolResponse {
            id: invocation.call_id.clone(),
            response: DynamicToolResponse {
                content_items: vec![DynamicToolCallOutputContentItem::InputText {
                    text: "Done.".into(),
                }],
                success: true,
            },
        })
        .await?;
    wait_for_event(parent, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    assert!(
        parent
            .tool_invocation(&invocation.turn_id, &invocation.call_id)
            .is_err()
    );
    Ok(())
}

fn create_request(name: &str, fork_history: bool) -> ThreadCreateRequest {
    ThreadCreateRequest {
        task_name: name.into(),
        fork_mode: fork_history.then_some(SpawnAgentForkMode::FullHistory),
        agent_type: None,
        model: None,
        reasoning_effort: None,
    }
}

#[test_case::test_case(false; "fresh")]
#[test_case::test_case(true; "forked")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_manager_creation_preserves_live_bindings_and_attachments(
    fork_history: bool,
) -> Result<()> {
    const PR: &str = "openai/codex#123";
    const INITIAL_RULE: &str = "Initial rules.";
    const UPDATED_RULE: &str = "Updated shared worker instructions.";
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let network_controller = NetworkPolicyController::default();
    let network_policy = network_controller.policy();
    assert!(network_controller.publish(network_policy.revision(), DestinationPolicy::Unrestricted));
    let provider = Arc::new(RecordingThreadInstructionsProvider::with_text(INITIAL_RULE).shared());
    let (test, parent, controller) = host_spawn_fixture(
        &server,
        test_codex().with_config(|config| config.allow_persistent_dynamic_tool_updates = true),
        network_policy.clone(),
        Some(provider.clone()),
    )
    .await?;
    let root_id = test.session_configured.thread_id;
    let captured = parent.config_snapshot().await;
    let parent_message = "Start the worker.";
    let invocation =
        host_spawn_invocation(&server, &parent, "spawn-worker", parent_message).await?;
    let context = parent.tool_invocation(&invocation.turn_id, &invocation.call_id)?;
    test.thread_store
        .add_thread_attachment(AddThreadAttachmentParams {
            thread_id: root_id,
            attachment_type: "pull_request".into(),
            identity_key: PR.into(),
            payload: json!({"number": 123}),
        })
        .await?;
    // Changing current thread settings must not change the invocation's captured settings.
    submit_thread_settings(
        &parent,
        ThreadSettingsOverrides {
            disabled_plugin_ids: Some(vec!["later-plugin@local".into()]),
            ..Default::default()
        },
    )
    .await?;
    provider.set_instructions(Some(Instructions {
        text: UPDATED_RULE.into(),
        source: None,
    }));
    // Tool inheritance reads the current catalog while invocation settings remain captured.
    assert_eq!(parent.prepare_child(&context).await?.dynamic_tools.len(), 1);
    parent.replace_dynamic_tools(Vec::new()).await?;
    let prepared = parent.prepare_child(&context).await?;
    assert!(prepared.dynamic_tools.is_empty());
    assert_eq!(prepared.disabled_plugin_ids, captured.disabled_plugin_ids);
    assert_eq!(prepared.thread_instructions.as_deref(), Some(INITIAL_RULE));
    assert_eq!(
        prepared.resolved_permission_profile,
        PermissionProfile::read_only()
    );
    let child_id = ThreadId::new();
    let request = || create_request("worker", fork_history);
    let reservation = || HostThreadSpawnReservation {
        thread_id: child_id,
        control: controller.clone(),
    };
    let mut mismatched = reservation();
    mismatched.control = Arc::new(TestAgentControl::new(ThreadId::new()));
    assert!(matches!(
        parent.create_child(&context, request(), mismatched).await,
        Err(ThreadSpawnError::Preparation(_))
    ));
    assert!(test.thread_manager.get_thread(child_id).await.is_err());

    let spawned = parent
        .create_child(&context, request(), reservation())
        .await?;
    assert_eq!(spawned.config.approval_policy, captured.approval_policy);
    assert_eq!(spawned.config.environments, captured.environments);
    assert_eq!(
        spawned.config.disabled_plugin_ids,
        captured.disabled_plugin_ids
    );
    let worker = test.thread_manager.get_thread(child_id).await?;
    // A handle cannot be used through a different source runtime.
    assert!(matches!(
        worker.prepare_child(&context).await,
        Err(ThreadSpawnError::Backend(error))
            if error.to_string().contains("tool invocation belongs to a different runtime")
    ));
    let child_network_policy = worker.config().await.application_network_policy.clone();
    assert_eq!(child_network_policy, network_policy);
    assert_eq!(
        worker.rollout_path().context("child rollout")?.is_file(),
        fork_history,
        "only copied history should be materialized at creation"
    );
    // Once creation returns, ending the spawning invocation leaves the idle child available.
    finish_spawn_tool(&server, &parent, &invocation).await?;
    let child_request =
        responses::mount_sse_once(&server, responses::sse_completed("worker-done")).await;
    let submission = worker
        .start_turn_if_idle(TurnInputRequest::user_input(vec![text(
            "Report your status.",
        )]))
        .await?;
    assert!(
        matches!(submission, StartIfIdleSubmission::Started { .. }),
        "expected the new child to be idle, got {submission:?}"
    );
    wait_for_successful_turn(&worker).await;
    let child_request = child_request.single_request();
    assert!(child_request.body_contains_text(UPDATED_RULE));
    assert_eq!(
        child_request.body_contains_text(parent_message),
        fork_history
    );
    let page = test
        .thread_store
        .list_thread_attachments(ListThreadAttachmentsParams {
            thread_id: child_id,
            cursor: None,
            limit: 1,
        })
        .await?;
    assert_eq!(
        page.attachments
            .first()
            .map(|attachment| attachment.identity_key.as_str()),
        fork_history.then_some(PR)
    );

    worker.shutdown_and_wait().await?;
    parent.shutdown_and_wait().await?;
    Ok(())
}

struct ChildStartupGate {
    child_id: ThreadId,
    entered: Notify,
}

impl ThreadLifecycleContributor<Config> for ChildStartupGate {
    fn on_thread_ready<'a>(
        &'a self,
        input: ThreadReadyInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if input.thread_store.level_id() == self.child_id.to_string() {
                self.entered.notify_one();
                std::future::pending::<()>().await;
            }
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_creation_cleans_registered_child() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let child_id = ThreadId::new();
    let gate = Arc::new(ChildStartupGate {
        child_id,
        entered: Notify::new(),
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(gate.clone());
    let (test, parent, controller) = host_spawn_fixture(
        &server,
        test_codex().with_extensions(Arc::new(extensions.build())),
        NetworkPolicy::unmanaged(),
        /*thread_instructions_provider*/ None,
    )
    .await?;
    let invocation =
        host_spawn_invocation(&server, &parent, "cancel-spawn", "Start a worker.").await?;
    let context = parent.tool_invocation(&invocation.turn_id, &invocation.call_id)?;
    let spawn = tokio::spawn({
        let parent = Arc::clone(&parent);
        async move {
            parent
                .create_child(
                    &context,
                    create_request("cancelled", /*fork_history*/ true),
                    HostThreadSpawnReservation {
                        thread_id: child_id,
                        control: controller,
                    },
                )
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(10), gate.entered.notified())
        .await
        .context("child never reached the startup gate")?;
    let child = test.thread_manager.get_thread(child_id).await?;
    parent.submit(Op::Interrupt).await?;
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(10), spawn)
            .await
            .context("host spawn did not observe invocation revocation")??,
        Err(ThreadSpawnError::Preparation(_))
    ));
    tokio::time::timeout(Duration::from_secs(10), child.wait_until_terminated())
        .await
        .context("cancelled child's runtime did not terminate")?;
    assert!(test.thread_manager.get_thread(child_id).await.is_err());
    parent.shutdown_and_wait().await?;
    Ok(())
}
