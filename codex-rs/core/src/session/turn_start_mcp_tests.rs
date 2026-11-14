//! Concurrent catalog discovery retains publication and releases preparation on cancellation.

use super::*;
use crate::McpRefreshReason;
use codex_mcp::McpResourceClient;
use codex_mcp::McpResourceClientCacheKey;
use pretty_assertions::assert_eq;

struct Recorder {
    client: McpResourceClient,
    ready: Arc<tokio::sync::Barrier>,
    pause: std::sync::atomic::AtomicBool,
    keys: std::sync::Mutex<Vec<McpResourceClientCacheKey>>,
    discovered: tokio::sync::Notify,
}

impl codex_extension_api::TurnLifecycleContributor for Recorder {
    fn refresh_catalog_for_step<'a>(
        &'a self,
        _input: codex_extension_api::CatalogRefreshInput<'a>,
    ) -> codex_extension_api::ExtensionFuture<'a, bool> {
        Box::pin(async move {
            self.keys
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(self.client.cache_key());
            self.ready.wait().await;
            self.discovered.notify_one();
            if self.pause.swap(false, std::sync::atomic::Ordering::Relaxed) {
                std::future::pending::<()>().await;
            }
            false
        })
    }
}

/// Step discovery overlaps providers on ready connections; cancellation releases the catalog lock.
#[tokio::test]
async fn step_discovery_reconciles_mcp_skips_prewarm_and_releases_lock_on_cancellation() {
    let (mut session, turn_context) = make_session_and_context().await;
    let ready = Arc::new(tokio::sync::Barrier::new(2));
    let recorder = Arc::new(Recorder {
        client: McpResourceClient::new(Arc::clone(&session.services.mcp_runtime)),
        ready: Arc::clone(&ready),
        pause: std::sync::atomic::AtomicBool::new(true),
        keys: Default::default(),
        discovered: Default::default(),
    });
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_lifecycle_contributor(recorder.clone());
    builder.turn_lifecycle_contributor(Arc::new(Recorder {
        client: recorder.client.clone(),
        ready,
        pause: std::sync::atomic::AtomicBool::new(false),
        keys: Default::default(),
        discovered: Default::default(),
    }));
    session.services.extensions = Arc::new(builder.build());
    let session = Arc::new(session);
    let turn_context = Arc::new(turn_context);
    let prewarm = session
        .new_startup_prewarm_turn_with_sub_id(INITIAL_SUBMIT_ID.to_owned())
        .await;
    session
        .mcp_runtime_for_step(&prewarm, &[], &[], &HashSet::new())
        .await;
    assert!(recorder.keys.lock().expect("recorded keys").is_empty());
    let initial_key = recorder.client.cache_key();

    session
        .spawn_task(
            Arc::clone(&turn_context),
            Vec::new(),
            HeldStepTask {
                kind: TaskKind::Regular,
                finish: Arc::new(tokio::sync::Notify::new()),
            },
        )
        .await;
    session.mark_mcp_runtime_dirty(McpRefreshReason::ExplicitInvalidation);
    let cancellation = CancellationToken::new();
    let capture = session.capture_step_context(Arc::clone(&turn_context), &cancellation);
    tokio::pin!(capture);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::select! {
            _ = &mut capture => panic!("step must wait for discovery"),
            () = recorder.discovered.notified() => {}
        }
    })
    .await
    .expect("regular step should discover catalogs");
    assert!(!session.mcp_refresh.is_pending());
    let prepared_key = recorder.client.cache_key();
    assert!(prepared_key != initial_key);
    assert!(recorder.keys.lock().expect("recorded keys")[0] == prepared_key);

    // Prewarm also skips discovery while a regular turn is preparing a step.
    session
        .mcp_runtime_for_step(&prewarm, &[], &[], &HashSet::new())
        .await;
    assert_eq!(recorder.keys.lock().expect("recorded keys").len(), 1);
    cancellation.cancel();
    assert!(capture.await.is_err());

    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        session.capture_step_context(Arc::clone(&turn_context), &CancellationToken::new()),
    )
    .await
    .expect("cancelled discovery must release the catalog lock")
    .expect("next step should prepare");
    assert!(recorder.client.cache_key() == prepared_key);

    session.services.mcp_runtime.reconnect_on_next_refresh();
    session.request_mcp_runtime_refresh(McpRefreshReason::ExplicitInvalidation);
    session
        .capture_step_context(turn_context, &CancellationToken::new())
        .await
        .expect("step should consume pending reconnect before discovery");
    let keys = recorder.keys.lock().expect("recorded keys").clone();
    assert_eq!(keys.len(), 3);
    assert!(keys[1] == prepared_key);
    assert!(keys[2] != prepared_key);
    assert!(keys[2] == recorder.client.cache_key());
    session.abort_all_tasks(TurnAbortReason::Interrupted).await;
}

