//! Host loss must reach the model without replaying completed external effects.

use super::*;
use codex_extension_api::ToolCallSource;
use core_test_support::test_codex::TestCodexHarness;
use pretty_assertions::assert_eq;

struct PauseCompletedCommand {
    gate: Mutex<Option<(oneshot::Sender<ToolCallOutcome>, oneshot::Receiver<()>)>>,
}

impl ToolLifecycleContributor for PauseCompletedCommand {
    fn on_tool_finish<'a>(&'a self, input: ToolFinishInput<'a>) -> ToolLifecycleFuture<'a> {
        Box::pin(async move {
            if input.tool_name.name != "exec_command"
                || !matches!(input.source, ToolCallSource::CodeMode { .. })
            {
                return;
            }
            let gate = self.gate.lock().unwrap().take();
            if let Some((reached, release)) = gate {
                let _ = reached.send(input.outcome);
                // Keep the host executing until the test kills it: the external
                // command completed, but its result has not reached JavaScript.
                let _ = release.await;
            }
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grpc_host_loss_reports_reset_without_replaying_side_effects() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let recorder = CodeModeHostRecorder::new()?;
    let (reached_tx, reached_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let mut extensions = ExtensionRegistryBuilder::<Config>::new();
    extensions.tool_lifecycle_contributor(Arc::new(PauseCompletedCommand {
        gate: Mutex::new(Some((reached_tx, release_rx))),
    }));
    let builder = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_code_mode_host_program(recorder.program())
        .with_config(|config| {
            config.features.enable(Feature::CodeModeOnly).unwrap();
            config.features.enable(Feature::CodeModeHostGrpc).unwrap();
        });
    let harness = TestCodexHarness::with_auto_env_builder(builder).await?;
    let test = harness.test();
    let server = harness.server();
    let (append, read) = match core_test_support::test_target_os() {
        core_test_support::TestTargetOs::Windows => (
            "[IO.File]::AppendAllText((Join-Path (Get-Location) 'effects.txt'), 'x')",
            "[Console]::Write([IO.File]::ReadAllText((Join-Path (Get-Location) 'effects.txt')))",
        ),
        core_test_support::TestTargetOs::Linux | core_test_support::TestTargetOs::MacOs => {
            ("printf x >> effects.txt", "cat effects.txt")
        }
    };
    let interrupted = format!(
        r#"// @exec: {{"yield_time_ms": 60000}}
store("before-crash", true);
await tools.exec_command({{ cmd: {append:?} }});
text("unexpected success");"#,
    );
    let fresh = format!(
        r#"const result = await tools.exec_command({{ cmd: {read:?} }});
text(JSON.stringify({{ previous: load("before-crash") ?? null, output: result.output }}));"#,
    );
    let mut requests = Vec::new();
    for (call_id, source) in [
        ("crashed", &interrupted),
        ("reset", &interrupted),
        ("fresh", &fresh),
    ] {
        requests.push(
            responses::mount_sse_once(
                server,
                sse(vec![
                    ev_response_created(call_id),
                    ev_custom_tool_call(call_id, "exec", source),
                    ev_completed(call_id),
                ]),
            )
            .await,
        );
    }
    let completion = responses::mount_sse_once(
        server,
        sse(vec![
            ev_assistant_message("done", "recovered"),
            ev_completed("done"),
        ]),
    )
    .await;

    let crash = async {
        assert_eq!(
            reached_rx.await?,
            ToolCallOutcome::Completed { success: true }
        );
        assert_eq!(harness.read_file_text("effects.txt").await?, "x");
        assert_eq!(
            recorder.invocations()?,
            vec!["--listen grpc+stdio://".to_string()]
        );
        let pid = recorder.pid()?;
        assert_eq!(unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) }, 0);
        let _ = release_tx.send(());
        Ok::<_, anyhow::Error>(())
    };
    let (turn_result, crash_result) =
        tokio::time::timeout(Duration::from_secs(/*secs*/ 30), async {
            tokio::join!(
                test.submit_turn("Run a command and recover if the host fails"),
                crash
            )
        })
        .await
        .context("host-loss recovery stalled")?;
    crash_result?;
    turn_result?;

    // The inference requests must expose the failure and the next admission's
    // reset warning. The API serializes these errors as text, without a success flag.
    let (failure, _) =
        custom_tool_output_body_and_success(&requests[1].single_request(), "crashed");
    assert!(
        failure.contains("gRPC code-mode") || failure.contains("exec runtime ended unexpectedly"),
        "{failure}"
    );
    let (reset, _) = custom_tool_output_body_and_success(&requests[2].single_request(), "reset");
    assert!(
        reset.contains("The exec runtime was replaced. Stored values and running cells were lost."),
        "{reset}"
    );
    assert!(
        reset.contains("Prior external tool operations may have completed; inspect their results before retrying."),
        "{reset}"
    );
    let (output, success) =
        custom_tool_output_body_and_success(&completion.single_request(), "fresh");
    assert_ne!(success, Some(false), "{output}");
    assert_eq!(
        serde_json::from_str::<Value>(&output)?,
        serde_json::json!({ "previous": null, "output": "x" })
    );
    assert_eq!(harness.read_file_text("effects.txt").await?, "x");
    assert_eq!(
        recorder.invocations()?,
        vec!["--listen grpc+stdio://".to_string(); 2]
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
