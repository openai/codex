//! Exercises host-reserved child startup with fresh and inherited history.

use super::super::agent_control::TestAgentControl;
use super::super::agents_md::RecordingThreadInstructionsProvider;
use super::*;
use anyhow::Context;
use codex_analytics::AnalyticsEventsClient;
use codex_core::AgentControl;
use codex_core::CodexThread;
use codex_core::HostThreadSpawnReservation;
use codex_core::HostToolInvocationLookup;
use codex_core::PromptCacheKey;
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
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::input_text;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_thread_store::AddThreadAttachmentParams;
use codex_thread_store::ListThreadAttachmentsParams;
use codex_thread_store::LoadThreadHistoryParams;
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
    prompt_cache_key: PromptCacheKey,
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
    options.prompt_cache_key = prompt_cache_key;
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

async fn start_first_turn(child: &CodexThread, prompt: &str) -> Result<()> {
    let submission = child
        .start_turn_if_idle(TurnInputRequest::user_input(vec![text(prompt)]))
        .await?;
    assert!(
        matches!(submission, StartIfIdleSubmission::Started { .. }),
        "expected the new child to be idle, got {submission:?}"
    );
    wait_for_successful_turn(child).await;
    Ok(())
}

#[test_case::test_case(false; "fresh")]
#[test_case::test_case(true; "forked")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_manager_creation_preserves_live_bindings_and_attachments(
    fork_history: bool,
) -> Result<()> {
    const PR: &str = "openai/codex#123";
    const UPDATED_RULE: &str = "Updated shared worker instructions.";
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let network_controller = NetworkPolicyController::default();
    let network_policy = network_controller.policy();
    assert!(network_controller.publish(network_policy.revision(), DestinationPolicy::Unrestricted));
    let provider =
        Arc::new(RecordingThreadInstructionsProvider::with_text("Initial rules.").shared());
    let (test, parent, controller) = host_spawn_fixture(
        &server,
        test_codex(),
        network_policy.clone(),
        PromptCacheKey::SessionDerived,
        Some(provider.clone()),
    )
    .await?;
    let root_id = test.session_configured.thread_id;
    let captured = parent.config_snapshot().await;
    let invocation =
        host_spawn_invocation(&server, &parent, "spawn-worker", "Start the worker.").await?;
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
    let child_id = ThreadId::new();
    let request = || create_request("worker", fork_history);
    let reservation = || HostThreadSpawnReservation {
        thread_id: child_id,
        control: controller.clone(),
    };
    // A detached snapshot cannot carry the source's live application policy.
    assert!(matches!(
        parent
            .prepare_child(&context, request())
            .await,
        Err(ThreadSpawnError::Backend(error))
            if error.to_string().contains("host-managed application authority")
    ));
    let mut mismatched = reservation();
    mismatched.control = Arc::new(TestAgentControl::new(ThreadId::new()));
    assert!(matches!(
        parent.create_child(&context, request(), mismatched).await,
        Err(ThreadSpawnError::Preparation(_))
    ));
    assert!(test.thread_manager.get_thread(child_id).await.is_err());

    provider.set_instructions(Some(Instructions {
        text: UPDATED_RULE.into(),
        source: None,
    }));
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
        worker.prepare_child(&context, request()).await,
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
    start_first_turn(&worker, "Report your status.").await?;
    assert!(
        child_request
            .single_request()
            .body_contains_text(UPDATED_RULE)
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

#[test_case::test_case(false, "inherited instruction providers"; "thread instructions")]
#[test_case::test_case(true, "EMA-authenticated MCP servers"; "enterprise MCP")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepare_child_rejects_live_bindings(ema_enabled: bool, expected: &str) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            r#"features.use_xaa = true
mcp_enterprise_managed_auth.idp = {{ issuer = "https://idp.example", client_id = "test" }}
mcp_servers.enterprise = {{ url = "https://mcp.example/mcp", auth = "ema_auth", enabled = {ema_enabled} }}"#
        ),
    )?;
    let (test, parent, controller) = host_spawn_fixture(
        &server,
        test_codex().with_home(home),
        NetworkPolicy::unmanaged(),
        PromptCacheKey::SessionDerived,
        (!ema_enabled).then(|| {
            Arc::new(RecordingThreadInstructionsProvider::with_text("Shared rules").shared())
                as Arc<dyn ThreadInstructionsProvider>
        }),
    )
    .await?;
    let call = host_spawn_invocation(&server, &parent, "prepare-worker", "Start a worker.").await?;
    let context = parent.tool_invocation(&call.turn_id, &call.call_id)?;
    let request = || create_request("worker", /*fork_history*/ false);
    let Err(ThreadSpawnError::Backend(error)) = parent.prepare_child(&context, request()).await
    else {
        anyhow::bail!("expected detached preparation to reject {expected}");
    };
    assert!(matches!(
        error.details(),
        CodexErrorDetails::InvalidRequest(message) if message.contains(expected)
    ));
    let child_id = ThreadId::new();
    parent
        .create_child(
            &context,
            request(),
            HostThreadSpawnReservation {
                thread_id: child_id,
                control: controller,
            },
        )
        .await?;
    test.thread_manager
        .get_thread(child_id)
        .await?
        .shutdown_and_wait()
        .await?;
    parent.shutdown_and_wait().await?;
    Ok(())
}

