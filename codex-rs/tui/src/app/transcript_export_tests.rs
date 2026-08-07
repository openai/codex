use std::sync::Arc;

use pretty_assertions::assert_eq;

use super::render_markdown_transcript;
use super::visible_export_items;
use super::write_transcript;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::UserHistoryCell;
use crate::history_cell::new_proposed_plan;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnItemsView;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;

#[test]
fn markdown_transcript_preserves_messages_and_formats_activity() {
    assert!(render_markdown_transcript(&[]).is_err());
    let user = |message: &str, local_image_paths| {
        Arc::new(UserHistoryCell {
            message: message.to_string(),
            text_elements: Vec::new(),
            local_image_paths,
            remote_image_urls: Vec::new(),
        }) as Arc<dyn HistoryCell>
    };
    let cells = vec![
        user(
            concat!(
                "# Context from my IDE setup:\n\n## Active file: src/lib.rs\n\n",
                "## My request for Codex:\nExplain \u{1b}[31m**the change**\u{1b}[0m"
            ),
            Vec::new(),
        ),
        Arc::new(AgentMarkdownCell::new(
            "```rust\nlet answer = 42;\n```".to_string(),
            std::path::Path::new("."),
        )),
        Arc::new(new_proposed_plan(
            "completed plan".to_string(),
            std::path::Path::new("."),
        )),
        Arc::new(PlainHistoryCell::new(vec!["$ cargo test".into()])),
        Arc::new(PlainHistoryCell::new(vec![
            "■ Export failed: missing parent".into(),
        ])),
        user("", vec!["image.png".into()]),
        user("[Image #1] describe this", vec!["image.png".into()]),
    ];

    insta::assert_snapshot!(
        "markdown_transcript",
        render_markdown_transcript(&cells).expect("exported transcript")
    );
}

#[test]
fn transcript_export_excludes_hidden_review_prompts_and_nested_duplicates() {
    let user = |id: &str, text: &str| ThreadItem::UserMessage {
        id: id.to_string(),
        client_id: None,
        content: vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
    };
    let entered_review = ThreadItem::EnteredReviewMode {
        id: "review-enter".to_string(),
        review: "review".to_string(),
    };
    let exited_review = ThreadItem::ExitedReviewMode {
        id: "review-exit".to_string(),
        review: "review".to_string(),
    };
    let duplicate_one = user("duplicate-one", "duplicate review prompt");
    let duplicate_two = user("duplicate-two", "duplicate review prompt");
    let turn = |id: &str, items: Vec<ThreadItem>, status| Turn {
        id: id.to_string(),
        items,
        items_view: TurnItemsView::Full,
        status,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    };
    let review = turn(
        "review-turn",
        vec![
            entered_review.clone(),
            user("review-prompt", "hidden review prompt"),
            exited_review.clone(),
        ],
        TurnStatus::Completed,
    );
    let nested = turn(
        "nested-turn",
        vec![duplicate_one.clone(), duplicate_two.clone()],
        TurnStatus::Interrupted,
    );
    assert_eq!(
        visible_export_items(vec![review.clone(), nested]),
        vec![entered_review.clone(), exited_review.clone()],
    );
    let completed = turn(
        "completed-turn",
        vec![duplicate_one.clone(), duplicate_two.clone()],
        TurnStatus::Completed,
    );
    assert_eq!(
        visible_export_items(vec![review, completed]),
        vec![entered_review, exited_review, duplicate_one, duplicate_two],
    );
}

#[test]
fn persisted_transcript_includes_file_and_mcp_details() {
    let cells = [
        serde_json::json!({
            "type": "fileChange", "id": "patch", "status": "completed",
            "changes": [{"path": "src/lib.rs", "kind": {"type": "add"},
                "diff": "+fn answer() -> u8 { 42 }"}]
        }),
        serde_json::json!({
            "type": "mcpToolCall", "id": "tool", "server": "docs", "tool": "search",
            "status": "completed", "arguments": {"query": "export"},
            "result": {"content": [
                {"type": "text", "text": "\u{1b}[31mmatching docs\u{1b}[0m\nnext line"},
                {"type": "image", "data": "c2VjcmV0", "mimeType": "image/png"},
                {"type": "audio", "data": "c2VjcmV0", "mimeType": "audio/wav"},
                {"type": "resource", "resource": {
                    "uri": "file:///report.md", "text": "hidden resource body"}},
                {"type": "resource_link", "uri": "file:///linked.md", "name": "linked"}
            ],
                "structuredContent": {"count": 1}, "_meta": null}
        }),
        serde_json::json!({
            "type": "mcpToolCall", "id": "failed", "server": "docs", "tool": "fetch",
            "status": "failed", "arguments": {"id": 7},
            "error": {"message": "document unavailable\n# injected heading"}
        }),
    ]
    .into_iter()
    .map(|item| serde_json::from_value::<ThreadItem>(item).expect("valid thread item"))
    .filter_map(|item| {
        super::export_activity_cell(&item).map(|cell| Arc::new(cell) as Arc<dyn HistoryCell>)
    })
    .collect::<Vec<_>>();
    let markdown = render_markdown_transcript(&cells).expect("exported transcript");

    for detail in [
        "src/lib.rs",
        "+fn answer() -> u8 { 42 }",
        r#"{"query":"export"}"#,
        "matching docs",
        "next line",
        "<image content>",
        "<audio content>",
        "embedded resource: file:///report.md",
        "link: file:///linked.md",
        r#"structured result: {"count":1}"#,
        "document unavailable",
        "    # injected heading",
    ] {
        assert!(markdown.contains(detail), "missing {detail}: {markdown}");
    }
    assert!(!markdown.contains("c2VjcmV0"));
    assert!(!markdown.contains('\u{1b}'));
}

#[test]
fn transcript_file_resolves_relative_paths_and_refuses_to_overwrite() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = write_transcript(
        directory.path(),
        "conversation.md".as_ref(),
        "# First export\n",
    )
    .expect("write first export");

    assert_eq!(path, directory.path().join("conversation.md"));
    assert!(write_transcript(directory.path(), &path, "# Second export\n").is_err());
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# First export\n"
    );

    let directory_name = directory.path().file_name().expect("directory name");
    let requested = std::path::Path::new("~")
        .join(directory_name)
        .join("conversation.md");
    let error = write_transcript(directory.path(), &requested, "# Export\n")
        .expect_err("missing home-relative parent directory");
    assert!(error.contains(&dirs::home_dir().expect("home").display().to_string()));
}
