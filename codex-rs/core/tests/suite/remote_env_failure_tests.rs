//! Environment startup failures reach the model, and transient failures can recover next turn.
use super::*;
use codex_protocol::protocol::TurnEnvironmentRequest;
use codex_protocol::protocol::TurnEnvironmentRequests;
use core_test_support::exec_server::ExecServerProcess;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicBool;
use test_case::test_case;
use tokio::task::JoinSet;
use tracing::Subscriber;
use tracing::span::Attributes;
use tracing::span::Id;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context as LayerContext;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

struct EnvironmentResolutionCounter(Arc<AtomicUsize>);

impl<S: Subscriber> Layer<S> for EnvironmentResolutionCounter {
    fn on_new_span(&self, attributes: &Attributes<'_>, _id: &Id, _context: LayerContext<'_, S>) {
        if attributes.metadata().target() == "codex_core::environment_selection"
            && attributes.metadata().name() == "environments.resolve"
        {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
}

#[test_case(true; "failed_before_turn")]
#[test_case(false; "failed_while_waiting")]
// Keep spawned tasks on the thread with the scoped tracing subscriber.
#[tokio::test(flavor = "current_thread")]
async fn provisioning_failure_reaches_model_and_turn_continues(
    fail_before_turn: bool,
) -> Result<()> {
    const REASON: &str = "This repository is empty. Push an initial commit, then retry.";
    const CALL_ID: &str = "wait-for-failed-environment";
    const SECOND_CALL_ID: &str = "wait-for-failed-environment-again";
    let resolutions = Arc::new(AtomicUsize::new(0));
    // Keep concurrent tests without a subscriber from caching this callsite as disabled.
    let _interest_cache_guard =
        tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let _tracing = tracing_subscriber::registry()
        .with(EnvironmentResolutionCounter(Arc::clone(&resolutions)))
        .set_default();
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_function_call(
                    CALL_ID,
                    "wait_for_environment",
                    &json!({
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message(
                    "message-1",
                    "I can explain how to initialize the repository.",
                ),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_function_call(
                    SECOND_CALL_ID,
                    "wait_for_environment",
                    &json!({
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                    })
                    .to_string(),
                ),
                ev_completed("resp-3"),
            ]),
            sse(vec![
                ev_assistant_message("message-2", "The repository still needs an initial commit."),
                ev_completed("resp-4"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex_with_wait_for_environment().with_config(|config| {
        assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
    });
    let test = expect_startup(builder.build(&server)).await;
    let resolutions_before_selection = resolutions.load(Ordering::SeqCst);
    let manager = test.thread_manager.environment_manager();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    manager.materialize_pending_noise_environment(
        REMOTE_ENVIRONMENT_ID.to_string(),
        provider.clone(),
    )?;
    if fail_before_turn {
        manager.report_environment_provisioning_status(
            REMOTE_ENVIRONMENT_ID.to_string(),
            Err(REASON.to_string()),
            provider.clone(),
        )?;
    }
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "inspect the repository".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentRequests::new(
                    test.config.cwd.clone(),
                    vec![TurnEnvironmentRequest {
                        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                        cwd: PathUri::from_abs_path(&test.config.cwd),
                        workspace_roots: vec![PathUri::from_abs_path(&test.config.cwd)],
                        config: EnvironmentConfigState::FromThread,
                    }],
                )),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_response_request_count(&response_mock, /*expected_count*/ 1).await;
    if !fail_before_turn {
        manager.report_environment_provisioning_status(
            REMOTE_ENVIRONMENT_ID.to_string(),
            Err(REASON.to_string()),
            provider.clone(),
        )?;
    }
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let (output, _) = requests[1]
        .function_call_output_content_and_success(CALL_ID)
        .context("the model should receive the failed wait result")?;
    assert!(
        output.as_deref().is_some_and(|text| text.contains(REASON)),
        "{output:?}"
    );
    let context_request = if fail_before_turn {
        &requests[0]
    } else {
        &requests[1]
    };
    let context = context_request.message_input_texts("user").join("\n");
    assert!(context.contains("<status>failed</status>"), "{context}");
    assert!(context.contains(REASON), "{context}");

    let initial_resolutions = resolutions.load(Ordering::SeqCst);
    assert!(initial_resolutions > resolutions_before_selection);
    let selections = test.codex.environment_selections().await;
    test.submit_text_turn("check the repository again").await?;
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 4);
    let (output, _) = requests[3]
        .function_call_output_content_and_success(SECOND_CALL_ID)
        .context("the unchanged next turn should receive the original failure")?;
    assert!(
        output.as_deref().is_some_and(|text| text.contains(REASON)),
        "{output:?}"
    );
    assert_eq!(test.codex.environment_selections().await, selections);
    // A retry can fail with the same provisioning error before invoking the provider.
    // Observe attachment resolution itself so preserving the error alone cannot pass.
    assert_eq!(resolutions.load(Ordering::SeqCst), initial_resolutions);
    assert_eq!(provider.calls.load(Ordering::Relaxed), 0);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unchanged_next_turn_recovers_from_transient_environment_startup_failure() -> Result<()> {
    const FIRST_CALL_ID: &str = "exec-before-recovery";
    const SECOND_CALL_ID: &str = "exec-after-recovery";
    const MARKER: &str = "recovered.txt";

    let exec_server = ExecServerProcess::start().await?;
    let upstream_address = url::Url::parse(&exec_server.websocket_url)?
        .socket_addrs(|| None)?
        .into_iter()
        .next()
        .context("exec-server should have a loopback address")?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let websocket_url = format!("ws://{}", listener.local_addr()?);
    let available = Arc::new(AtomicBool::new(false));
    let proxy_available = Arc::clone(&available);
    let mut proxy_tasks = JoinSet::new();
    proxy_tasks.spawn(async move {
        while let Ok((mut incoming, _)) = listener.accept().await {
            if !proxy_available.load(Ordering::SeqCst) {
                // Fail the handshake until the executor becomes available between turns.
                continue;
            }
            let mut outgoing = TcpStream::connect(upstream_address).await?;
            let _ = tokio::io::copy_bidirectional(&mut incoming, &mut outgoing).await;
        }
        Ok::<_, std::io::Error>(())
    });

    let server = start_mock_server().await;
    let arguments = json!({
        "cmd": format!("echo recovered > {MARKER}"),
        "login": false,
        "yield_time_ms": 10_000,
        "environment_id": REMOTE_ENVIRONMENT_ID,
    })
    .to_string();
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_function_call(FIRST_CALL_ID, "exec_command", &arguments),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-1", "The environment is unavailable."),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_function_call(SECOND_CALL_ID, "exec_command", &arguments),
                ev_completed("resp-3"),
            ]),
            sse(vec![
                ev_assistant_message("msg-2", "The command succeeded."),
                ev_completed("resp-4"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex().with_config(|config| {
        assert!(config.features.disable(Feature::DeferredExecutor).is_ok());
    });
    let test = builder.build(&server).await?;
    let manager = test.thread_manager.environment_manager();
    manager.upsert_environment(
        REMOTE_ENVIRONMENT_ID.to_string(),
        websocket_url,
        /*connect_timeout*/ None,
    )?;
    let environment = manager
        .get_environment(REMOTE_ENVIRONMENT_ID)
        .context("remote environment should be registered")?;
    let selection = TurnEnvironmentSelection {
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: PathUri::from_abs_path(&test.config.cwd),
        workspace_roots: vec![PathUri::from_abs_path(&test.config.cwd)],
        config: EnvironmentConfigState::FromThread,
        selected_capability_roots: Default::default(),
    };
    test.submit_turn_with_environments("write the marker", Some(vec![selection]))
        .await?;
    let selections = test.codex.environment_selections().await;
    let marker_path = test.config.cwd.join(MARKER);
    let first_output = response_mock
        .function_call_output_text(FIRST_CALL_ID)
        .context("model should receive the initial tool failure")?;
    assert!(
        first_output.contains("No usable execution environment is available."),
        "{first_output}"
    );
    assert!(!marker_path.as_path().exists());

    // Restore only endpoint availability. The next turn keeps the same environment and
    // workspace, without refreshing the transport or resubmitting environment settings.
    available.store(true, Ordering::SeqCst);
    test.submit_text_turn("try writing the marker again")
        .await?;
    let second_output = response_mock
        .function_call_output_text(SECOND_CALL_ID)
        .context("model should receive the recovered command result")?;
    assert!(
        second_output.contains("Process exited with code 0"),
        "the unchanged next turn should execute successfully: {second_output}"
    );
    assert!(fs::metadata(marker_path)?.len() > 0);
    assert_eq!(test.codex.environment_selections().await, selections);
    assert!(Arc::ptr_eq(
        &environment,
        &manager
            .get_environment(REMOTE_ENVIRONMENT_ID)
            .context("remote environment should remain registered")?,
    ));
    Ok(())
}