#[test_case::test_case(true, None; "full_history")]
#[test_case::test_case(false, Some("host-cache-key"); "fresh_history")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_spawn_starts_on_an_independent_destination_after_parent_shutdown(
    fork_history: bool,
    cache_key: Option<&str>,
) -> Result<()> {
    const PROJECT_RULE: &str = "Run the destination repository checks.";
    const EARLIER_INPUT: &str = "Remember PRIOR_CONTEXT_5241.";
    const SPAWN_INPUT: &str =
        "Remember TRANSFERRED_CONTEXT_7391 and ask the host to start a worker.";
    const TASK: &str = "Report TRANSFERRED_TASK_8812 and the remembered context.";
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let (source, parent, source_control) = host_spawn_fixture(
        &server,
        test_codex(),
        NetworkPolicy::unmanaged(),
        cache_key.map_or(PromptCacheKey::Omit, |key| {
            PromptCacheKey::Custom(key.into())
        }),
        /*thread_instructions_provider*/ None,
    )
    .await?;
    let root_id = source.session_configured.thread_id;
    let session_id = source.session_configured.session_id;
    parent
        .set_app_server_client_info(
            Some("source-client".into()),
            Some("1.0".into()),
            /*mcp_elicitations_auto_deny*/ false,
        )
        .await?;
    let captured = parent.config_snapshot().await;
    responses::mount_sse_once(&server, sse(vec![ev_completed("remembered")])).await;
    source.submit_text_turn(EARLIER_INPUT).await?;
    let invocation = host_spawn_invocation(&server, &parent, "prepare-worker", SPAWN_INPUT).await?;
    let context = parent.tool_invocation(&invocation.turn_id, &invocation.call_id)?;
    let prepared = parent
        .prepare_child(&context, create_request("remote_worker", fork_history))
        .await?;
    let child_path = prepared.agent_path().clone();
    assert_eq!(child_path.as_str(), "/root/remote_worker");
    let agents_md = source.workspace_path_uri("AGENTS.md")?;

    // Keep only durable storage and referenced paths; startup cannot consult A's runtime,
    // manager, controller, or environment services after this point.
    let source_store = Arc::clone(&source.thread_store);
    let _source_home = Arc::clone(&source.home);
    let _source_cwd = Arc::clone(&source.cwd);
    let source_manager = Arc::downgrade(&source.thread_manager);
    let source_parent = Arc::downgrade(&parent);
    parent.shutdown_and_wait().await?;
    source.thread_manager.remove_thread(&root_id).await;
    drop(parent);
    drop(source);
    drop(source_control);
    assert!(source_manager.upgrade().is_none());
    assert!(source_parent.upgrade().is_none());

    // Independent runtimes may share durable storage.
    wiremock::Mock::given(wiremock::matchers::path("/codex/analytics-events/events"))
        .respond_with(wiremock::ResponseTemplate::new(/*status*/ 200))
        .mount(&server)
        .await;
    let analytics = AnalyticsEventsClient::new(
        codex_core::test_support::auth_manager_from_auth(
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ),
        server.uri(),
        /*analytics_enabled*/ Some(true),
    );
    let destination = test_codex()
        .with_analytics_events_client(analytics.clone())
        .with_thread_store(source_store)
        .with_model("gpt-6-astra")
        .with_config(configure_scenario_catalog)
        .build_with_auto_env(&server)
        .await?;
    let manager = &destination.thread_manager;
    destination.codex.shutdown_and_wait().await?;
    manager
        .remove_thread(&destination.session_configured.thread_id)
        .await;
    destination
        .fs()
        .write_file(
            &agents_md,
            PROJECT_RULE.as_bytes().to_vec(),
            Default::default(),
            /*sandbox*/ None,
        )
        .await?;
    let child_id = ThreadId::new();
    // The destination controller shares the tree identity but has no entry for the child path.
    let destination_control = Arc::new(TestAgentControl::new(root_id));
    let child_request =
        responses::mount_sse_once(&server, responses::sse_completed("remote-done")).await;
    let spawned = manager
        .create_prepared_child(
            prepared,
            HostThreadSpawnReservation {
                thread_id: child_id,
                control: destination_control,
            },
        )
        .await?;
    assert_eq!(
        (
            spawned.agent.thread_id,
            &spawned.task_name,
            spawned.config.parent_thread_id,
        ),
        (child_id, &child_path, Some(root_id))
    );
    assert_eq!(
        (
            spawned.config.permission_profile,
            spawned.config.disabled_plugin_ids,
        ),
        (captured.permission_profile, captured.disabled_plugin_ids),
    );
    let worker = manager.get_thread(child_id).await?;
    start_first_turn(&worker, TASK).await?;
    analytics.flush().await;
    let events = responses::received_analytics_events(&server, "codex_thread_initialized").await;
    let child_starts: Vec<_> = events
        .iter()
        .map(|event| &event["event_params"])
        .filter(|params| params["thread_id"] == json!(child_id))
        .collect();
    let [start] = child_starts.as_slice() else {
        anyhow::bail!("expected one initialization for {child_id}, got {child_starts:?}");
    };
    assert_eq!(start["parent_thread_id"], json!(root_id));
    assert_eq!(start["app_server_client"]["client_name"], "source-client");
    assert_eq!(start["app_server_client"]["client_version"], "1.0");
    let requests = responses::received_responses_requests(&server).await;
    let history = context_snapshot::format_request_history_snapshot(
        "A prepares a child. After A and its manager shut down, a second manager creates the idle child and then starts its first turn.",
        &requests,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    let snapshot_name = if fork_history {
        "detached_spawn_full_history"
    } else {
        "detached_spawn_fresh_history"
    };
    insta::assert_snapshot!(snapshot_name, history);
    let child_request = child_request.single_request();
    assert_eq!(
        child_request.body_json().get("prompt_cache_key").cloned(),
        cache_key.map(|key| json!(key)),
    );
    assert!(
        child_request
            .inputs_of_type("additional_tools")
            .iter()
            .any(|item| {
                responses::namespace_child_tool(item, "functions", "exec")
                    .and_then(|tool| tool["description"].as_str())
                    .is_some_and(|description| description.contains("host_spawn(args:"))
            })
    );
    let user_messages = child_request.message_input_texts("user");
    assert!(user_messages.iter().any(|text| text.contains(PROJECT_RULE)));
    worker.ensure_rollout_materialized().await;
    worker.flush_rollout().await?;
    let stored = destination
        .thread_store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id: child_id,
            include_archived: false,
        })
        .await?;
    assert!(stored.items.iter().any(|item| matches!(item,
        RolloutItem::SessionMeta(meta) if meta.meta.session_id == session_id
            && meta.meta.parent_thread_id == Some(root_id)
    )));
    for input in [EARLIER_INPUT, SPAWN_INPUT] {
        let span = input_text(input);
        let inherited = stored.items.iter().find_map(|item| match item {
            RolloutItem::ResponseItem(envelope)
                if matches!(&envelope.item, ResponseItem::Message { role, content, .. }
                    if role == "user" && content.contains(&span)) =>
            {
                Some(
                    envelope
                        .metadata
                        .as_ref()
                        .is_some_and(|meta| meta.inherited_user_message),
                )
            }
            _ => None,
        });
        assert_eq!(inherited, fork_history.then_some(true), "{input}");
    }
    assert!(manager.get_thread(root_id).await.is_err());
    worker.shutdown_and_wait().await?;
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
        PromptCacheKey::SessionDerived,
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
