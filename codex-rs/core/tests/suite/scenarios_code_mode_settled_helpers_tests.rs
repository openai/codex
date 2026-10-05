//! Snapshots settlement records delivered to the model after code-mode execution.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn code_mode_settled_helpers_report_successes_and_failures() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_model("gpt-6-astra")
        .with_config(|config| {
            configure_scenario_catalog(config);
            config.workspace_roots = vec![config.cwd.clone()];
            for feature in [
                Feature::CodeMode,
                Feature::CodeModeOnly,
                Feature::CodeModeHost,
            ] {
                config.features.enable(feature).expect("enable code mode");
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("exec-response"),
                ev_custom_tool_call(
                    "exec-call",
                    "exec",
                    r#"for await (const result of as_settled([Promise.resolve("ready")])) text(result);
await stream_settled(new Map([["failed", Promise.reject("unavailable")], ["completed", "done"]]), text);"#,
                ),
                ev_completed("exec-response"),
            ]),
            sse(vec![
                ev_assistant_message("final", "Two inputs succeeded; one failed with unavailable."),
                ev_completed("final-response"),
            ]),
        ],
    )
    .await;
    test.submit_turn(
        "Collect each completed result, including failures, using the settlement helpers.",
    )
    .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let output = requests[1].custom_tool_call_output("exec-call");
    let output = output.to_string();
    assert!(output.contains("fulfilled"), "{output}");
    assert!(output.contains("rejected"), "{output}");
    insta::assert_snapshot!(
        "code_mode_settled_helpers",
        context_snapshot::format_request_history_snapshot(
            "Settlement helpers deliver fulfilled and rejected records with input indices and map keys.",
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}
