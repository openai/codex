//! Native output preserves ordering, quota warning visibility, and replay consistency.

use super::*;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallStatus;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn quota_warnings_are_emitted_once_and_survive_native_replay() -> Result<()> {
    for initial_replay in [false, true] {
        let (mut app, mut events, _ops) = crate::app::tests::make_test_app_with_channels().await;
        let mut tui = crate::tui::test_support::make_test_tui()?;
        if initial_replay {
            app.begin_initial_history_replay_buffer();
        }
        app.insert_history_cell(
            &mut tui,
            Box::new(history_cell::new_warning_event("Hidden diagnostic".into())),
        );
        let quota: codex_app_server_protocol::RateLimitSnapshot =
            serde_json::from_value(serde_json::json!({
                "primary": {"usedPercent": 80, "windowDurationMins": 300},
            }))?;
        for _ in 0..2 {
            app.chat_widget.on_rate_limit_snapshot(Some(quota.clone()));
            while let Ok(event) = events.try_recv() {
                if let AppEvent::InsertHistoryCell(cell) = event {
                    app.insert_history_cell(&mut tui, cell);
                }
            }
        }
        app.finish_initial_history_replay_buffer(&mut tui);
        let inserted = tui.pending_history_lines_for_test();
        insta::allow_duplicates! {
            insta::assert_snapshot!("native_quota_warning", inserted.iter().map(|line| line.line.to_string()).collect::<Vec<_>>().join("\n"));
        }
        assert_eq!(
            app.render_transcript_lines_for_reflow(/*width*/ 80).lines,
            inserted
        );
        app.flush_native_history(&mut tui);
        assert_eq!(tui.pending_history_lines_for_test(), inserted);
        assert_eq!(history_cell::warning_count(&app.transcript_cells), 2);
    }
    Ok(())
}

#[tokio::test]
async fn settled_tool_precedes_queued_output_and_is_not_emitted_twice() -> Result<()> {
    let mut app = crate::app::test_support::make_test_app().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let item = |status| ThreadItem::DynamicToolCall {
        id: "lookup".into(),
        namespace: None,
        tool: "lookup".into(),
        arguments: serde_json::json!({}),
        status,
        success: None,
        duration_ms: None,
        content_items: Some(vec![DynamicToolCallOutputContentItem::InputText {
            text: "unique result".into(),
        }]),
    };
    let cell =
        history_cell::DynamicToolCallCell::from_item(item(DynamicToolCallStatus::InProgress))
            .unwrap();
    // Stale in-progress history is not in the live queue and cannot hide later context.
    app.transcript_cells.push(Arc::new(cell.clone()));
    app.transcript_cells
        .push(Arc::new(history_cell::PlainHistoryCell::new(vec![
            "past context".into(),
        ])));
    app.insert_history_cell(&mut tui, Box::new(cell.clone()));
    app.flush_native_history(&mut tui);
    assert!(tui.pending_history_lines_for_test().is_empty());
    // A long-lived tool must not lose output queued behind it.
    for index in 0..4097 {
        app.insert_history_cell(
            &mut tui,
            Box::new(history_cell::PlainHistoryCell::new(vec![
                format!("later output {index}").into(),
            ])),
        );
    }
    app.flush_native_history(&mut tui);
    assert!(tui.pending_history_lines_for_test().is_empty());
    app.native_history.replayed();
    assert!(
        app.render_transcript_lines_for_reflow(/*width*/ 80)
            .lines
            .iter()
            .any(|line| line.line.to_string().contains("past context"))
    );
    let historical = app.render_transcript_lines_for_reflow(/*width*/ 80).lines;
    assert!(
        historical
            .iter()
            .any(|line| line.line.to_string().contains("Calling lookup"))
    );
    cell.update_from_item(item(DynamicToolCallStatus::Completed));
    for _ in 0..=4097 / CELLS_PER_FRAME {
        app.flush_native_history(&mut tui);
    }
    let lines = tui.pending_history_lines_for_test();
    let text = lines
        .iter()
        .map(|line| line.line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(text.matches("unique result").count(), 1);
    assert_eq!(text.matches("later output").count(), 4097);
    assert!(text.find("unique result").unwrap() < text.find("later output").unwrap());
    app.flush_native_history(&mut tui);
    assert_eq!(tui.pending_history_lines_for_test(), lines);
    // Rollback replay consumes even a queue larger than one frame's drain budget.
    for cell in app.transcript_cells.iter().rev().take(CELLS_PER_FRAME + 1) {
        app.native_history.defer(cell);
    }
    app.rebuild_transcript_after_backtrack(
        &mut tui,
        ratatui::layout::Size::new(/*width*/ 80, /*height*/ 24).into(),
    )?;
    let replayed = tui.pending_history_lines_for_test();
    app.flush_native_history(&mut tui);
    assert_eq!(tui.pending_history_lines_for_test(), replayed);
    Ok(())
}
