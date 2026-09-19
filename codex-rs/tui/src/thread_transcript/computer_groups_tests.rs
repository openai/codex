//! Page boundaries must not change the compact or expanded rendering of computer activity.

use super::*;
use crate::thread_transcript::RawReasoningVisibility;
use crate::thread_transcript::TranscriptCells;
use crate::thread_transcript::thread_items_to_transcript_cells;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStatus;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

fn computer(id: &str) -> ThreadItem {
    serde_json::from_value(serde_json::json!({
        "type": "mcpToolCall", "id": id, "server": "cua_repl", "tool": "js",
        "status": "completed", "arguments": {"title": format!("Inspect {id}"), "code": "await cua.getState()"},
        "appContext": null, "pluginId": null, "readOnlyHint": null,
        "result": {"content": [{"type": "text", "text": format!("Result {id}")}], "structuredContent": null},
        "error": null, "durationMs": 10
    })).unwrap()
}

fn turn(id: &str, items: Vec<ThreadItem>) -> Turn {
    Turn {
        id: id.to_string(),
        items,
        items_view: TurnItemsView::Full,
        status: TurnStatus::Completed,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}

fn project(items: &[ThreadItem]) -> TranscriptCells {
    thread_items_to_transcript_cells(
        /*thread_id*/ None,
        &AbsolutePathBuf::current_dir().unwrap(),
        items.to_vec(),
        RawReasoningVisibility::Hidden,
        /*config*/ None,
    )
}

#[test]
fn every_page_split_retains_the_same_rich_computer_group() {
    let turns = vec![turn(
        "turn",
        vec![
            computer("first"),
            ThreadItem::Reasoning {
                id: "reasoning-first".to_owned(),
                summary: vec!["Inspect the second screen".to_owned()],
                content: Vec::new(),
            },
            computer("second"),
            computer("third"),
            ThreadItem::Reasoning {
                id: "reasoning-second".to_owned(),
                summary: vec!["Confirm the final screen".to_owned()],
                content: Vec::new(),
            },
            computer("last"),
        ],
    )];
    let items = &turns[0].items;
    let expected = project(items);
    for split in 1..items.len() {
        let older = project(&items[..split]);
        let newer = project(&items[split..]);
        let detail_count = newer
            .iter()
            .take_while(|cell| {
                crate::thread_transcript::activity_pages::is_hidden_activity_detail(cell)
            })
            .count();
        let older_group = if detail_count > 0 {
            crate::thread_transcript::activity_pages::fold_trailing_activity_details(
                &older[0],
                &newer[..detail_count],
                &turns,
            )
            .unwrap()
        } else {
            Arc::clone(&older[0])
        };
        let joined = join_computer_groups(&older_group, &newer[detail_count], &turns).unwrap();
        assert_eq!(
            joined.display_hyperlink_lines(/*width*/ 30),
            expected[0].display_hyperlink_lines(/*width*/ 30)
        );
        assert_eq!(
            joined.transcript_hyperlink_lines(/*width*/ 30),
            expected[0].transcript_hyperlink_lines(/*width*/ 30)
        );
        assert_eq!(joined.raw_lines(), expected[0].raw_lines());
    }
}

#[test]
fn successive_single_item_pages_reunite_without_duplicating_calls() {
    let turns = vec![turn(
        "turn",
        (0..6).map(|i| computer(&i.to_string())).collect(),
    )];
    let items = &turns[0].items;
    let mut joined = project(&items[5..]).remove(/*index*/ 0);
    for item in items[..5].iter().rev() {
        let older = project(std::slice::from_ref(item));
        joined = join_computer_groups(&older[0], &joined, &turns).unwrap();
    }
    let expected = project(items);
    assert_eq!(
        joined.transcript_lines(/*width*/ 80),
        expected[0].transcript_lines(/*width*/ 80)
    );
}

#[test]
fn joins_respect_actual_turns_and_intervening_items() {
    let first = computer("first");
    let last = computer("last");
    let older = project(std::slice::from_ref(&first));
    let newer = project(std::slice::from_ref(&last));
    let turns = vec![
        turn("first-turn", vec![first.clone()]),
        turn("last-turn", vec![last.clone()]),
    ];
    assert!(join_computer_groups(&older[0], &newer[0], &turns).is_none());
    let grouped = project(&[
        first.clone(),
        ThreadItem::Sleep(codex_app_server_protocol::SleepItem {
            id: "sleep".into(),
            duration_ms: 10,
        }),
        last.clone(),
    ]);
    let adjacent = project(&[first.clone(), last.clone()]);
    assert_eq!(grouped.len(), 1);
    assert_eq!(
        grouped[0].transcript_lines(/*width*/ 80),
        adjacent[0].transcript_lines(/*width*/ 80)
    );
    let mut turns = vec![turn(
        "turn",
        vec![
            first,
            ThreadItem::Reasoning {
                id: "reasoning".to_string(),
                summary: Vec::new(),
                content: Vec::new(),
            },
            last,
        ],
    )];
    let joined = join_computer_groups(&older[0], &newer[0], &turns)
        .expect("hidden reasoning retains the adjacent computer group");
    let expected = project(&turns[0].items);
    assert_eq!(
        joined.transcript_lines(/*width*/ 80),
        expected[0].transcript_lines(/*width*/ 80),
    );
    turns[0].items[1] = ThreadItem::AgentMessage {
        id: "message".to_string(),
        text: "Visible boundary".to_string(),
        phase: None,
        memory_citation: None,
        delivery: None,
        questions: None,
    };
    assert!(join_computer_groups(&older[0], &newer[0], &turns).is_none());
}