/// Explicit refresh overlaps providers under one lock and retains publication while a sibling waits.
#[tokio::test]
async fn explicit_cloud_refresh_runs_concurrently_and_awaits_all_results() {
    struct CatalogGate {
        entered: Arc<tokio::sync::Barrier>,
        finish: tokio::sync::Notify,
        result: Result<(), &'static str>,
    }

    impl codex_extension_api::TurnLifecycleContributor for CatalogGate {
        fn refresh_cloud_catalog<'a>(
            &'a self,
            _input: codex_extension_api::CloudCatalogRefreshInput<'a>,
        ) -> codex_extension_api::ExtensionFuture<'a, Result<(), String>> {
            Box::pin(async move {
                self.entered.wait().await;
                self.finish.notified().await;
                self.result.map_err(str::to_string)
            })
        }
    }

    let (mut session, _) = make_session_and_context().await;
    let entered = Arc::new(tokio::sync::Barrier::new(3));
    let failing = Arc::new(CatalogGate {
        entered: Arc::clone(&entered),
        finish: Default::default(),
        result: Err("plugin discovery failed"),
    });
    let succeeding = Arc::new(CatalogGate {
        entered: Arc::clone(&entered),
        finish: Default::default(),
        result: Ok(()),
    });
    let mut builder = codex_extension_api::ExtensionRegistryBuilder::new();
    builder.turn_lifecycle_contributor(failing.clone());
    builder.turn_lifecycle_contributor(succeeding.clone());
    session.services.extensions = Arc::new(builder.build());
    let session = Arc::new(session);
    let mut refresh = Box::pin(session.refresh_cloud_plugins());
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::select! {
            result = &mut refresh => panic!("refresh completed before both providers started: {result:?}"),
            _ = entered.wait() => {}
        }
    })
    .await
    .expect("both providers must start before either finishes");
    assert!(!session.mcp_refresh.is_pending());

    let mut lock = Box::pin(session.lock_cloud_catalogs());
    assert!(futures::poll!(&mut lock).is_pending());
    drop(lock);

    failing.finish.notify_one();
    assert!(futures::poll!(&mut refresh).is_pending());
    assert!(session.mcp_refresh.is_pending());
    succeeding.finish.notify_one();
    assert_eq!(
        refresh.await.expect_err("one provider failed").to_string(),
        "plugin discovery failed"
    );
    drop(
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            session.lock_cloud_catalogs(),
        )
        .await
        .expect("completed refresh must release the catalog lock"),
    );
}

#[derive(Clone, Copy)]
enum McpPause {
    Start,
    Result,
}

struct PauseMcpCall {
    phase: McpPause,
    entered: Notify,
    release: Notify,
    outcome: std::sync::OnceLock<codex_extension_api::ToolCallOutcome>,
}

impl codex_extension_api::ToolLifecycleContributor for PauseMcpCall {
    fn on_tool_start<'a>(
        &'a self,
        input: codex_extension_api::ToolStartInput<'a>,
    ) -> codex_extension_api::ToolLifecycleFuture<'a> {
        Box::pin(async move {
            assert_eq!(input.mcp_tool.unwrap().tool_info().tool.name, "probe");
            if matches!(self.phase, McpPause::Start) {
                self.entered.notify_one();
                self.release.notified().await;
            }
        })
    }

    fn on_mcp_tool_result<'a>(
        &'a self,
        _input: codex_extension_api::McpToolResultInput<'a>,
    ) -> codex_extension_api::ToolLifecycleFuture<'a> {
        Box::pin(async move {
            if matches!(self.phase, McpPause::Result) {
                self.entered.notify_one();
                self.release.notified().await;
            }
        })
    }

    fn on_tool_finish<'a>(
        &'a self,
        input: codex_extension_api::ToolFinishInput<'a>,
    ) -> codex_extension_api::ToolLifecycleFuture<'a> {
        Box::pin(async move { self.outcome.set(input.outcome).expect("one finish") })
    }
}

