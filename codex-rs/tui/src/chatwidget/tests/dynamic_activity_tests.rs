//! Dynamic calls must finish visibly whether transcript rows are retained or terminal-owned.

use super::*;
use crate::local_settings::LocalSettings;
use crate::transcript_mode::TranscriptMode;
use codex_app_server_protocol::DynamicToolCallOutputContentItem;
use codex_app_server_protocol::DynamicToolCallStatus;
use codex_config::types::AltScreenMode;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

fn dynamic_item(id: &str, status: DynamicToolCallStatus) -> AppServerThreadItem {
    let completed = status != DynamicToolCallStatus::InProgress;
    AppServerThreadItem::DynamicToolCall {
        id: id.to_string(),
        namespace: Some("test".to_string()),
        tool: "lookup".to_string(),
        arguments: json!({ "id": id }),
        status,
        content_items: completed.then(|| {
            vec![DynamicToolCallOutputContentItem::InputText {
                text: format!("Result for {id}"),
            }]
        }),
        success: None,
        duration_ms: completed.then_some(/*t*/ 25),
    }
}

#[tokio::test]
async fn terminal_dynamic_activity_retains_calls_across_both_fallbacks() {
    let mut outputs = Vec::new();
    for (owned_enabled, alternate_screen) in
        [(false, AltScreenMode::Always), (true, AltScreenMode::Never)]
    {
        let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.config.tui_fullscreen_transcript = owned_enabled;
        chat.config.tui_alternate_screen = alternate_screen;
        chat.local_settings = LocalSettings::from(&chat.config);
        assert_eq!(
            chat.local_settings.transcript_mode,
            TranscriptMode::Terminal
        );
        drain_insert_history(&mut rx);

        for id in ["call-1", "call-2"] {
            chat.on_dynamic_tool_item(dynamic_item(id, DynamicToolCallStatus::InProgress));
        }
        let cells = std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => Some(cell),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(cells.len(), 2);
        assert_eq!(chat.transcript.dynamic_calls.len(), 2);

        chat.on_dynamic_tool_item(dynamic_item("call-2", DynamicToolCallStatus::Completed));
        chat.on_dynamic_tool_item(dynamic_item("call-1", DynamicToolCallStatus::Failed));
        assert!(drain_insert_history(&mut rx).is_empty());
        outputs.push(
            cells
                .iter()
                .flat_map(|cell| cell.display_lines(/*width*/ 80))
                .collect::<Vec<_>>(),
        );
        assert!(chat.transcript.dynamic_calls.is_empty());
    }
    assert_eq!(outputs[0], outputs[1]);
    insta::assert_snapshot!(
        lines_to_single_string(&outputs[0]),
        @"
        • Failed test.lookup · 25ms
          └ Result for call-1
        • Called test.lookup · 25ms
          └ Result for call-2
        "
    );
}

#[tokio::test]
async fn owned_dynamic_activity_updates_the_retained_row_after_config_refresh() {
    let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.local_settings.transcript_mode = TranscriptMode::Owned;
    // A thread/config refresh cannot change ownership after the terminal starts.
    chat.config.tui_fullscreen_transcript = false;
    drain_insert_history(&mut rx);
    chat.on_dynamic_tool_item(dynamic_item("call-1", DynamicToolCallStatus::InProgress));
    let retained = std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 1);
    assert_eq!(
        lines_to_single_string(&retained[0].display_lines(/*width*/ 80)),
        "• Calling test.lookup\n",
    );

    chat.on_dynamic_tool_item(dynamic_item("call-1", DynamicToolCallStatus::Completed));
    assert!(drain_insert_history(&mut rx).is_empty());
    assert!(chat.transcript.dynamic_calls.is_empty());
    assert_eq!(
        lines_to_single_string(&retained[0].display_lines(/*width*/ 80)),
        "• Called test.lookup · 25ms\n  └ Result for call-1\n",
    );
}

#[tokio::test]
async fn turn_finalization_drains_lifecycle_queue_before_interrupting_remaining_calls() {
    let (mut chat, mut events, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
    drain_insert_history(&mut events);
    chat.interrupts
        .push_item_started(dynamic_item("completed", DynamicToolCallStatus::InProgress));
    chat.interrupts
        .push_item_completed(dynamic_item("completed", DynamicToolCallStatus::Completed));
    chat.interrupts.push_item_started(dynamic_item(
        "unfinished",
        DynamicToolCallStatus::InProgress,
    ));
    chat.finalize_turn();
    let cells = std::iter::from_fn(|| events.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell)
                if cell.as_any().is::<history_cell::DynamicToolCallCell>() =>
            {
                Some(cell)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(cells.len(), 2);
    assert!(chat.transcript.dynamic_calls.is_empty());
    let raw = cells
        .iter()
        .map(|cell| lines_to_single_string(&cell.raw_lines()))
        .collect::<Vec<_>>();
    assert!(raw[0].contains("Result for completed"));
    assert!(!raw[0].contains("Interrupted"));
    assert!(raw[1].contains("Interrupted before this tool returned a result."));
    let lines = cells[1].display_lines(/*width*/ 64);
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        /*width*/ 64,
        lines.len() as u16,
    );
    let mut buffer = Buffer::empty(area);
    Paragraph::new(lines).render(area, &mut buffer);
    insta::assert_snapshot!("interrupted_dynamic_activity", format!("{buffer:?}"));
}
