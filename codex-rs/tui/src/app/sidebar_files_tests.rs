#![cfg(test)]

use super::*;
use crate::app::owned_transcript::tests::buffer_text;
use crate::markdown_render::SourceLocation;
use crate::sidebar_chooser;
use crate::sidebar_chooser::NewChatState;
use codex_app_server_protocol::FileUpdateChange;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::buffer::Cell;

#[test]
fn sidebar_files_chooser_matches_the_grouped_file_inventory() -> color_eyre::eyre::Result<()> {
    let workspace = tempfile::tempdir()?;
    std::fs::create_dir(workspace.path().join("folder.with-dot"))?;
    std::fs::write(workspace.path().join("Makefile"), "")?;
    let cwd = PathUri::from_abs_path(&codex_utils_absolute_path::AbsolutePathBuf::try_from(
        workspace.path(),
    )?);
    let mut entries = Vec::new();
    item_entries(
        Some(&cwd),
        &ThreadItemPayload::FileChange {
            id: "patch".into(),
            changes: [
                ("src/lib.rs", PatchChangeKind::Update { move_path: None }),
                ("./notes/guide.md", PatchChangeKind::Add),
                ("removed.md", PatchChangeKind::Delete),
            ]
            .into_iter()
            .map(|(path, kind)| FileUpdateChange {
                path: path.into(),
                kind,
                diff: String::new(),
            })
            .collect(),
            status: PatchApplyStatus::Completed,
        },
        &mut entries,
    );
    item_entries(
        Some(&cwd),
        &ThreadItemPayload::Plan {
            id: "plan".into(),
            text: "[Guide](./notes/guide.md#L4) [Source](./src/lib.rs#L12C4) \
                [Image](./figure.png) [Folder](./folder.with-dot) [Build](./Makefile) \
                [Reference](https://example.net) https://example.org"
                .into(),
        },
        &mut entries,
    );
    let mut files = Files::new(
        Some(cwd),
        /*next*/ None,
        /*inspect_local_paths*/ true,
    );
    files.promote(entries.into_iter().rev());
    let mut pane = super::super::sidebar::SidebarPane::default();
    pane.files = Some(files);
    pane.agents.rows.push(sidebar_chooser::AgentRow {
        id: ThreadId::new(),
        name: "Reviewer".into(),
        symbol: "○",
        status: "Completed",
        selectable: true,
    });
    pane.choice = pane.chat_tabs().file_start();
    pane.update_saved_chats(vec![ThreadId::new().into()]);
    assert_eq!(pane.chat_tabs().file_offset(pane.choice), Some(0));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 50, /*height*/ 30,
    );
    let mut buffer = Buffer::empty(area);
    sidebar_chooser::paint(
        /*focused*/ true,
        area,
        &mut buffer,
        Line::default(),
        pane.chat_tabs(),
        pane.choice,
        &sidebar_chooser::SidebarStyle {
            new_chat: NewChatState::Available,
            use_theme_colors: false,
            motion: crate::motion::MotionMode::Reduced,
            frames: &tui::FrameRequester::test_dummy(),
            hovered_choice: None,
            full_path: None,
            path_scroll: 0,
        },
    );
    let screen = buffer
        .content
        .chunks(usize::from(area.width))
        .map(|cells| cells.iter().map(Cell::symbol).collect::<String>())
        .map(|line| line.trim().to_owned())
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(screen.trim(), @"
    Open in sidebar

    + New side chat
    Start from this conversation

    Side chats  1

    ○ Side chat

    Subagents  1

    ○ Reviewer
    Completed

    Files  4

    › · Makefile
    Project root
    · figure.png
    Project root
    · lib.rs
    src
    · guide.md
    notes
    ");
    let long_path =
        "/Users/example/workspaces/really-long-project-name/codex-rs/tui/src/app/guide.md";
    pane.files
        .as_mut()
        .expect("file inventory")
        .extend([FileLink {
            path: long_path.into(),
            location: None,
        }]);
    let wide = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 75, /*height*/ 24,
    );
    let mut wide_buffer = Buffer::empty(wide);
    let frames = tui::FrameRequester::test_dummy();
    let style = sidebar_chooser::SidebarStyle {
        new_chat: NewChatState::Available,
        use_theme_colors: false,
        motion: crate::motion::MotionMode::Reduced,
        frames: &frames,
        hovered_choice: Some(pane.chat_tabs().choice_count() - 1),
        full_path: None,
        path_scroll: 0,
    };
    sidebar_chooser::paint(
        /*focused*/ true,
        wide,
        &mut wide_buffer,
        Line::default(),
        pane.chat_tabs(),
        pane.choice,
        &style,
    );
    let file_rows = wide_buffer
        .content
        .chunks(usize::from(wide.width))
        .map(|cells| cells.iter().map(Cell::symbol).collect::<String>())
        .filter(|line| line.contains("guide.md") || line.contains("Makefile"))
        .map(|line| line.trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(file_rows, @"
     › · Makefile  Project root
       · guide.md  notes
       · guide.md  /Users/example/workspaces/re…ct-name/codex-rs/tui/src/app
    ");
    let right_padding = usize::from(sidebar_chooser::body_row(wide, wide.y).right() - 1);
    let [selected, hovered]: [_; 2] = wide_buffer
        .content
        .chunks(usize::from(wide.width))
        .filter(|cells| {
            let text = cells.iter().map(Cell::symbol).collect::<String>();
            text.contains("Makefile") || text.contains("/Users/example")
        })
        .map(|row| {
            assert_eq!(
                (row[right_padding].symbol(), row[right_padding].bg),
                (" ", row[2].bg)
            );
            (row[2].bg, row[2].modifier)
        })
        .collect::<Vec<_>>()
        .try_into()
        .expect("selected and hovered files");
    assert_ne!(selected, hovered);

    sidebar_chooser::paint(
        /*focused*/ true,
        wide,
        &mut wide_buffer,
        Line::default(),
        pane.chat_tabs(),
        pane.choice,
        &sidebar_chooser::SidebarStyle {
            full_path: Some(long_path),
            ..style
        },
    );
    let path_screen = wide_buffer
        .content
        .chunks(usize::from(wide.width))
        .map(|row| {
            row.iter()
                .map(Cell::symbol)
                .collect::<String>()
                .trim()
                .to_owned()
        })
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(path_screen[0], "File path");
    assert_eq!(path_screen[1..].join(""), long_path);
    let edge_path = "/tmp/2026-11-22-something-with-many-parts-and-text.md";
    let edge = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 52, /*height*/ 2,
    );
    let mut edge_buffer = Buffer::empty(edge);
    sidebar_chooser::paint_file_path(edge, edge_path, /*top*/ 0, &mut edge_buffer);
    assert_eq!(
        buffer_text(&edge_buffer).lines().collect::<String>(),
        edge_path
    );

    let narrow_path = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 18, /*height*/ 7,
    );
    let mut narrow_buffer = Buffer::empty(narrow_path);
    let bottom = sidebar_chooser::scroll_file_path(
        sidebar_chooser::file_path_areas(narrow_path)[0],
        long_path,
        /*top*/ 0,
        crate::keymap::ListAction::JumpBottom,
    );
    assert!(bottom > 0);
    sidebar_chooser::paint(
        /*focused*/ true,
        narrow_path,
        &mut narrow_buffer,
        Line::default(),
        pane.chat_tabs(),
        pane.choice,
        &sidebar_chooser::SidebarStyle {
            full_path: Some(long_path),
            path_scroll: bottom,
            ..style
        },
    );
    let rendered = narrow_buffer
        .content
        .chunks(usize::from(narrow_path.width))
        .map(|line| line.iter().map(Cell::symbol).collect::<String>())
        .map(|line| line.trim_end().to_owned())
        .collect::<String>();
    assert!(rendered.ends_with("guide.md"), "{rendered}");

    let tabs = pane.chat_tabs();
    let targets: Vec<_> =
        sidebar_chooser::row_targets(area, tabs, pane.choice, NewChatState::Available).collect();
    for row in (10..=12).chain(15..=17) {
        assert!(
            targets
                .iter()
                .all(|(_, area)| !area.contains((1, row).into()))
        );
    }
    assert_eq!(
        targets
            .iter()
            .find(|(_, area)| area.contains((1, 13).into()))
            .and_then(|(index, _)| tabs.agent(*index))
            .map(|agent| agent.name.as_str()),
        Some("Reviewer"),
    );
    assert_eq!(
        targets
            .iter()
            .find(|(_, area)| area.contains((1, 18).into()))
            .and_then(|(index, _)| tabs.file(*index)),
        tabs.file(pane.choice),
    );
    Ok(())
}