#[test_case::test_case(McpPause::Start; "stop_before_send")]
#[test_case::test_case(McpPause::Result; "stop_after_send")]
#[tokio::test]
async fn mcp_budget_stop_at_handoff(phase: McpPause) -> anyhow::Result<()> {
    use crate::tools::parallel::ToolCallRuntime;
    use crate::tools::router::ToolCall;
    use codex_extension_api::ToolCallOutcome;
    use core_test_support::hooks::mount_hook_mcp;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    core_test_support::skip_if_no_network!(Ok(()));
    let endpoint = wiremock::MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let received = Arc::clone(&calls);
    mount_hook_mcp(&endpoint, "handoff", &["probe"], move |_| {
        received.fetch_add(1, Ordering::Relaxed);
        json!({"content": [{"type":"text", "text":"peer result"}]})
    })
    .await;
    let observer = Arc::new(PauseMcpCall {
        phase,
        entered: Notify::new(),
        release: Notify::new(),
        outcome: Default::default(),
    });
    let (mut session, _) = make_session_and_context().await;
    let mut extensions = codex_extension_api::ExtensionRegistryBuilder::new();
    extensions.tool_lifecycle_contributor(observer.clone());
    session.services.extensions = Arc::new(extensions.build());
    session
        .rollout_budget()
        .configure(crate::config::RolloutBudgetConfig {
            enforce_limit: true,
            limit_budget: 1.0,
            reminder_at_context_start: true,
            reminder_at_remaining_tokens: Vec::new(),
            sampling_token_weight: 1.0,
            prefill_token_weight: 1.0,
        });
    {
        let mut state = session.state.lock().await;
        let config = Arc::make_mut(&mut state.session_configuration.original_config_do_not_use);
        config
            .mcp_servers
            .set(serde_json::from_value(json!({"handoff": {
                "url": format!("{}/repl", endpoint.uri()),
                "default_tools_approval_mode": "approve"
            }}))?)?;
    }
    let session = Arc::new(session);
    session.mark_mcp_runtime_dirty(McpRefreshReason::ExplicitInvalidation);
    session
        .wait_for_mcp_server(&codex_mcp::McpServerKey::unscoped("handoff"))
        .await;
    let turn = session.new_default_turn().await;
    let cancel = CancellationToken::new();
    let step = session
        .capture_step_context_with_required_mcp_servers(
            turn,
            &cancel,
            &["handoff".to_owned()],
            &HashSet::new(),
        )
        .await?;
    let tool = step
        .mcp
        .tools()
        .iter()
        .find(|tool| tool.server_name() == "handoff" && tool.tool.name == "probe")
        .expect("MCP server advertised probe");
    let call = ToolCall {
        tool_name: tool.canonical_tool_name(),
        call_id: "handoff-probe".into(),
        payload: ToolPayload::Function {
            arguments: "{}".into(),
        },
        encrypted_function_args: None,
        native_call: None,
    };
    let runtime = ToolCallRuntime::new(
        session.clone(),
        step,
        Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
    );
    let response = runtime.handle_tool_call(call, cancel);
    tokio::pin!(response);
    tokio::select! {
        () = observer.entered.notified() => {}
        reply = &mut response => anyhow::bail!("MCP call finished before the observer paused: {reply:?}"),
    }
    let usage = codex_protocol::protocol::TokenUsage {
        output_tokens: 2,
        ..Default::default()
    };
    session
        .record_rollout_budget_usage("peer-turn", &usage.into())
        .await?;
    observer.release.notify_one();
    let reply = response.await?;
    let (count, outcome) = match phase {
        McpPause::Start => {
            assert_eq!(
                reply.item,
                ResponseInputItem::FunctionCallOutput {
                    call_id: "handoff-probe".into(),
                    output: FunctionCallOutputPayload::from_text(
                        "aborted before MCP handoff: rollout budget exhausted".into()
                    ),
                }
                .into()
            );
            (0, ToolCallOutcome::Aborted)
        }
        McpPause::Result => (1, ToolCallOutcome::Completed { success: true }),
    };
    assert_eq!(calls.load(Ordering::Relaxed), count);
    assert_eq!(observer.outcome.get(), Some(&outcome));
    session.services.mcp_runtime.shutdown().await;
    Ok(())
}
