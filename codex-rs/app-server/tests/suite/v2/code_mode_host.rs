use std::process::Stdio;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use codex_app_server_protocol::ThreadDecrementElicitationParams;
use codex_app_server_protocol::ThreadDecrementElicitationResponse;
use codex_app_server_protocol::ThreadGoalSetResponse;
use codex_app_server_protocol::ThreadGoalStatus;
use codex_app_server_protocol::ThreadGoalUpdatedNotification;
use codex_app_server_protocol::ThreadIncrementElicitationParams;
use codex_app_server_protocol::ThreadIncrementElicitationResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_features::Feature;
use core_test_support::responses;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use test_case::test_case;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::timeout;

#[cfg(any(target_os = "macos", windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 60);
#[cfg(not(any(target_os = "macos", windows)))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(/*secs*/ 10);

enum TimingOutput {
    HostOnly,
    WithOverhead,
}

/// Responses with overhead use the same host and handler measurements as Bridge,
/// including app-server waiting. The default shows only host duration.
#[test_case("exec", "grpc", TimingOutput::HostOnly; "exec_grpc_host_only")]
#[test_case("wait", "grpc", TimingOutput::HostOnly; "wait_grpc_host_only")]
#[test_case("exec", "stdio", TimingOutput::HostOnly; "exec_stdio_host_only")]
#[test_case("wait", "stdio", TimingOutput::HostOnly; "wait_stdio_host_only")]
#[test_case("exec", "grpc", TimingOutput::WithOverhead; "exec_grpc_with_overhead")]
#[test_case("wait", "grpc", TimingOutput::WithOverhead; "wait_grpc_with_overhead")]
#[test_case("exec", "stdio", TimingOutput::WithOverhead; "exec_stdio_with_overhead")]
#[test_case("wait", "stdio", TimingOutput::WithOverhead; "wait_stdio_with_overhead")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_model_output_uses_structured_host_timing(
    tool_name: &str,
    transport: &str,
    timing_output: TimingOutput,
) -> Result<()> {
    let experimental_show_cell_overhead = matches!(timing_output, TimingOutput::WithOverhead);
    let mut host = match transport {
        "grpc" => Some(
            Command::new(codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?)
                .args(["--listen", "grpc://127.0.0.1:0"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .kill_on_drop(/*kill_on_drop*/ true)
                .spawn()?,
        ),
        "stdio" => None,
        _ => anyhow::bail!("unknown host transport: {transport}"),
    };
    let host_url = if let Some(host) = host.as_mut() {
        let stdout = host.stdout.take().context("host stdout unavailable")?;
        Some(
            timeout(
                DEFAULT_READ_TIMEOUT,
                BufReader::new(stdout).lines().next_line(),
            )
            .await??
            .context("host exited before publishing its URL")?,
        )
    } else {
        None
    };

    let model_server = responses::start_mock_server().await;
    let invocation = if tool_name == "exec" {
        responses::ev_custom_tool_call(
            "timed-call",
            "exec",
            "await new Promise(resolve => setTimeout(resolve, 100)); text('timed');",
        )
    } else {
        responses::ev_function_call("timed-call", "wait", r#"{"cell_id":"missing"}"#)
    };
    responses::mount_sse_once(
        &model_server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            invocation,
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;
    let follow_up = responses::mount_sse_once(
        &model_server,
        responses::sse(vec![
            responses::ev_assistant_message("msg-1", "Done"),
            responses::ev_completed("resp-2"),
        ]),
    )
    .await;
    let codex_home = TempDir::new()?;
    let mut config =
        MockResponsesConfig::new(&model_server.uri()).enable_feature(Feature::CodeModeOnly);
    if experimental_show_cell_overhead {
        config = config
            .with_extra_config("[features.code_mode]\nexperimental_show_cell_overhead = true");
    }
    config.write(codex_home.path())?;
    let mut builder = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_json_logging("codex_code_mode::timing=info,codex_core::tools::parallel=info");
    if let Some(host_url) = host_url.as_deref() {
        builder = builder.with_args(&["--code-mode-host", host_url]);
    }
    let mut app_server = builder.build_initialized().await?;
    let thread = app_server
        .start_thread(ThreadStartParams::default())
        .await?;
    let increment_id = app_server
        .send_request(
            "thread/increment_elicitation",
            Some(serde_json::to_value(ThreadIncrementElicitationParams {
                thread_id: thread.thread.id.clone(),
            })?),
        )
        .await?;
    let _: ThreadIncrementElicitationResponse = app_server.read_response(increment_id).await?;
    let start_id = app_server
        .send_turn_start_request(TurnStartParams {
            thread_id: thread.thread.id.clone(),
            input: vec![UserInput::Text {
                text: "run the timed operation".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        })
        .await?;
    let _: TurnStartResponse = app_server.read_response(start_id).await?;
    let host_timing = app_server
        .wait_for_json_log_event("codex.code_mode.host_timing")
        .await?;
    // Hold only the app-server after the host outcome, so local elapsed time
    // cannot round to the same displayed duration as the host measurement.
    tokio::time::sleep(Duration::from_millis(/*millis*/ 300)).await;
    assert!(
        follow_up.requests().is_empty(),
        "the model must not receive the result while elicitation is outstanding"
    );
    let decrement_id = app_server
        .send_request(
            "thread/decrement_elicitation",
            Some(serde_json::to_value(ThreadDecrementElicitationParams {
                thread_id: thread.thread.id.clone(),
            })?),
        )
        .await?;
    let _: ThreadDecrementElicitationResponse = app_server.read_response(decrement_id).await?;
    let completed: TurnCompletedNotification =
        app_server.read_notification("turn/completed").await?;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    let host_fields = &host_timing["fields"];
    let code_mode_host_duration_ns = host_fields["code_mode_host_duration_ns"]
        .as_u64()
        .context("missing host timing")?;
    assert_eq!(host_fields["conversation_id"], thread.thread.id);
    assert_eq!(host_fields["turn_id"], completed.turn.id);
    assert_eq!(host_fields["call_id"], "timed-call");
    assert_eq!(host_fields["tool_name"], tool_name);
    let handler_timing = app_server
        .wait_for_json_log_event("codex.tool_call")
        .await?;
    let handler_fields = &handler_timing["fields"];
    let handler_duration_ms = handler_fields["handler_duration_ms"]
        .as_u64()
        .context("missing handler timing")?;
    assert_eq!(
        [
            &handler_fields["conversation.id"],
            &handler_fields["turn_id"],
            &handler_fields["call_id"],
            &handler_fields["tool_name"],
        ],
        [
            &host_fields["conversation_id"],
            &host_fields["turn_id"],
            &host_fields["call_id"],
            &host_fields["tool_name"],
        ]
    );
    assert_eq!(handler_fields["tool_source"], "direct");
    assert_eq!(handler_fields["execution_started"], true);
    // The host stopped its timer before the elicitation hold. The outer handler
    // must include that hold, with some allowance for its millisecond precision.
    let overhead_ns =
        i128::from(handler_duration_ms) * 1_000_000 - i128::from(code_mode_host_duration_ns);
    assert!(
        overhead_ns >= 250_000_000,
        "handler timing omitted the post-host elicitation hold: {overhead_ns}ns"
    );
    let request = follow_up.single_request();
    let output = if tool_name == "exec" {
        request.custom_tool_call_output("timed-call")
    } else {
        request.function_call_output("timed-call")
    };
    let status = if tool_name == "exec" {
        "Script completed"
    } else {
        "Script failed"
    };
    let expected_header = if experimental_show_cell_overhead {
        let total_seconds = Duration::from_millis(handler_duration_ms).as_secs_f64();
        let host_seconds = Duration::from_nanos(code_mode_host_duration_ns).as_secs_f64();
        let overhead_seconds = overhead_ns as f64 / 1_000_000_000.0;
        format!(
            "{status}\nWall time {total_seconds:.3} seconds (code-mode {host_seconds:.3} seconds; overhead {overhead_seconds:.3} seconds)\nOutput:\n"
        )
    } else {
        let seconds = Duration::from_nanos(code_mode_host_duration_ns).as_secs_f32();
        let seconds = (seconds * 10.0).round() / 10.0;
        format!("{status}\nWall time {seconds:.1} seconds\nOutput:\n")
    };
    assert_eq!(output["output"][0]["text"], expected_header);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_server_shares_flag_selected_grpc_code_mode_host_across_threads() -> Result<()> {
    let host_program = codex_utils_cargo_bin::cargo_bin("codex-code-mode-host")?;
    let mut code_mode_host = Command::new(host_program)
        .args(["--listen", "grpc://127.0.0.1:0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .context("failed to start remote code-mode host")?;
    let stdout = code_mode_host
        .stdout
        .take()
        .context("remote code-mode host stdout was not captured")?;
    let mut lines = BufReader::new(stdout).lines();
    let host_url = timeout(DEFAULT_READ_TIMEOUT, lines.next_line())
        .await
        .context("timed out waiting for remote code-mode host URL")??
        .context("remote code-mode host exited before publishing its URL")?;

    let model_server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_sequence(
        &model_server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-1"),
                responses::ev_custom_tool_call(
                    "first-remote-cell",
                    "exec",
                    "text('remote app-server host')",
                ),
                responses::ev_completed("resp-1"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-1", "Done"),
                responses::ev_completed("resp-2"),
            ]),
            responses::sse(vec![
                responses::ev_response_created("resp-3"),
                responses::ev_custom_tool_call(
                    "second-remote-cell",
                    "exec",
                    "text('remote app-server host')",
                ),
                responses::ev_completed("resp-3"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("msg-2", "Done"),
                responses::ev_completed("resp-4"),
            ]),
        ],
    )
    .await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&model_server.uri())
        .enable_feature(Feature::CodeModeOnly)
        .enable_feature(Feature::CodeModePrewarm)
        .write(codex_home.path())?;
    let original_config = std::fs::read_to_string(codex_home.path().join("config.toml"))?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_args(&["--code-mode-host", &host_url])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;

    for prompt in ["run the first remote cell", "run the second remote cell"] {
        let thread = app_server
            .start_thread(ThreadStartParams::default())
            .await?;
        let completed = timeout(
            DEFAULT_READ_TIMEOUT,
            app_server.start_turn_and_wait_for_completion(TurnStartParams {
                thread_id: thread.thread.id,
                input: vec![UserInput::Text {
                    text: prompt.to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            }),
        )
        .await??;

        assert_eq!(completed.turn.status, TurnStatus::Completed);
    }

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 4);
    for (request, call_id) in [
        (&requests[1], "first-remote-cell"),
        (&requests[3], "second-remote-cell"),
    ] {
        let output = request.custom_tool_call_output(call_id);
        assert_eq!(
            output["output"]
                .as_array()
                .and_then(|items| items.last())
                .cloned(),
            Some(json!({
                "type": "input_text",
                "text": "remote app-server host",
            }))
        );
    }
    assert_eq!(
        std::fs::read_to_string(codex_home.path().join("config.toml"))?,
        original_config
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_server_prewarms_flag_selected_grpc_code_mode_host_before_first_turn() -> Result<()> {
    let model_server = responses::start_mock_server().await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&model_server.uri())
        .enable_feature(Feature::CodeModePrewarm)
        .write(codex_home.path())?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let host_url = format!("http://{}", listener.local_addr()?);
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_args(&["--code-mode-host", &host_url])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    app_server
        .start_thread(ThreadStartParams::default())
        .await?;

    let (_stalled_connection, _) = timeout(DEFAULT_READ_TIMEOUT, listener.accept())
        .await
        .context("code-mode host was not contacted before the first turn")??;
    let status = timeout(Duration::from_secs(5), app_server.shutdown_gracefully())
        .await
        .context("stalled code-mode prewarm blocked thread shutdown")??;
    assert!(status.success(), "app-server did not exit successfully");
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn app_server_blocks_goal_after_repeated_code_mode_host_failures() -> Result<()> {
    let model_server = responses::start_mock_server().await;
    let mut model_responses = Vec::new();
    for turn in 1..=3 {
        model_responses.push(responses::sse(vec![
            responses::ev_response_created(&format!("resp-{turn}-exec")),
            responses::ev_custom_tool_call(
                &format!("call-exec-{turn}"),
                "exec",
                "text('unreachable')",
            ),
            responses::ev_completed(&format!("resp-{turn}-exec")),
        ]));
        model_responses.push(responses::sse(vec![
            responses::ev_assistant_message(
                &format!("msg-{turn}"),
                "The execution host is unavailable.",
            ),
            responses::ev_completed(&format!("resp-{turn}-done")),
        ]));
    }
    let response_mock = responses::mount_sse_sequence(&model_server, model_responses).await;

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&model_server.uri())
        .enable_feature(Feature::CodeModeOnly)
        .enable_feature(Feature::Goals)
        .enable_feature(Feature::Sqlite)
        .write(codex_home.path())?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let host_url = format!("http://{}", listener.local_addr()?);
    drop(listener);
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_args(&["--code-mode-host", &host_url])
        .build_initialized_with_timeout(DEFAULT_READ_TIMEOUT)
        .await?;
    let thread = app_server
        .start_thread(ThreadStartParams::default())
        .await?;
    let goal_request = app_server
        .send_raw_request(
            "thread/goal/set",
            Some(json!({
                "threadId": thread.thread.id,
                "objective": "finish the task",
                "status": "active",
            })),
        )
        .await?;
    let _: ThreadGoalSetResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(goal_request)).await??;

    let goal = timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let notification: ThreadGoalUpdatedNotification =
                app_server.read_notification("thread/goal/updated").await?;
            if notification.goal.status == ThreadGoalStatus::Blocked {
                return Ok::<_, anyhow::Error>(notification.goal);
            }
        }
    })
    .await??;

    assert_eq!(goal.status, ThreadGoalStatus::Blocked);
    assert_eq!(response_mock.requests().len(), 6);
    Ok(())
}
