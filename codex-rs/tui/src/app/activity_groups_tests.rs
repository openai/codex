//! Page transport cannot change activity chronology, rich details, or pending live completion.

use super::*;
use crate::app::test_support::make_test_app;
use crate::thread_transcript::RawReasoningVisibility;
use crate::thread_transcript::thread_items_to_transcript_cells;
use codex_app_server_protocol::TurnItemsView;
use pretty_assertions::assert_eq;

#[derive(Clone, Copy, Debug)]
enum ActivityKind {
    Computer,
    Exploration,
}

fn call(id: &str, kind: ActivityKind, status: &str) -> ThreadItem {
    let value = match kind {
        ActivityKind::Computer => serde_json::json!({
            "type": "mcpToolCall", "id": id, "server": "cua_repl", "tool": "js",
            "status": status, "arguments": {"title": format!("Inspect {id}")},
            "result": {"content": [{"type": "text", "text": format!("Result {id}")}]},
            "durationMs": 10
        }),
        ActivityKind::Exploration => serde_json::json!({
            "type": "commandExecution", "id": id, "command": format!("cat {id}.rs"),
            "cwd": crate::test_support::test_path_buf("/workspace"), "source": "agent",
            "status": status, "commandActions": [{"type": "read", "command": format!("cat {id}.rs"), "name": format!("{id}.rs"), "path": format!("{id}.rs")}],
            "aggregatedOutput": format!("Result {id}"), "exitCode": 0, "durationMs": 10
        }),
    };
    serde_json::from_value(value).expect("activity fixture")
}

fn reasoning(id: &str) -> ThreadItem {
    ThreadItem::Reasoning {
        id: id.to_owned(),
        summary: vec![format!(
            "**Considering {id}**\n\nRetained reasoning for {id}."
        )],
        content: Vec::new(),
    }
}

