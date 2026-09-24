//! Scheduled repetitions preserve human overrides without flooding review evidence.

use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_features::Feature;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::turn_input::TurnInputSubmission;
use codex_protocol::turn_input::TurnStartOptions;
use codex_protocol::user_input::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::SnapshotEntry;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::json;
use std::time::Duration;
use tokio::sync::oneshot;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn heartbeat_repetitions_keep_later_human_authorization() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let replies = (0..4)
        .map(|index| {
            responses::sse(vec![
                responses::ev_assistant_message(&format!("actor-{index}"), "Recorded this input."),
                responses::ev_completed(&format!("done-{index}")),
            ])
        })
        .chain([
        responses::sse(vec![responses::ev_function_call("check", "exec_command", r#"{"cmd":"exit 0","sandbox_permissions":"require_escalated","justification":"Check the explicitly requested change."}"#), responses::ev_completed("action")]),
        // Exercise review construction without executing an unsandboxed command.
        responses::sse(vec![responses::ev_assistant_message("assessment", &json!({"risk_level":"high","user_authorization":"low","outcome":"deny","rationale":"Mock decision."}).to_string()), responses::ev_completed("review")]),
        responses::sse(vec![responses::ev_assistant_message("result", "The mock review completed."), responses::ev_completed("done")]),
    ]);
    let (release, gate) = oneshot::channel();
    let mut release = Some(release);
    let mut streams = replies
        .map(|body| vec![StreamingSseChunk { gate: None, body }])
        .collect::<Vec<_>>();
    // Keep the human override's turn active until the scheduled repeat steers it.
    streams[2][0].gate = Some(gate);
    let (streaming, _) = start_streaming_sse_server(streams).await;
    let base_url = format!("{}/v1", streaming.uri());
    let test = test_codex()
        .with_model("gpt-5.5")
        .with_config(move |config| {
            super::configure_scenario_catalog(config);
            config.model_provider.base_url = Some(base_url);
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .unwrap();
            config
                .features
                .enable(Feature::GuardianThreadContext)
                .unwrap();
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::read_only())
                .unwrap();
        })
        .build_with_auto_env(&server)
        .await?;

    for index in 0..4 {
        let (prompt, trigger) = if index == 2 {
            ("Create a new worktree for the fix; this overrides the monitor's no-worktree restriction.".to_owned(), "composer")
        } else {
            (
                format!(
                    "<heartbeat>\n  <automation_id>monitor</automation_id>\n  <current_time_iso>2026-01-01T00:{index:02}:00Z</current_time_iso>\n  <instructions>\nMonitor only; do not create worktrees.\n  </instructions>\n</heartbeat>\n"
                ),
                "automation_heartbeat_scheduled",
            )
        };
        let submission = test
            .codex
            .start_or_steer_turn(
                TurnInputRequest::user_input(vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }])
                .on_start(TurnStartOptions {
                    turn_trigger: Some(trigger.to_owned()),
                    ..Default::default()
                }),
            )
            .await?;
        if index == 2 {
            tokio::time::timeout(
                Duration::from_secs(/*secs*/ 10),
                streaming.wait_for_request_count(/*count*/ 3),
            )
            .await?;
            continue;
        }
        if index == 3 {
            assert!(matches!(submission, TurnInputSubmission::Steered { .. }));
            release.take().unwrap().send(()).unwrap();
        }
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }
    test.submit_text_turn("Check the fix in the requested worktree.")
        .await?;
    let requests = streaming
        .requests()
        .await
        .iter()
        .map(|body| serde_json::from_slice(body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    let entries = requests.iter().map(SnapshotEntry::body).collect::<Vec<_>>();
    let mut snapshot = context_snapshot::format_context_snapshot(
        "Three scheduled runs share one instruction body, a human overrides its restriction between runs, and the last scheduled run steers the active human turn. Guardian reviews the later action with the override intact. The assessment is mocked.",
        &entries,
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    );
    for (pattern, replacement) in [
        (
            r#"(?m)^(\s*"environment_id": )"(?:local|remote)""#,
            "$1\"<ENVIRONMENT>\"",
        ),
        (
            r#"(The active permission profile for environment )"(?:local|remote)""#,
            "$1\"<ENVIRONMENT>\"",
        ),
        (r#"(?m)^(\s*"cwd": )"[^"]*""#, "$1\"<CWD>\""),
        (
            r#""command": \[\s*(?:"[^"]*",\s*)*"exit 0"\s*\]"#,
            "\"command\": [\"<SHELL>\", \"exit 0\"]",
        ),
    ] {
        snapshot = regex_lite::Regex::new(pattern)?
            .replace_all(&snapshot, replacement)
            .into_owned();
    }
    insta::assert_snapshot!(
        "heartbeat_repetitions_keep_later_human_authorization",
        snapshot
    );
    streaming.shutdown().await;
    Ok(())
}
