//! Older pages must preserve newer events and identify repeated hidden prompts precisely.

use super::*;
use crate::app::test_support::make_test_app;
use crate::history_cell::ComputerActivityCell;
use crate::history_cell::FinalMessageSeparator;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::UserInput;
use pretty_assertions::assert_eq;

fn turn(id: &str, status: TurnStatus, item_ids: &[&str]) -> Turn {
    Turn {
        id: id.to_string(),
        items: item_ids
            .iter()
            .map(|id| ThreadItem::UserMessage {
                id: id.to_string(),
                client_id: None,
                content: Vec::new(),
            })
            .collect(),
        items_view: TurnItemsView::Full,
        status,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    }
}

#[test]
fn overlapping_history_keeps_live_turn_state_and_newer_items() {
    let mut current = vec![turn("shared", TurnStatus::InProgress, &["overlap", "live"])];
    let older = vec![
        turn("old", TurnStatus::Completed, &["first"]),
        turn("shared", TurnStatus::Completed, &["before", "overlap"]),
    ];
    merge_older_turns(&mut current, older);
    assert_eq!(
        current,
        vec![
            turn("old", TurnStatus::Completed, &["first"]),
            turn(
                "shared",
                TurnStatus::InProgress,
                &["before", "overlap", "live"]
            ),
        ],
    );
}

fn user_cell(message: &str) -> Arc<dyn HistoryCell> {
    Arc::new(UserHistoryCell {
        spoken: false,
        message: message.to_string(),
        text_elements: Vec::new(),
        local_image_paths: Vec::new(),
        remote_image_urls: Vec::new(),
    })
}

#[test]
fn repeated_prompt_matching_removes_only_the_hidden_occurrence() {
    let cells = vec![user_cell("repeat"), user_cell("other"), user_cell("repeat")];
    let persisted = vec![
        ("unloaded".to_string(), user_cell("repeat")),
        ("hidden".to_string(), user_cell("repeat")),
        ("middle".to_string(), user_cell("other")),
        ("visible".to_string(), user_cell("repeat")),
    ];
    assert_eq!(
        hidden_transcript_indices(&cells, persisted, &HashSet::from(["hidden", "unloaded"])),
        vec![0],
    );
}

#[tokio::test]
async fn hidden_last_item_keeps_turn_groups_and_completion_boundaries() {
    let computer = |id: &str| {
        serde_json::from_value(serde_json::json!({
            "type": "mcpToolCall", "id": id, "server": "cua_repl", "tool": "js",
            "status": "completed", "arguments": {"title": format!("Inspect {id}")},
            "appContext": null, "pluginId": null, "readOnlyHint": null,
            "result": {"content": [], "structuredContent": null},
            "error": null, "durationMs": 10
        }))
        .expect("valid completed computer call")
    };
    for status in [
        TurnStatus::Completed,
        TurnStatus::Failed,
        TurnStatus::Interrupted,
    ] {
        let mut app = make_test_app().await;
        let cwd = app.config.cwd.clone();
        let thread_id = ThreadId::new();
        let expected_completions = usize::from(status == TurnStatus::Completed);
        let mut first = turn("first", status, &[]);
        first.items = vec![
            ThreadItem::EnteredReviewMode {
                id: "enter-review".to_string(),
                review: "current changes".to_string(),
            },
            computer("older-call"),
            ThreadItem::UserMessage {
                id: "hidden".to_string(),
                client_id: None,
                content: vec![UserInput::Text {
                    text: "internal review prompt".to_string(),
                    text_elements: Vec::new(),
                }],
            },
        ];
        first.completed_at = Some(1_700_000_000);
        first.duration_ms = Some(125_000);
        let mut second = turn("second", TurnStatus::InProgress, &[]);
        second.items = vec![computer("newer-call")];
        let turns = vec![first, second];
        let hidden = hidden_review_item_ids(&turns);
        assert_eq!(hidden, HashSet::from(["hidden"]));
        let items = turns
            .iter()
            .flat_map(|turn| turn.items.clone())
            .collect::<Vec<_>>();

        let cells = app.project_older_history_cells(
            items.clone(),
            &turns,
            &hidden,
            thread_id,
            &cwd,
            RawReasoningVisibility::Hidden,
        );
        let groups = cells
            .iter()
            .filter_map(|cell| cell.as_any().downcast_ref::<ComputerActivityCell>())
            .map(|cell| cell.call_ids().map(str::to_string).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let completions = cells
            .iter()
            .filter(|cell| cell.as_any().is::<FinalMessageSeparator>())
            .count();
        let visible_users = cells
            .iter()
            .filter(|cell| cell.as_any().is::<UserHistoryCell>())
            .count();
        assert_eq!(
            (groups, completions, visible_users),
            (
                vec![
                    vec!["older-call".to_string()],
                    vec!["newer-call".to_string()]
                ],
                expected_completions,
                0,
            ),
        );

        let repeated = app.project_older_history_cells(
            items,
            &turns,
            &hidden,
            thread_id,
            &cwd,
            RawReasoningVisibility::Hidden,
        );
        assert_eq!(
            repeated
                .iter()
                .filter(|cell| cell.as_any().is::<FinalMessageSeparator>())
                .count(),
            0,
        );
    }
}
