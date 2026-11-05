//! End-to-end command lifecycle with a snapshot of its complete model-visible history.

use super::*;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::protocol::ThreadSettingsOverrides;
use core_test_support::responses;
use core_test_support::test_codex::TestCodexHarness;
use pretty_assertions::assert_eq;

// The watcher does not yield after sending ExecCommandEnd. A single worker lets it
// drop the interaction guard before this task resumes and captures another snapshot.
#[tokio::test]
async fn command_sessions_world_state_tracks_retained_processes() -> Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_wine_exec!(Ok(()), "PowerShell execution through Wine is unavailable");
    let builder = || {
        test_codex()
            .with_model("gpt-6-astra")
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(|config| {
                configure_scenario_catalog(config);
                let catalog = config
                    .model_catalog
                    .as_mut()
                    .expect("scenario model catalog");
                let mut no_polling = catalog
                    .models
                    .iter()
                    .find(|model| model.slug == "gpt-6-astra")
                    .expect("scenario model")
                    .clone();
                no_polling.slug = "no-command-polling".to_string();
                no_polling.shell_type = ConfigShellToolType::Disabled;
                catalog.models.push(no_polling);
                for feature in [
                    Feature::CodeMode,
                    Feature::CodeModeOnly,
                    Feature::CodeModeHost,
                    Feature::CommandSessionsWorldState,
                ] {
                    config
                        .features
                        .enable(feature)
                        .expect("enable scenario feature");
                }
            })
    };
    let harness = TestCodexHarness::with_auto_env_builder(builder()).await?;
    let (server, test) = (harness.server(), harness.test());
    let command = match core_test_support::test_target_os() {
        core_test_support::TestTargetOs::Linux | core_test_support::TestTargetOs::MacOs => {
            "while [ ! -f release-command ]; do sleep 0.02; done; printf finished; exit 7"
        }
        core_test_support::TestTargetOs::Windows => {
            "while (-not (Test-Path -LiteralPath 'release-command')) { Start-Sleep -Milliseconds 20 }; [Console]::Out.Write('finished'); exit 7"
        }
    };
    let command_json = serde_json::to_string(command)?;
    let script = format!(
        r#"// @exec: {{"yield_time_ms": 30000}}
const result = await tools.exec_command({{cmd: {command_json}, login: false, yield_time_ms: 250}});
text(result.output);"#
    );
    let _mock = mount_sse_sequence(
        server,
        vec![
            sse(vec![
                ev_custom_tool_call("start", "exec", &script),
                ev_completed("start"),
            ]),
            sse(vec![ev_completed("started")]),
            sse(vec![ev_completed("unchanged")]),
            sse(vec![ev_completed("polling-unavailable")]),
            sse(vec![
                json!({"type": "response.output_item.done", "item": {
                    "type": "compaction", "encrypted_content": "COMMAND_CHECKPOINT"
                }}),
                ev_completed("compact"),
            ]),
            sse(vec![ev_completed("after-compact")]),
            sse(vec![ev_completed("exited")]),
        ],
    )
    .await;
    test.submit_turn("Start the command and print only its output.")
        .await?;
    let terminals = test.codex.list_background_terminals().await;
    assert_eq!(terminals.len(), 1);
    let id = &terminals[0].process_id;
    test.submit_turn("Keep the command running.").await?;
    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some("no-command-polling".to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("Check the command while polling is unavailable.")
        .await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    // submit_turn restores the configured model, including write_stdin.
    test.submit_turn("Check the command after compaction with polling restored.")
        .await?;
    harness.write_file("release-command", "done").await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ExecCommandEnd(event) if event.process_id.as_ref() == Some(id))
    }).await;
    test.submit_turn("Check the command after it exits.")
        .await?;

    let drain = mount_sse_sequence(server, vec![
        sse(vec![ev_custom_tool_call("drain", "exec", &format!(
            "const result = await tools.write_stdin({{session_id: {id}, chars: \"\"}}); text({{output: result.output, exit_code: result.exit_code}});"
        )), ev_completed("drain")]),
        sse(vec![ev_completed("done")]),
    ]).await;
    test.submit_turn("Collect the final command result.")
        .await?;
    assert!(test.codex.list_background_terminals().await.is_empty());
    let output = super::super::code_mode::custom_tool_output_last_non_empty_text(
        &drain.last_request().expect("drained command request"),
        "drain",
    )
    .expect("final command result");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&output)?,
        json!({"output": "finished", "exit_code": 7})
    );

    // Persist a live session, then restart without inheriting its process manager.
    let _restart = mount_sse_sequence(
        server,
        vec![
            sse(vec![
                ev_custom_tool_call(
                    "restart-start",
                    "exec",
                    &script.replace("release-command", "release-second-command"),
                ),
                ev_completed("restart-start"),
            ]),
            sse(vec![ev_completed("restart-running")]),
            sse(vec![ev_completed("restarted")]),
        ],
    )
    .await;
    test.submit_turn("Start another command before restarting.")
        .await?;
    let terminals = test.codex.list_background_terminals().await;
    assert_eq!(terminals.len(), 1);
    let restarted_id = &terminals[0].process_id;
    let resumed = builder().restart_with_auto_env(server, test).await?;
    resumed
        .submit_turn("Check command sessions after restarting.")
        .await?;
    assert!(resumed.codex.list_background_terminals().await.is_empty());
    resumed.codex.shutdown_and_wait().await?;
    let rollout = fs::read_to_string(resumed.codex.rollout_path().expect("resumed rollout"))?
        .lines()
        .map(codex_rollout::parse_rollout_line)
        .collect::<Result<Vec<_>, _>>()?;
    let cleared = rollout
        .iter()
        .rev()
        .find_map(|line| match &line.item {
            RolloutItem::WorldState(state) => state.state.get("command_sessions"),
            _ => None,
        })
        .expect("command session checkpoint");
    assert_eq!(cleared, &json!({"sessions": {restarted_id: null}}));

    // Command text is platform-specific; lifecycle assertions run on every native executor.
    if core_test_support::test_target_os() != core_test_support::TestTargetOs::Windows {
        insta::assert_snapshot!(
            "command_sessions_world_state",
            context_snapshot::format_request_history_snapshot(
                "A Code Mode script drops its command handle. WorldState retains it while polling is unavailable, restores it after compaction, preserves exited results, clears drained sessions, and deletes stale handles after restart.",
                &responses::received_responses_requests(server).await,
                &ContextSnapshotOptions::default().rewrite_known_segments(),
            )
        );
    }
    Ok(())
}
