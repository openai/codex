//! Persisted tool projections retain rich detail, styles, and truthful outcomes.

use super::cells;
use crate::diff_model::FileChange;
use crate::history_cell;
use crate::history_cell::HistoryCell;
use crate::test_support::PathBufExt;
use crate::test_support::test_path_buf;
use codex_app_server_protocol::FileUpdateChange;
use codex_app_server_protocol::ImageGenerationItem;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::PatchChangeKind;
use codex_app_server_protocol::SubAgentActivityKind;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::WebSearchAction;
use codex_app_server_protocol::WebSearchItem;
use codex_utils_path_uri::LegacyAppPathString;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn completed_patch_restores_rich_diff_and_styles() {
    let cwd = test_path_buf("/workspace").abs();
    let mut changes = vec![
        FileUpdateChange {
            path: "new.rs".to_string(),
            kind: PatchChangeKind::Add,
            diff: "fn greet() {\n    println!(\"hello\");\n}\n".to_string(),
        },
        FileUpdateChange {
            path: "old.txt".to_string(),
            kind: PatchChangeKind::Delete,
            diff: "outdated\n".to_string(),
        },
        FileUpdateChange {
            path: "src/before.rs".to_string(),
            kind: PatchChangeKind::Update {
                move_path: Some(PathBuf::from("src/after.rs")),
            },
            diff: "@@ -1 +1 @@\n-let before = 1;\n+let after = 2;\n".to_string(),
        },
    ];
    let expected = history_cell::new_patch_event(
        HashMap::from([
            (
                PathBuf::from("new.rs"),
                FileChange::Add {
                    content: changes[0].diff.clone(),
                },
            ),
            (
                PathBuf::from("old.txt"),
                FileChange::Delete {
                    content: changes[1].diff.clone(),
                },
            ),
            (
                PathBuf::from("src/before.rs"),
                FileChange::Update {
                    unified_diff: changes[2].diff.clone(),
                    move_path: Some(PathBuf::from("src/after.rs")),
                },
            ),
        ]),
        cwd.as_path(),
    );
    changes[2].diff.push_str("\n\nMoved to: src/after.rs");
    let actual = cells(
        ThreadItem::FileChange {
            id: "patch-1".to_string(),
            changes,
            status: PatchApplyStatus::Completed,
        },
        &cwd,
    );

    assert_eq!(actual.len(), 1);
    assert_eq!(
        actual[0].transcript_hyperlink_lines(/*width*/ 80),
        expected.transcript_hyperlink_lines(/*width*/ 80),
    );
}

#[test]
fn unfinished_and_rejected_patches_keep_their_outcome() {
    let cwd = test_path_buf("/workspace").abs();
    let rendered = [
        PatchApplyStatus::InProgress,
        PatchApplyStatus::Declined,
        PatchApplyStatus::Failed,
    ]
    .into_iter()
    .flat_map(|status| {
        cells(
            ThreadItem::FileChange {
                id: "patch-1".to_string(),
                changes: vec![FileUpdateChange {
                    path: "main.rs".to_string(),
                    kind: PatchChangeKind::Add,
                    diff: "fn main() {}\n".to_string(),
                }],
                status,
            },
            &cwd,
        )
    })
    .flat_map(|cell| cell.display_lines(/*width*/ 80))
    .map(|line| line.to_string())
    .collect::<Vec<_>>()
    .join("\n");

    insta::assert_snapshot!(rendered, @"
    • Patch application in progress
    • Patch application declined
    ✘ Failed to apply patch
    ");
}

#[test]
fn tool_and_notice_projection_uses_normal_transcript_presentation() {
    let cwd = test_path_buf("/workspace").abs();
    let image = ImageGenerationItem {
        id: "image-1".to_string(),
        status: "completed".to_string(),
        revised_prompt: Some("A diagram of the history pages".to_string()),
        result: String::new(),
        transparent_background: None,
        failure: None,
        saved_path: None,
        imagegen_request_id: None,
        generation_id: None,
    };
    let items = vec![
        ThreadItem::EnteredReviewMode {
            id: "review-start".to_string(),
            review: "current changes".to_string(),
        },
        ThreadItem::WebSearch(WebSearchItem {
            id: "search-1".to_string(),
            query: "fallback query".to_string(),
            action: Some(WebSearchAction::FindInPage {
                url: Some("https://example.com".to_string()),
                pattern: Some("pagination".to_string()),
            }),
            results: None,
        }),
        ThreadItem::ImageView {
            id: "view-1".to_string(),
            path: LegacyAppPathString::from_string("diagram.png".to_string()),
        },
        ThreadItem::ImageGeneration(image.clone()),
        ThreadItem::ImageGeneration(ImageGenerationItem {
            status: "in_progress".into(),
            ..image
        }),
        ThreadItem::SubAgentActivity {
            id: "agent-1".to_string(),
            kind: SubAgentActivityKind::Completed,
            agent_thread_id: "01912345-1234-7123-8123-123456789abc".to_string(),
            agent_path: "/root/reviewer".to_string(),
        },
        ThreadItem::ExitedReviewMode {
            id: "review-end".to_string(),
            review: "No findings".to_string(),
        },
        ThreadItem::ContextCompaction {
            id: "compact-1".to_string(),
        },
    ];
    let rendered = items
        .into_iter()
        .flat_map(|item| cells(item, &cwd))
        .flat_map(|cell| cell.display_lines(/*width*/ 80))
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(rendered, @"
    >> Code review started: current changes <<
    • Searched for 'pagination' in https://example.com
    • Viewed image diagram.png
    • Generated Image:
      └ A diagram of the history pages
    • Image generation · in_progress
    • Completed `/root/reviewer`
    << Code review finished: No findings >>
    • Context compacted
    ");
}
