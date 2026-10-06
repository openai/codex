//! Request-history coverage for adopting incremental tools when initial context is rebuilt.

use anyhow::Result;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use serde_json::json;
use test_case::test_case;

fn builder(incremental_tools: bool, token_budget: bool) -> TestCodexBuilder {
    test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model_info_override("gpt-5.4", |model| {
            model.use_responses_lite = true;
            model.tool_mode = Some(ToolMode::CodeMode);
        })
        .with_config(move |config| {
            if incremental_tools {
                config
                    .features
                    .enable(Feature::IncrementalTools)
                    .expect("enable incremental tools");
            } else {
                config
                    .features
                    .disable(Feature::IncrementalTools)
                    .expect("disable incremental tools");
            }
            if token_budget {
                config
                    .features
                    .enable(Feature::TokenBudget)
                    .expect("enable token budget");
            }
            config.base_instructions =
                Some("Use the available tools to help the user.".to_string());
            config.model_auto_compact_token_limit = Some(100_000);
            config.code_mode.disable_in_process_fallback = true;
        })
}

#[test_case(true; "window_reset")]
#[test_case(false; "remote_compaction")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_resume_switches_only_after_window_replacement(token_budget: bool) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mut replies = (1..=2)
        .map(|i| {
            responses::sse(vec![responses::ev_completed_with_tokens(
                &format!("before-{i}"),
                if i == 2 && !token_budget { 100_001 } else { 0 },
            )])
        })
        .collect::<Vec<_>>();
    if !token_budget {
        replies.push(responses::sse(vec![
            json!({"type": "response.output_item.done", "item": {"type": "compaction", "encrypted_content": "COMPACTED_HISTORY"}}),
            responses::ev_completed("compact"),
        ]));
    }
    replies.extend(
        (1..=4).map(|i| responses::sse(vec![responses::ev_completed(&format!("after-{i}"))])),
    );
    let mock = responses::mount_sse_sequence(&server, replies).await;
    let initial = builder(/*incremental_tools*/ false, token_budget)
        .build_with_auto_env(&server)
        .await?;
    initial
        .submit_turn("Begin with the legacy tool prefix.")
        .await?;
    let resumed = builder(/*incremental_tools*/ true, token_budget)
        .restart_with_auto_env(&server, &initial)
        .await?;
    resumed
        .submit_turn("Resume without moving the tool prefix.")
        .await?;
    if token_budget {
        resumed.codex.submit(Op::Compact).await?;
        wait_for_event(&resumed.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
    }
    // A window reset survives restart; remote compaction runs before the next turn.
    let migrated = builder(/*incremental_tools*/ true, token_budget)
        .restart_with_auto_env(&server, &resumed)
        .await?;
    migrated
        .submit_turn("Continue in the new window with incremental tools.")
        .await?;
    let migrated = builder(/*incremental_tools*/ true, token_budget)
        .restart_with_auto_env(&server, &migrated)
        .await?;
    migrated
        .submit_turn("Resume with the new window unchanged.")
        .await?;
    let changed = builder(/*incremental_tools*/ true, token_budget)
        .with_config(|config| {
            config.update_plan_enabled = true;
            config
                .features
                .disable(Feature::ShellTool)
                .expect("disable shell");
        })
        .restart_with_auto_env(&server, &migrated)
        .await?;
    changed
        .submit_turn("Enable planning and remove shell tools after migration.")
        .await?;
    changed.submit_turn("Continue with the same tools.").await?;
    let requests = mock.requests();
    insta::assert_snapshot!(
        if token_budget {
            "legacy_resume_window_reset"
        } else {
            "legacy_resume_remote_compaction"
        },
        context_snapshot::format_request_history_snapshot(
            "A resumed legacy window retains its generated prefix; the next window records tools and instructions once, then emits only tool changes after resume.",
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disable_incremental_tools_at_next_window() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![responses::ev_completed("initial")]),
            responses::sse(vec![responses::ev_completed("disabled")]),
            responses::sse(vec![
                json!({"type": "response.output_item.done", "item": {"type": "compaction", "encrypted_content": "COMPACTED_HISTORY"}}),
                responses::ev_completed("compact"),
            ]),
            responses::sse(vec![responses::ev_completed("new-window")]),
            responses::sse(vec![responses::ev_completed("resumed")]),
        ],
    )
    .await;
    let session_builder = |enabled| {
        builder(enabled, /*token_budget*/ false).with_model_info_override("gpt-5.4", |model| {
            model.use_responses_lite = true;
            model.tool_mode = Some(ToolMode::Direct);
        })
    };
    let initial = session_builder(/*enabled*/ true)
        .build_with_auto_env(&server)
        .await?;
    initial.submit_turn("Begin with all tools.").await?;
    let disabled_builder = || {
        session_builder(/*enabled*/ false).with_config(|config| {
            config
                .features
                .disable(Feature::ShellTool)
                .expect("disable shell");
        })
    };
    let disabled = disabled_builder()
        .restart_with_auto_env(&server, &initial)
        .await?;
    disabled
        .submit_turn("Disable incremental tools and remove shell tools in the existing window.")
        .await?;
    disabled.codex.submit(Op::Compact).await?;
    wait_for_event(&disabled.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    disabled.submit_turn("Continue in the new window.").await?;
    let resumed = disabled_builder()
        .restart_with_auto_env(&server, &disabled)
        .await?;
    resumed.submit_turn("Resume with the same tools.").await?;
    insta::assert_snapshot!(
        "disable_incremental_tools",
        context_snapshot::format_request_history_snapshot(
            "Disabling incremental tools preserves updates in the existing window; after compaction the generated catalog survives resume.",
            &mock.requests(),
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    Ok(())
}
