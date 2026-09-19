//! Dynamic tool projections keep truthful status and bounded previews without losing detail.

use super::*;
use crate::test_support::PathBufExt;
use crate::test_support::test_path_buf;
use crate::thread_transcript::RawReasoningVisibility;
use crate::thread_transcript::thread_items_to_transcript_cells;
use pretty_assertions::assert_eq;
use serde_json::json;

fn item(status: DynamicToolCallStatus, output: Option<&str>) -> ThreadItem {
    ThreadItem::DynamicToolCall {
        id: "dynamic-1".to_string(),
        namespace: Some("example".to_string()),
        tool: "inspect".to_string(),
        arguments: json!({"path": "src/main.rs"}),
        status,
        content_items: output.map(|text| {
            vec![DynamicToolCallOutputContentItem::InputText {
                text: text.to_string(),
            }]
        }),
        success: None,
        duration_ms: None,
    }
}

#[test]
fn dynamic_status_and_output_match_persisted_presentations() {
    let cwd = test_path_buf("/workspace").abs();
    let mut snapshots = Vec::new();
    for (label, status, output) in [
        ("pending", DynamicToolCallStatus::InProgress, None),
        (
            "success",
            DynamicToolCallStatus::Completed,
            Some("Found the definition"),
        ),
        (
            "failure",
            DynamicToolCallStatus::Failed,
            Some("Permission denied"),
        ),
        ("unavailable", DynamicToolCallStatus::Completed, None),
    ] {
        let replayed = thread_items_to_transcript_cells(
            /*thread_id*/ None,
            &cwd,
            [item(status, output)],
            RawReasoningVisibility::Hidden,
            /*config*/ None,
        );
        assert_eq!(replayed.len(), 1);
        let cell = &replayed[0];
        for (mode, lines) in [
            ("compact", cell.display_lines(/*width*/ 80)),
            (
                "full",
                visible_lines(cell.transcript_hyperlink_lines(/*width*/ 80)),
            ),
            ("raw", cell.raw_lines()),
        ] {
            let text = lines
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            if mode == "compact" || label == "success" {
                snapshots.push(format!("{label}, {mode}\n{text}"));
            }
        }
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}

#[test]
fn dynamic_preview_reports_hidden_lines_and_retains_full_output() {
    let mut snapshots = Vec::new();
    for count in [3, 4] {
        let output = (1..=count)
            .map(|index| format!("Result line {index}: retained content"))
            .collect::<Vec<_>>()
            .join("\n");
        let cell =
            DynamicToolCallCell::from_item(item(DynamicToolCallStatus::Completed, Some(&output)))
                .unwrap();
        for width in [20, 80] {
            let compact = cell.display_lines(width);
            assert!(
                compact
                    .iter()
                    .all(|line| line.width() <= usize::from(width))
            );
            let text = compact
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            snapshots.push(format!("lines={count}, width={width}\n{text}"));
        }
        let raw = cell
            .raw_lines()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(raw.ends_with(&output));
        let detailed = visible_lines(cell.transcript_hyperlink_lines(/*width*/ 80))
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(output.lines().all(|line| detailed.contains(line)));
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}
