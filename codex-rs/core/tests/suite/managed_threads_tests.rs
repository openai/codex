//! Exercises owned thread startup and shutdown through the real runtime and persistence store.

use std::future::Future;
use std::sync::Arc;
use std::task::Context as TaskContext;
use std::task::Wake;
use std::time::Duration;

use anyhow::Context;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_extension_api::SessionIsolation;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::UserInput;
use codex_thread_store::ThreadStoreError;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_startup_cleans_up_while_required_mcp_is_stalled() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let fixture = test_codex().build_with_auto_env(&server).await?;
    let mcp_server = responses::start_mock_server().await;
    let (http_server, control) = AppsTestServer::mount_with_startup_control(&mcp_server).await?;
    let release = control.hold_next_successful_initialize();
    let mut config = fixture.config.clone();
    let mut servers = config.mcp_servers.get().clone();
    servers.insert(
        "stalled".to_owned(),
        serde_json::from_value(json!({
            "url": format!("{}/api/codex/ps/mcp", http_server.chatgpt_base_url),
            "http_headers": { "Authorization": "Bearer synthetic-test-token" },
            "required": true,
            "startup_timeout_sec": 120,
        }))?,
    );
    config.mcp_servers.set(servers)?;
    let thread_id = fixture.thread_manager.reserve_thread_id();
    let mut options = StartThreadOptions::new(config);
    options.reserved_thread_id = Some(thread_id);
    options.environments = Some(fixture.codex.environment_selections().await);
    options
        .thread_extension_init
        .insert(SessionIsolation::Isolated);
    let tasks = TaskTracker::new();
    let mut start = Box::pin(fixture.thread_manager.start_thread_until(
        options,
        std::future::pending(),
        &tasks,
    ));

    tokio::select! {
        _ = &mut start => panic!("startup must wait for the required MCP server"),
        result = tokio::time::timeout(Duration::from_secs(10), async {
            while control.initialize_attempts() == 0 {
                tokio::task::yield_now().await;
            }
        }) => result.context("required MCP initialization did not begin")?,
    }
    // Persistence is already open while initialization is blocked on the server.
    fixture.thread_store.flush_thread(thread_id).await?;
    drop(start);
    tasks.close();
    tokio::time::timeout(Duration::from_secs(10), tasks.wait())
        .await
        .context("dropping startup must finish cleanup without releasing MCP initialization")?;
    assert_eq!(
        fixture.thread_manager.list_thread_ids().await,
        vec![fixture.session_configured.thread_id],
    );
    assert!(matches!(
        fixture.thread_store.flush_thread(thread_id).await,
        Err(ThreadStoreError::ThreadNotFound { .. })
    ));
    drop(release);
    fixture.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_startup_before_receiving_the_agent_finishes_cleanup() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    // Startup first suspends on its result channel. Observe that channel's wake without
    // polling again, so the result stays sent but unconsumed until we drop the caller.
    struct ResultReady(tokio::sync::Notify);
    impl Wake for ResultReady {
        fn wake(self: Arc<Self>) {
            self.0.notify_one();
        }
    }
    let server = responses::start_mock_server().await;
    let fixture = test_codex().build_with_auto_env(&server).await?;
    let mut options = StartThreadOptions::new(fixture.config.clone());
    let thread_id = fixture.thread_manager.reserve_thread_id();
    options.reserved_thread_id = Some(thread_id);
    options.environments = Some(fixture.codex.environment_selections().await);
    options
        .thread_extension_init
        .insert(SessionIsolation::Isolated);
    let tasks = TaskTracker::new();
    let mut start = Box::pin(fixture.thread_manager.start_thread_until(
        options,
        std::future::pending(),
        &tasks,
    ));
    let ready = Arc::new(ResultReady(tokio::sync::Notify::new()));
    let waker = Arc::clone(&ready).into();
    assert!(
        start
            .as_mut()
            .poll(&mut TaskContext::from_waker(&waker))
            .is_pending()
    );
    tokio::time::timeout(Duration::from_secs(10), ready.0.notified()).await?;
    fixture.thread_manager.get_thread(thread_id).await?;
    drop(start);
    tasks.close();
    tokio::time::timeout(Duration::from_secs(10), tasks.wait())
        .await
        .context("dropping an unconsumed result must also finish cleanup")?;
    assert_eq!(
        fixture.thread_manager.list_thread_ids().await,
        vec![fixture.session_configured.thread_id]
    );
    assert!(matches!(
        fixture.thread_store.flush_thread(thread_id).await,
        Err(ThreadStoreError::ThreadNotFound { .. })
    ));
    fixture.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_cancellation_closes_agent_and_preserves_history_and_parent() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let fixture = test_codex().build_with_auto_env(&server).await?;
    let mut options = StartThreadOptions::new(fixture.config.clone());
    options.environments = Some(fixture.codex.environment_selections().await);
    options
        .thread_extension_init
        .insert(SessionIsolation::Isolated);
    let cancelled = CancellationToken::new();
    let tasks = TaskTracker::new();
    let agent = fixture
        .thread_manager
        .start_thread_until(options, cancelled.clone().cancelled_owned(), &tasks)
        .await?;
    // A failed startup with the same ID must leave the original agent's writer intact.
    let mut duplicate = StartThreadOptions::new(fixture.config.clone());
    duplicate.reserved_thread_id = Some(agent.thread_id);
    duplicate.environments = Some(fixture.codex.environment_selections().await);
    duplicate
        .thread_extension_init
        .insert(SessionIsolation::Isolated);
    let failed_tasks = TaskTracker::new();
    assert!(
        fixture
            .thread_manager
            .start_thread_until(duplicate, std::future::pending(), &failed_tasks,)
            .await
            .is_err()
    );
    failed_tasks.close();
    tokio::time::timeout(Duration::from_secs(10), failed_tasks.wait()).await?;
    fixture.thread_store.flush_thread(agent.thread_id).await?;

    let response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("agent-response"),
            responses::ev_assistant_message("agent-message", "agent finished"),
            responses::ev_completed("agent-response"),
        ]),
    )
    .await;
    agent
        .thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "run the owned agent".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&agent.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert!(
        response
            .single_request()
            .body_contains_text("run the owned agent")
    );

    cancelled.cancel();
    tasks.close();
    tokio::time::timeout(Duration::from_secs(10), tasks.wait())
        .await
        .context("the owner must be able to join shutdown and deregistration")?;
    tokio::time::timeout(Duration::from_secs(1), agent.thread.wait_until_terminated()).await?;
    assert_eq!(
        fixture.thread_manager.list_thread_ids().await,
        vec![fixture.session_configured.thread_id],
    );
    assert!(matches!(
        fixture.thread_store.flush_thread(agent.thread_id).await,
        Err(ThreadStoreError::ThreadNotFound { .. })
    ));
    let history = agent
        .thread
        .load_history(/*include_archived*/ false)
        .await?;
    assert!(history.items.iter().any(|item| matches!(
        item,
        codex_history::RolloutItem::EventMsg(EventMsg::TurnComplete(_))
    )));

    let parent_response = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("parent-response"),
            responses::ev_completed("parent-response"),
        ]),
    )
    .await;
    fixture.submit_turn("parent still works").await?;
    assert!(
        parent_response
            .single_request()
            .body_contains_text("parent still works")
    );
    fixture.codex.shutdown_and_wait().await?;
    Ok(())
}