fn turn(items: Vec<ThreadItem>) -> Turn {
    Turn {
        id: "turn".to_owned(),
        items,
        items_view: TurnItemsView::Full,
        status: TurnStatus::Completed,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}

fn project(app: &App, items: &[ThreadItem]) -> Vec<Arc<dyn HistoryCell>> {
    thread_items_to_transcript_cells(
        /*thread_id*/ None,
        &app.config.cwd,
        items.iter().cloned(),
        RawReasoningVisibility::Hidden,
        Some(&app.config),
    )
}

#[tokio::test]
async fn older_page_hydration_keeps_pending_computer_and_exploration_completion() {
    for kind in [ActivityKind::Computer, ActivityKind::Exploration] {
        let mut app = make_test_app().await;
        match kind {
            ActivityKind::Computer => {
                app.chat_widget.handle_mcp_tool_call_completed_now(call(
                    "newer",
                    kind,
                    "completed",
                ));
                app.chat_widget.handle_mcp_tool_call_started_now(call(
                    "pending",
                    kind,
                    "inProgress",
                ));
            }
            ActivityKind::Exploration => {
                app.chat_widget.handle_command_execution_completed_now(call(
                    "newer",
                    kind,
                    "completed",
                ));
                app.chat_widget.handle_command_execution_started_now(call(
                    "pending",
                    kind,
                    "inProgress",
                ));
            }
        }
        let mut current = turn(vec![
            call("older", kind, "completed"),
            reasoning("boundary"),
            call("newer", kind, "completed"),
            call("pending", kind, "inProgress"),
        ]);
        current.status = TurnStatus::InProgress;
        // Separate projections model reasoning arriving on its own intervening page.
        app.transcript_cells = project(&app, &current.items[..1]);
        app.transcript_cells
            .extend(project(&app, &current.items[1..2]));
        app.join_older_activity_group(/*boundary*/ 1, std::slice::from_ref(&current));
        assert!(app.transcript_cells.is_empty());
        match kind {
            ActivityKind::Computer => app.chat_widget.handle_mcp_tool_call_completed_now(call(
                "pending",
                kind,
                "completed",
            )),
            ActivityKind::Exploration => app
                .chat_widget
                .handle_command_execution_completed_now(call("pending", kind, "completed")),
        }
        current.items[3] = call("pending", kind, "completed");
        let expected = project(&app, &current.items);
        assert_eq!(
            app.chat_widget
                .active_cell_transcript_lines(/*width*/ 80)
                .unwrap(),
            expected[0].transcript_lines(/*width*/ 80),
            "{kind:?}"
        );
    }
}

#[tokio::test]
async fn trailing_reasoning_identity_rejects_identical_text_from_another_turn() {
    for kind in [ActivityKind::Computer, ActivityKind::Exploration] {
        let mut app = make_test_app().await;
        let mut same_turn = reasoning("same-turn-detail");
        let mut next_turn = reasoning("next-turn-detail");
        for item in [&mut same_turn, &mut next_turn] {
            if let ThreadItem::Reasoning { summary, .. } = item {
                *summary = vec!["Exactly the same reasoning text".to_owned()];
            }
        }
        let first = turn(vec![call("call", kind, "completed"), same_turn.clone()]);
        let mut second = turn(vec![next_turn.clone()]);
        second.id = "next-turn".to_owned();
        let turns = vec![first, second];
        for detail in [same_turn, next_turn] {
            let older = project(&app, &turns[0].items[..1]).remove(/*index*/ 0);
            let trailing = project(&app, std::slice::from_ref(&detail)).remove(/*index*/ 0);
            app.transcript_cells = vec![Arc::clone(&older), Arc::clone(&trailing)];
            app.join_older_activity_group(/*boundary*/ 1, &turns);
            if detail.id() == "same-turn-detail" {
                let expected = project(&app, &turns[0].items);
                assert_eq!(app.transcript_cells.len(), 1);
                assert_eq!(app.transcript_cells[0].raw_lines(), expected[0].raw_lines());
            } else {
                assert_eq!(app.transcript_cells.len(), 2);
                assert!(Arc::ptr_eq(&app.transcript_cells[0], &older));
                assert!(Arc::ptr_eq(&app.transcript_cells[1], &trailing));
            }
        }
        // Live reasoning without persisted identity cannot prove an older-page relationship.
        let unidentified: Arc<dyn HistoryCell> = Arc::new(history_cell::ReasoningSummaryCell::new(
            String::new(),
            "Exactly the same reasoning text".to_owned(),
            app.config.cwd.as_path(),
            /*transcript_only*/ true,
        ));
        app.transcript_cells = project(&app, &turns[0].items[..1]);
        app.transcript_cells.push(Arc::clone(&unidentified));
        app.join_older_activity_group(/*boundary*/ 1, &turns);
        assert_eq!(app.transcript_cells.len(), 2);
        assert!(Arc::ptr_eq(&app.transcript_cells[1], &unidentified));
    }
}

#[tokio::test]
async fn every_page_split_folds_reasoning_before_answer_and_completion_boundaries() {
    for kind in [ActivityKind::Computer, ActivityKind::Exploration] {
        for answer in [None, Some("The answer")] {
            let mut app = make_test_app().await;
            let mut items = vec![call("call", kind, "completed"), reasoning("detail")];
            if let Some(text) = answer {
                items.push(ThreadItem::AgentMessage {
                    id: "answer".to_owned(),
                    text: text.to_owned(),
                    phase: None,
                    memory_citation: None,
                    delivery: None,
                    questions: None,
                });
            }
            let current = turn(items);
            let separator: Arc<dyn HistoryCell> =
                Arc::new(history_cell::FinalMessageSeparator::new(
                    /*elapsed_seconds*/ Some(65),
                    /*runtime_metrics*/ None,
                ));
            let mut expected = project(&app, &current.items);
            expected.push(Arc::clone(&separator));
            for split in 1..current.items.len() {
                let older = project(&app, &current.items[..split]);
                let mut newer = project(&app, &current.items[split..]);
                newer.push(Arc::clone(&separator));
                let boundary = older.len();
                app.transcript_cells = older.into_iter().chain(newer).collect();
                app.join_older_activity_group(boundary, std::slice::from_ref(&current));
                for width in [24, 80] {
                    let rendered = |cells: &[Arc<dyn HistoryCell>]| {
                        cells
                            .iter()
                            .map(|cell| {
                                (
                                    cell.display_hyperlink_lines(width),
                                    cell.transcript_hyperlink_lines(width),
                                    cell.raw_lines(),
                                )
                            })
                            .collect::<Vec<_>>()
                    };
                    assert_eq!(
                        rendered(&app.transcript_cells),
                        rendered(&expected),
                        "{kind:?}, answer {answer:?}, split {split}"
                    );
                }
                assert!(Arc::ptr_eq(
                    app.transcript_cells.last().unwrap(),
                    &separator
                ));
            }
        }
    }
}

#[tokio::test]
async fn failed_exploration_retains_following_reasoning_at_every_page_split() {
    let mut app = make_test_app().await;
    let mut failed = call("failed", ActivityKind::Exploration, "failed");
    if let ThreadItem::CommandExecution { exit_code, .. } = &mut failed {
        *exit_code = Some(1);
    }
    let current = turn(vec![
        call("first", ActivityKind::Exploration, "completed"),
        failed,
        reasoning("after-failure"),
    ]);
    let expected = project(&app, &current.items);
    assert_eq!(expected.len(), 1);
    for split in 1..current.items.len() {
        let older = project(&app, &current.items[..split]);
        let boundary = older.len();
        app.transcript_cells = older
            .into_iter()
            .chain(project(&app, &current.items[split..]))
            .collect();
        app.join_older_activity_group(boundary, std::slice::from_ref(&current));
        let rendered = |cells: &[Arc<dyn HistoryCell>]| {
            cells
                .iter()
                .map(|cell| cell.transcript_hyperlink_lines(/*width*/ 80))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            rendered(&app.transcript_cells),
            rendered(&expected),
            "split {split}"
        );
    }
}
