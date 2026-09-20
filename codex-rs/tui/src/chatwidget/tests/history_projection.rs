//! Persisted tool cells must match initial replay in compact and detailed presentations.

use super::*;
use crate::thread_transcript::RawReasoningVisibility;
use crate::thread_transcript::thread_items_to_transcript_cells;
use codex_app_server_protocol::McpToolCallResult;
use codex_app_server_protocol::McpToolCallStatus;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn older_tool_projection_matches_initial_replay() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    drain_insert_history(&mut rx);
    let command = "cargo check";
    let mut items = vec![AppServerThreadItem::CommandExecution {
        id: "command".to_string(),
        plugin_id: None,
        script_path: None,
        model_context: None,
        command: command.to_string(),
        cwd: chat.config.cwd.clone().into(),
        process_id: None,
        source: ExecCommandSource::Agent,
        status: AppServerCommandExecutionStatus::Completed,
        command_actions: vec![AppServerCommandAction::Unknown {
            command: command.to_string(),
        }],
        aggregated_output: Some(
            (1..=12)
                .map(|line| format!("Checking crate {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        exit_code: Some(0),
        duration_ms: Some(25),
    }];
    // Completion-only replay must group consecutive exploration just like older-page projection.
    for name in ["one.rs", "two.rs"] {
        let command = format!("cat {name}");
        items.push(AppServerThreadItem::CommandExecution {
            id: name.to_string(),
            plugin_id: None,
            script_path: None,
            model_context: None,
            command: command.clone(),
            cwd: chat.config.cwd.clone().into(),
            process_id: None,
            source: ExecCommandSource::Agent,
            status: AppServerCommandExecutionStatus::Completed,
            command_actions: vec![AppServerCommandAction::Read {
                command,
                name: name.to_string(),
                path: chat.config.cwd.join(name).into(),
            }],
            aggregated_output: Some("fn main() {}".to_string()),
            exit_code: Some(0),
            duration_ms: Some(5),
        });
    }
    // Four adjacent CUA calls compact together; a regular MCP call is a group boundary.
    for (server, id) in [
        ("cua_repl", "1"),
        ("cua_repl", "2"),
        ("cua_repl", "3"),
        ("cua_repl", "4"),
        ("example", "mcp"),
        ("cua_repl", "5"),
    ] {
        if id == "2" {
            items.push(AppServerThreadItem::Reasoning {
                id: "computer-reasoning".to_string(),
                summary: vec!["**Inspecting the page**".to_string()],
                content: Vec::new(),
            });
        }
        items.push(AppServerThreadItem::McpToolCall {
            id: id.to_string(),
            server: server.to_string(),
            tool: "js".to_string(),
            status: McpToolCallStatus::Completed,
            arguments: json!({"title": format!("Inspect page {id}"), "code": "await cua.getState()"}),
            app_context: None,
            mcp_app_resource_uri: None,
            plugin_id: None,
            read_only_hint: None,
            mcp_app_ui: None,
            result: Some(Box::new(McpToolCallResult {
                content: vec![json!({"type": "text", "text": format!("Full result for {id}")})],
                structured_content: None,
                meta: None,
            })),
            error: None,
            duration_ms: Some(5),
        });
    }
    // Switching back to exploration must flush the computer group, while reasoning stays
    // inside the active exploration group until the completed turn commits it.
    for (index, id) in [(1, "trailing-read-one"), (2, "trailing-read-two")] {
        if index == 2 {
            items.push(AppServerThreadItem::Reasoning {
                id: "exploration-reasoning".to_string(),
                summary: vec!["**Checking the implementation**".to_string()],
                content: Vec::new(),
            });
        }
        let mut read = items[index].clone();
        if let AppServerThreadItem::CommandExecution { id: read_id, .. } = &mut read {
            *read_id = id.to_string();
        }
        items.push(read);
    }
    let projected = thread_items_to_transcript_cells(
        chat.thread_id,
        &chat.config.cwd,
        items.clone(),
        RawReasoningVisibility::Hidden,
        Some(&chat.config),
    );
    chat.replay_thread_turns(
        vec![AppServerTurn {
            items,
            ..app_server_turn(
                "turn",
                AppServerTurnStatus::Completed,
                /*duration_ms*/ None,
                /*error*/ None,
            )
        }],
        ReplayKind::ResumeInitialMessages,
    );
    // Even without an answer or saved completion label, the completed turn must commit its
    // last tool group so an older page can join the same canonical history cells.
    assert!(chat.transcript.active_cell.is_none());
    let replayed = std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell)
                if !cell.as_any().is::<history_cell::FinalMessageSeparator>() =>
            {
                Some(cell)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for width in [40, 100] {
        let render =
            |cell: &dyn HistoryCell| (cell.display_lines(width), cell.transcript_lines(width));
        assert_eq!(
            projected
                .iter()
                .map(|cell| render(cell.as_ref()))
                .collect::<Vec<_>>(),
            replayed
                .iter()
                .map(|cell| render(cell.as_ref()))
                .collect::<Vec<_>>()
        );
    }
    let exploration = replayed
        .iter()
        .find(|cell| {
            cell.as_any()
                .downcast_ref::<ExecCell>()
                .is_some_and(|cell| cell.iter_calls().count() == 2)
        })
        .expect("adjacent completed reads share one exploration group");
    insta::assert_snapshot!(
        "completion_only_replay_exploration_group",
        format!(
            "compact:\n{}\ndetailed:\n{}",
            lines_to_single_string(&exploration.display_lines(/*width*/ 40)),
            lines_to_single_string(&exploration.transcript_lines(/*width*/ 40)),
        )
    );
}

#[tokio::test]
async fn snapshot_formatter_reasoning_matches_compact_and_detailed_replay() {
    let mut snapshots = Vec::new();
    for visibility in [
        RawReasoningVisibility::Hidden,
        RawReasoningVisibility::Visible,
    ] {
        let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
        drain_insert_history(&mut rx);
        chat.config.show_raw_agent_reasoning = visibility == RawReasoningVisibility::Visible;
        let item = AppServerThreadItem::Reasoning {
            id: "reasoning".to_string(),
            summary: vec![
                "**Plan**\n\nRead [src/main.rs](src/main.rs:3).".to_string(),
                "**Checking tests**\n\n<!-- -->".to_string(),
            ],
            content: vec!["Raw detail.".to_string()],
        };
        let projected = thread_items_to_transcript_cells(
            chat.thread_id,
            &chat.config.cwd,
            [item.clone()],
            visibility,
            Some(&chat.config),
        );
        chat.replay_thread_item(item.clone(), "turn".to_string(), ReplayKind::ThreadSnapshot);
        let replayed = take_history_cells(&mut rx);
        assert_eq!((projected.len(), replayed.len()), (1, 1));
        assert!(
            projected[0]
                .display_hyperlink_lines(/*width*/ 80)
                .is_empty()
        );
        assert!(replayed[0].display_hyperlink_lines(/*width*/ 80).is_empty());
        for width in [28, 80] {
            assert_eq!(
                projected[0].transcript_hyperlink_lines(width),
                replayed[0].transcript_hyperlink_lines(width)
            );
        }
        snapshots.push(format!(
            "{visibility:?}\ncompact: {:?}\ndetailed:\n{}",
            projected[0].display_lines(/*width*/ 80),
            lines_to_single_string(&projected[0].transcript_lines(/*width*/ 80)),
        ));
        chat.replay_thread_turns(
            vec![AppServerTurn {
                items: vec![item],
                ..app_server_turn(
                    "turn",
                    AppServerTurnStatus::Completed,
                    /*duration_ms*/ None,
                    /*error*/ None,
                )
            }],
            ReplayKind::ResumeInitialMessages,
        );
        let replayed = take_history_cells(&mut rx);
        let reasoning = replayed
            .iter()
            .find_map(|cell| {
                cell.as_any()
                    .downcast_ref::<history_cell::ReasoningSummaryCell>()
            })
            .expect("initial replay retains reasoning for older-page joins");
        assert_eq!(reasoning.source_item_id(), Some("reasoning"));
    }
    insta::assert_snapshot!(snapshots.join("\n"));
}

#[test]
fn raw_reasoning_keeps_its_own_heading() {
    let projected = thread_items_to_transcript_cells(
        /*thread_id*/ None,
        &codex_utils_absolute_path::AbsolutePathBuf::current_dir().unwrap(),
        [AppServerThreadItem::Reasoning {
            id: "raw".into(),
            summary: Vec::new(),
            content: vec!["**Raw investigation**\nKeep this heading and its details.".into()],
        }],
        RawReasoningVisibility::Visible,
        /*config*/ None,
    );
    insta::assert_snapshot!(lines_to_single_string(&projected[0].transcript_lines(/*width*/ 80)), @"
    • Raw investigation
      Keep this heading and its details.
    ");
}

#[tokio::test]
async fn snapshot_formatter_completed_patch_needs_no_started_notification() {
    for replay_kind in [
        ReplayKind::ResumeInitialMessages,
        ReplayKind::ThreadSnapshot,
    ] {
        let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
        drain_insert_history(&mut rx);
        let item = patch_item(AppServerPatchApplyStatus::Completed);
        let projected = thread_items_to_transcript_cells(
            chat.thread_id,
            &chat.config.cwd,
            [item.clone()],
            RawReasoningVisibility::Hidden,
            Some(&chat.config),
        );
        chat.replay_thread_item(item, "turn".to_string(), replay_kind);
        let replayed = take_history_cells(&mut rx);
        assert_eq!((projected.len(), replayed.len()), (1, 1));
        for width in [28, 80] {
            assert_eq!(
                (
                    replayed[0].display_hyperlink_lines(width),
                    replayed[0].transcript_hyperlink_lines(width),
                ),
                (
                    projected[0].display_hyperlink_lines(width),
                    projected[0].transcript_hyperlink_lines(width),
                )
            );
        }
        insta::assert_snapshot!(
            "snapshot_formatter_completed_patch",
            lines_to_single_string(&replayed[0].display_lines(/*width*/ 80))
        );
    }
}

#[tokio::test]
async fn snapshot_formatter_live_and_buffered_patch_notifications_render_once() {
    for replay_kind in [
        None,
        Some(ReplayKind::ThreadSnapshot),
        Some(ReplayKind::ResumeInitialMessages),
    ] {
        let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
        drain_insert_history(&mut rx);
        chat.handle_server_notification(
            ServerNotification::ItemStarted(ItemStartedNotification {
                thread_id: "thread".to_string(),
                turn_id: "turn".to_string(),
                item: patch_item(AppServerPatchApplyStatus::InProgress),
                started_at_ms: 0,
            }),
            replay_kind,
        );
        chat.handle_server_notification(
            ServerNotification::ItemCompleted(ItemCompletedNotification {
                thread_id: "thread".to_string(),
                turn_id: "turn".to_string(),
                item: patch_item(AppServerPatchApplyStatus::Completed),
                completed_at_ms: 0,
            }),
            replay_kind,
        );
        let replayed = take_history_cells(&mut rx);
        assert_eq!(replayed.len(), 1);
        insta::assert_snapshot!(
            "snapshot_formatter_completed_patch",
            lines_to_single_string(&replayed[0].display_lines(/*width*/ 80))
        );
    }
}

#[tokio::test]
async fn snapshot_formatter_empty_success_and_failed_patch_keep_their_outcomes() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    drain_insert_history(&mut rx);
    chat.replay_thread_item(
        AppServerThreadItem::FileChange {
            id: "empty".to_string(),
            changes: Vec::new(),
            status: AppServerPatchApplyStatus::Completed,
        },
        "turn".to_string(),
        ReplayKind::ThreadSnapshot,
    );
    chat.replay_thread_item(
        patch_item(AppServerPatchApplyStatus::Failed),
        "turn".to_string(),
        ReplayKind::ThreadSnapshot,
    );
    let replayed = take_history_cells(&mut rx);
    assert_eq!(replayed.len(), 1);
    insta::assert_snapshot!(lines_to_single_string(&replayed[0].display_lines(/*width*/ 80)), @"
    ✘ Failed to apply patch
    ");
}

fn patch_item(status: AppServerPatchApplyStatus) -> AppServerThreadItem {
    AppServerThreadItem::FileChange {
        id: "patch".to_string(),
        changes: vec![FileUpdateChange {
            path: "sample.txt".to_string(),
            kind: PatchChangeKind::Update { move_path: None },
            diff: "@@ -1 +1 @@\n-old sample\n+new sample\n".to_string(),
        }],
        status,
    }
}

fn take_history_cells(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> Vec<Box<dyn HistoryCell>> {
    std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell),
            _ => None,
        })
        .collect()
}