#[test]
fn sidebar_files_follow_latest_mentions_and_preserve_selection() {
    let mut pane = super::super::sidebar::SidebarPane::default();
    pane.files = Some(Files::new(
        /*cwd*/ None, /*next*/ None, /*inspect_local_paths*/ false,
    ));
    let link = |path: &str| FileLink {
        path: path.into(),
        location: None,
    };
    promote_files(&mut pane, [link("dog.png"), link("guide.md")]);
    pane.choice = pane.chat_tabs().file_start() + 1;
    promote_files(&mut pane, [link("guide.md"), link("new.rs")]);
    pane.files
        .as_mut()
        .unwrap()
        .extend([link("dog.png"), link("old.md")]);
    assert_eq!(
        pane.files.as_ref().unwrap().entries,
        vec![
            link("guide.md"),
            link("new.rs"),
            link("dog.png"),
            link("old.md")
        ]
    );
    assert_eq!(pane.chat_tabs().file(pane.choice), Some(&link("guide.md")));
}

#[test]
fn sidebar_files_scrolls_the_body_without_wrapping_or_moving_the_wheel_selection() {
    use crate::keymap::ListAction;

    let mut pane = super::super::sidebar::SidebarPane::default();
    let mut files = Files::new(
        /*cwd*/ None, /*next*/ None, /*inspect_local_paths*/ false,
    );
    files.extend((1..=8).map(|number| FileLink {
        path: format!("file{number}.rs"),
        location: None,
    }));
    pane.files = Some(files);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 72, /*height*/ 8,
    );
    let tabs = pane.chat_tabs();
    let (last, bottom) =
        sidebar_chooser::navigate(area, tabs, /*choice*/ 0, ListAction::JumpBottom);
    pane.chooser_scroll = bottom;
    let tabs = pane.chat_tabs();
    assert_eq!(
        sidebar_chooser::navigate(area, tabs, last, ListAction::MoveDown),
        (last, bottom)
    );
    let (above, same_view) = sidebar_chooser::navigate(area, tabs, last, ListAction::MoveUp);
    assert_eq!((above, same_view), (last - 1, bottom));
    assert_eq!(
        sidebar_chooser::choice_at(area, tabs, above, /*y*/ 1),
        Some(4)
    );
    let (previous, scrolled_up) =
        sidebar_chooser::navigate(area, tabs, /*choice*/ 4, ListAction::MoveUp);
    pane.chooser_scroll = scrolled_up;
    assert_eq!(
        sidebar_chooser::choice_at(area, pane.chat_tabs(), previous, /*y*/ 1),
        Some(previous),
    );

    pane.chooser_scroll = Default::default();
    let tabs = pane.chat_tabs();
    assert_eq!(
        sidebar_chooser::navigate(area, tabs, /*choice*/ 0, ListAction::MoveUp),
        (0, Default::default())
    );
    pane.chooser_scroll =
        sidebar_chooser::wheel_scroll(area, tabs, /*choice*/ 0, /*delta*/ 3);
    pane.chooser_scroll =
        sidebar_chooser::wheel_scroll(area, pane.chat_tabs(), /*choice*/ 0, /*delta*/ 3);
    let tabs = pane.chat_tabs();
    assert!(!sidebar_chooser::choice_is_visible(
        area, tabs, /*choice*/ 0
    ));
    assert_eq!(
        sidebar_chooser::choice_at(area, tabs, /*choice*/ 0, /*y*/ 1),
        None
    );
    assert_eq!(
        sidebar_chooser::choice_at(area, tabs, /*choice*/ 0, /*y*/ 2),
        Some(1)
    );
    let (first, keyboard_scroll) =
        sidebar_chooser::navigate(area, tabs, /*choice*/ 0, ListAction::MoveDown);
    assert_eq!(first, 1);
    assert_eq!(
        sidebar_chooser::choice_at(
            area,
            sidebar_chooser::ChatTabs {
                scroll: keyboard_scroll,
                ..tabs
            },
            first,
            /*y*/ 2
        ),
        Some(first),
    );
    let mut buffer = Buffer::empty(area);
    sidebar_chooser::paint(
        /*focused*/ true,
        area,
        &mut buffer,
        Line::from("enter open"),
        tabs,
        /*choice*/ 0,
        &sidebar_chooser::SidebarStyle {
            new_chat: NewChatState::Available,
            use_theme_colors: false,
            motion: crate::motion::MotionMode::Reduced,
            frames: &tui::FrameRequester::test_dummy(),
            hovered_choice: None,
            full_path: None,
            path_scroll: 0,
        },
    );
    let screen = buffer
        .content
        .chunks(usize::from(area.width))
        .map(|cells| cells.iter().map(Cell::symbol).collect::<String>())
        .map(|line| line.trim().to_owned())
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(screen.trim(), @"
    ↑
    · file1.rs  Project root
    · file2.rs  Project root
    · file3.rs  Project root
    · file4.rs  Project root
    ↓
    enter open
    ");
    pane.chooser_scroll =
        sidebar_chooser::wheel_scroll(area, tabs, /*choice*/ 0, /*delta*/ 100);
    let tabs = pane.chat_tabs();
    assert_eq!(
        sidebar_chooser::wheel_scroll(area, tabs, /*choice*/ 0, /*delta*/ 3),
        pane.chooser_scroll
    );
    assert_eq!(
        sidebar_chooser::choice_at(area, tabs, /*choice*/ 0, /*y*/ 5),
        Some(last)
    );
}

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
            location: Some(SourceLocation::single(/*line*/ 12, Some(4))),
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
