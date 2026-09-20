//! Live completion, replay, and picker projection share retained tool details.

use super::*;
use crate::thread_transcript::RawReasoningVisibility;
use crate::thread_transcript::thread_items_to_transcript_cells;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn completed_tool_details_match_live_replay_and_picker_projection() {
    let output = (1..=12)
        .map(|line| format!("result line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let command: AppServerThreadItem = serde_json::from_value(json!({
        "type": "commandExecution", "id": "command", "command": "cargo check",
        "cwd": test_path_buf("/workspace"), "source": "agent",
        "status": "completed", "commandActions": [{"type": "unknown", "command": "cargo check"}],
        "aggregatedOutput": output, "exitCode": 0, "durationMs": 5
    }))
    .expect("completed command");
    let mcp: AppServerThreadItem = serde_json::from_value(json!({
        "type": "mcpToolCall", "id": "mcp", "server": "example", "tool": "read",
        "status": "completed", "arguments": {"path": "source.rs"},
        "result": {"content": [{"type": "text", "text": output}]}, "durationMs": 5
    }))
    .expect("completed MCP result");
    for item in [command, mcp] {
        let mut projections = Vec::new();
        for replay in [false, true] {
            let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
            drain_insert_history(&mut rx);
            chat.handle_paste("Unsent draft".to_owned());
            if replay {
                chat.replay_thread_item(
                    item.clone(),
                    "turn".to_owned(),
                    ReplayKind::ThreadSnapshot,
                );
            } else {
                match item.clone() {
                    item @ AppServerThreadItem::CommandExecution { .. } => {
                        chat.handle_command_execution_completed_now(item)
                    }
                    item @ AppServerThreadItem::McpToolCall { .. } => {
                        chat.handle_mcp_tool_call_completed_now(item)
                    }
                    _ => unreachable!("tool fixture"),
                }
            }
            chat.flush_active_cell();
            let mut retained = Vec::new();
            while let Ok(event) = rx.try_recv() {
                if let AppEvent::InsertHistoryCell(cell) = event {
                    retained.push((
                        cell.transcript_hyperlink_lines(/*width*/ 32),
                        cell.raw_lines(),
                    ));
                }
            }
            assert_eq!(chat.composer_text_with_pending(), "Unsent draft");
            projections.push(retained);
        }
        let picker = thread_items_to_transcript_cells(
            /*thread_id*/ None,
            &test_path_buf("/workspace").abs(),
            [item],
            RawReasoningVisibility::Hidden,
            /*config*/ None,
        );
        projections.push(
            picker
                .iter()
                .map(|cell| {
                    (
                        cell.transcript_hyperlink_lines(/*width*/ 32),
                        cell.raw_lines(),
                    )
                })
                .collect(),
        );
        assert_eq!(projections[0].len(), 1);
        for projection in &projections[1..] {
            assert_eq!(&projections[0], projection);
        }
        let raw = projections[0][0]
            .1
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(raw.contains(&output));
    }
}
