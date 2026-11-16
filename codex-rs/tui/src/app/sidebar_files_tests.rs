#![cfg(test)]

use super::*;
use crate::markdown_render::SourceLocation;
use pretty_assertions::assert_eq;

#[test]
fn files_from_missed_pages_precede_older_cached_paths_and_reader_opens() {
    let link = |path: &str| FileLink {
        path: path.into(),
        location: None,
    };
    let mut files = Files::new(
        /*cwd*/ None, /*next*/ None, /*inspect_local_paths*/ false,
    );
    files.promote([link("new.md"), link("old.md")]);
    files.extend([link("opened.md")]);
    files.connection_replaced();
    files.promote([link("new.md")]);
    files.extend_page(vec![link("missing.md")]);
    files.promote([link("live.md")]);
    files.extend_page(vec![link("old.md"), link("oldest.md")]);
    assert_eq!(
        files.entries,
        [
            "live.md",
            "new.md",
            "missing.md",
            "old.md",
            "oldest.md",
            "opened.md"
        ]
        .map(link)
    );
}

#[test]
fn sidebar_files_keeps_remote_paths_for_the_owning_reader() {
    let cwd = LegacyAppPathString::from_string("/remote/workspace")
        .to_inferred_path_uri()
        .expect("remote working directory");
    let mut entries = Vec::new();
    item_entries(
        Some(&cwd),
        &ThreadItemPayload::Plan {
            id: "plan".into(),
            text: "[Source](./src/lib.rs#L12C4)".into(),
        },
        &mut entries,
    );
    let mut files = Files::new(
        Some(cwd),
        /*next*/ None,
        /*inspect_local_paths*/ false,
    );
    files.promote(entries);
    let path = LegacyAppPathString::from_string("/remote/workspace/src/lib.rs")
        .to_inferred_path_uri()
        .expect("remote document path")
        .inferred_native_path_string();
    assert!(!files.extend([FileLink {
        path: path.clone(),
        location: None,
    }]));
    assert_eq!(
        files.entries,
        vec![FileLink {
            path,
            location: Some(SourceLocation {
                line: 12,
                column: 4
            }),
        }],
    );
}

#[test]
fn sidebar_files_skip_local_directories_and_keep_extensionless_files()
-> color_eyre::eyre::Result<()> {
    let workspace = tempfile::tempdir()?;
    std::fs::create_dir(workspace.path().join("folder.with-dot"))?;
    std::fs::write(workspace.path().join("Makefile"), "")?;
    let cwd = PathUri::from_abs_path(&codex_utils_absolute_path::AbsolutePathBuf::try_from(
        workspace.path(),
    )?);
    let mut entries = Vec::new();
    item_entries(
        Some(&cwd),
        &ThreadItemPayload::Plan {
            id: "plan".into(),
            text: "[Folder](./folder.with-dot) [Build](./Makefile)".into(),
        },
        &mut entries,
    );
    let mut files = Files::new(
        Some(cwd),
        /*next*/ None,
        /*inspect_local_paths*/ true,
    );
    files.extend(entries);
    assert_eq!(
        files.entries,
        vec![FileLink {
            path: workspace
                .path()
                .join("Makefile")
                .to_string_lossy()
                .into_owned(),
            location: None,
        }]
    );
    Ok(())
}

#[test]
fn sidebar_files_respect_page_boundaries_for_typed_voice_and_review_items() {
    use codex_app_server_protocol::FileUpdateChange;
    use codex_app_server_protocol::ThreadItem;
    use codex_app_server_protocol::ThreadItemEntry;
    use codex_app_server_protocol::ThreadItemsListResponse;
    use codex_protocol::models::MessagePhase;

    let entry = |payload| ThreadItemEntry {
        turn_id: "turn".into(),
        item: ThreadItem::untimed(payload),
        output_deferred: false,
        started_at_ms: None,
        completed_at_ms: None,
    };
    let user = |id: &str, text: &str| {
        entry(ThreadItemPayload::UserMessage {
            id: id.into(),
            client_id: None,
            content: vec![UserInput::Text {
                text: text.into(),
                text_elements: Vec::new(),
            }],
        })
    };
    let status = |id: &str| {
        entry(ThreadItemPayload::AgentMessage {
            id: id.into(),
            text: format!("[{id}](./{id}.md)"),
            phase: Some(MessagePhase::Commentary),
            memory_citation: None,
            moderation: None,
            delivery: None,
            questions: None,
        })
    };
    let mut files = Files::new(
        /*cwd*/ None,
        Some("first".into()),
        /*inspect_local_paths*/ false,
    );
    assert!(!files.apply_page(ThreadItemsListResponse {
        data: vec![
            entry(ThreadItemPayload::ExitedReviewMode {
                id: "exit".into(),
                review: String::new()
            }),
            user("review", "[Hidden review](./review.md)"),
            status("voice"),
        ],
        next_cursor: Some("older".into()),
        backwards_cursor: None,
    }));
    assert!(files.apply_page(ThreadItemsListResponse {
        data: vec![
            user(
                "handoff",
                "<realtime_delegation><input>Spoken follow-up</input></realtime_delegation>"
            ),
            entry(ThreadItemPayload::EnteredReviewMode {
                id: "enter".into(),
                review: String::new()
            }),
            status("typed"),
            entry(ThreadItemPayload::FileChange {
                id: "edit".into(),
                changes: vec![FileUpdateChange {
                    path: "src/lib.rs".into(),
                    kind: PatchChangeKind::Update { move_path: None },
                    diff: String::new(),
                }],
                status: PatchApplyStatus::Completed,
            }),
        ],
        next_cursor: None,
        backwards_cursor: None,
    }));
    assert_eq!(
        files
            .entries
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        ["./typed.md", "src/lib.rs"],
    );
}
