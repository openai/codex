//! Request-history coverage for incremental tool modes across resume, fork, and window rebuilds.

use anyhow::Result;
use codex_core::ForkSnapshot;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_successful_turn;
use pretty_assertions::assert_eq;
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
        (1..=5).map(|i| responses::sse(vec![responses::ev_completed(&format!("after-{i}"))])),
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
    let resumed_update = builder(/*incremental_tools*/ true, token_budget)
        .with_config(|config| {
            config.update_plan_enabled = true;
            config
                .features
                .disable(Feature::ShellTool)
                .expect("disable shell");
        })
        .restart_with_auto_env(&server, &changed)
        .await?;
    resumed_update
        .submit_turn("Resume with the same updated tools.")
        .await?;
    let requests = mock.requests();
    insta::assert_snapshot!(
        if token_budget {
            "legacy_resume_window_reset"
        } else {
            "legacy_resume_remote_compaction"
        },
        context_snapshot::format_request_history_snapshot(
            "A resumed legacy window retains its generated prefix; rebuilding the next window emits no incremental notice. A later update appends one developer notice, changed definitions, then the removal notice. Unchanged turns and another restart preserve that pair without duplication.",
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
            config.update_plan_enabled = true;
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
        .submit_turn("Disable incremental tools, enable planning, and remove shell tools in the existing window.")
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
    let requests = mock.requests();
    insta::assert_snapshot!(
        "disable_incremental_tools",
        context_snapshot::format_request_history_snapshot(
            "Disabling incremental tools preserves updates in the existing window; after compaction the generated catalog survives resume.",
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_tools_follow_current_settings_across_history() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let replies = (0..13)
        .map(|index| {
            let mut events = Vec::new();
            if index == 9 {
                events.push(json!({"type": "response.output_item.done", "item": {"type": "compaction", "encrypted_content": "COMPACTED_HISTORY"}}));
            }
            events.push(responses::ev_completed(&format!("lifecycle-{index}")));
            responses::sse(events)
        })
        .collect();
    let mock = responses::mount_sse_sequence(&server, replies).await;
    let session_builder = |enabled, mode, planning, changed_catalog| {
        builder(/*incremental_tools*/ true, /*token_budget*/ false)
            .with_model_info_override("gpt-5.4", move |model| {
                model.use_responses_lite = true;
                model.tool_mode = Some(mode);
            })
            .with_config(move |config| {
                config
                    .features
                    .set_enabled(Feature::IncrementalExecTools, enabled)
                    .expect("set incremental exec tools");
                config.update_plan_enabled = planning;
                if changed_catalog {
                    config.features.disable(Feature::ShellTool).unwrap();
                }
            })
    };
    let initial = session_builder(
        /*enabled*/ true,
        ToolMode::CodeModeOnly,
        /*planning*/ false,
        /*changed_catalog*/ false,
    )
    .build_with_auto_env(&server)
    .await?;
    initial
        .submit_turn("Begin with an incremental exec catalog.")
        .await?;
    let before_updates = session_builder(
        /*enabled*/ false,
        ToolMode::CodeModeOnly,
        /*planning*/ false,
        /*changed_catalog*/ false,
    )
    .restart_with_auto_env(&server, &initial)
    .await?;
    before_updates
        .submit_turn("Disable granular updates before any catalog change.")
        .await?;
    let enabled = session_builder(
        /*enabled*/ true,
        ToolMode::CodeModeOnly,
        /*planning*/ false,
        /*changed_catalog*/ false,
    )
    .restart_with_auto_env(&server, &before_updates)
    .await?;
    enabled
        .submit_turn("Re-enable granular updates with the same catalog.")
        .await?;
    let mixed = session_builder(
        /*enabled*/ true,
        ToolMode::CodeModeOnly,
        /*planning*/ true,
        /*changed_catalog*/ true,
    )
    .restart_with_auto_env(&server, &enabled)
    .await?;
    mixed
        .submit_turn("Add planning and remove shell tools.")
        .await?;
    let disabled_after_changes = session_builder(
        /*enabled*/ false,
        ToolMode::CodeModeOnly,
        /*planning*/ true,
        /*changed_catalog*/ true,
    )
    .restart_with_auto_env(&server, &mixed)
    .await?;
    disabled_after_changes
        .submit_turn("Disable granular updates while retaining those catalog changes.")
        .await?;
    let hybrid = session_builder(
        /*enabled*/ true,
        ToolMode::CodeMode,
        /*planning*/ false,
        /*changed_catalog*/ false,
    )
    .restart_with_auto_env(&server, &disabled_after_changes)
    .await?;
    hybrid
        .submit_turn("Resume in hybrid mode with the catalog flag enabled.")
        .await?;
    let returned = session_builder(
        /*enabled*/ false,
        ToolMode::CodeModeOnly,
        /*planning*/ false,
        /*changed_catalog*/ false,
    )
    .restart_with_auto_env(&server, &hybrid)
    .await?;
    returned
        .submit_turn("Return to CodeModeOnly with planning still removed.")
        .await?;

    returned.codex.flush_rollout().await?;
    let mut fork_config = returned.config.clone();
    fork_config.update_plan_enabled = true;
    let forked = returned
        .thread_manager
        .fork_legacy_thread(
            ForkSnapshot::Interrupted,
            StartThreadOptions::new(fork_config),
            returned
                .codex
                .rollout_path()
                .expect("persisted parent rollout"),
        )
        .await?;
    forked
        .thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Fork with planning enabled and granular updates disabled.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_successful_turn(&forked.thread).await;
    forked.thread.shutdown_and_wait().await?;

    let disabled = session_builder(
        /*enabled*/ false,
        ToolMode::CodeModeOnly,
        /*planning*/ true,
        /*changed_catalog*/ false,
    )
    .restart_with_auto_env(&server, &returned)
    .await?;
    disabled
        .submit_turn("Resume with planning enabled and granular updates disabled.")
        .await?;
    disabled.codex.submit(Op::Compact).await?;
    wait_for_successful_turn(&disabled.codex).await;
    disabled
        .submit_turn("Rebuild the complete catalog with granular updates disabled.")
        .await?;
    let enabled = session_builder(
        /*enabled*/ true,
        ToolMode::CodeModeOnly,
        /*planning*/ false,
        /*changed_catalog*/ false,
    )
    .restart_with_auto_env(&server, &disabled)
    .await?;
    enabled
        .submit_turn("Enable granular updates and remove planning.")
        .await?;
    let resumed = session_builder(
        /*enabled*/ true,
        ToolMode::CodeModeOnly,
        /*planning*/ true,
        /*changed_catalog*/ false,
    )
    .restart_with_auto_env(&server, &enabled)
    .await?;
    resumed
        .submit_turn("Resume with planning enabled and granular updates enabled.")
        .await?;

    let requests = mock.requests();
    let declarations = requests
        .iter()
        .map(|request| {
            request
                .inputs_of_type("additional_tools")
                .iter()
                .filter_map(|item| responses::namespace_child_tool(item, "functions", "exec"))
                .cloned()
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        declarations.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![
            1, // Initial catalog.
            1, // Disabling before any delta keeps the existing declaration.
            2, // Enabling initializes a full catalog and its comparison state.
            2, // Added and removed entries use developer messages.
            2, // Disabling keeps those messages without a forced declaration.
            3, // Hybrid mode uses ordinary declarations even with the flag enabled.
            4, // Returning with the flag disabled uses a whole declaration.
            5, // Fork adds planning through a whole declaration.
            5, // Resume adds planning through the same whole declaration.
            5, // Compaction samples the retained history.
            1, // Full replacement uses the current disabled flag.
            2, // Enabling initializes a full catalog before further deltas.
            2, // Resume adds planning in a developer message.
        ]
    );
    for (before, after) in [(0, 1), (3, 4)] {
        assert_eq!(declarations[before], declarations[after]);
        assert!(
            requests[after]
                .input()
                .starts_with(&requests[before].input())
        );
    }
    assert_eq!(
        declarations[0].last(),
        declarations[6].last(),
        "returning from hybrid restores the complete current catalog"
    );
    assert_eq!(
        declarations[7].last(),
        declarations[8].last(),
        "fork and resume publish the same complete catalog while granular updates are disabled"
    );
    let has_update_plan = [6, 8].map(|index| {
        declarations[index].last().expect("full exec declaration")["description"]
            .as_str()
            .expect("exec description")
            .lines()
            .any(|line| line == "### `update_plan`")
    });
    assert_eq!(has_update_plan, [false, true]);
    assert_eq!(
        declarations[10].last(),
        declarations[8].last(),
        "compaction while disabled preserves the complete current catalog"
    );
    assert_eq!(declarations[11].last(), declarations[0].last());
    assert_eq!(declarations[12], declarations[11]);
    let exec_definition_updates = requests
        .iter()
        .map(|request| {
            request
                .message_input_texts("developer")
                .into_iter()
                .filter(|text| {
                    text.starts_with(
                        "Use these definitions and namespace instructions with `functions.exec`.",
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        exec_definition_updates
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>(),
        vec![0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1]
    );
    for index in [3, 12] {
        let update = &exec_definition_updates[index][0];
        assert_eq!(
            update
                .lines()
                .filter(|line| *line != "## Namespace: functions")
                .filter(|line| line.starts_with("## ") || line.starts_with("### "))
                .collect::<Vec<_>>(),
            vec!["### `update_plan`"]
        );
    }
    assert_eq!(
        exec_definition_updates[3], exec_definition_updates[4],
        "disabling granular updates retains the previously published definitions"
    );
    assert_eq!(
        exec_definition_updates[8], exec_definition_updates[9],
        "compaction receives retained definitions without repeating them"
    );
    let removal_notices = requests
        .iter()
        .map(|request| {
            request
                .message_input_texts("developer")
                .into_iter()
                .filter(|text| text.contains("- Tool: tools.exec_command"))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(removal_notices[3].len(), 1);
    assert_eq!(removal_notices[3], removal_notices[4]);
    assert!(removal_notices[10].is_empty());
    assert!(removal_notices[12].is_empty());
    Ok(())
}
