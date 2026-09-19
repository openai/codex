//! Persisted reasoning matches live replay in compact and detailed presentations.

use super::*;
use crate::thread_transcript::RawReasoningVisibility;
use crate::thread_transcript::thread_items_to_transcript_cells;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn snapshot_formatter_reasoning_matches_legacy_and_detailed_replay() {
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
        chat.replay_thread_item(item, "turn".to_string(), ReplayKind::ThreadSnapshot);
        let replayed: Vec<Box<dyn HistoryCell>> = std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|event| match event {
                AppEvent::InsertHistoryCell(cell) => Some(cell),
                _ => None,
            })
            .collect();
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
            "{visibility:?}\nlegacy: {:?}\ndetailed:\n{}",
            projected[0].display_lines(/*width*/ 80),
            lines_to_single_string(&projected[0].transcript_lines(/*width*/ 80)),
        ));
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
