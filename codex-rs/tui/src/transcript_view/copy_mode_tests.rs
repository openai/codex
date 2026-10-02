//! Covers transcript copy navigation, rendering, and original-source clipboard payloads.

use super::*;
use crate::clipboard_copy::CopyStatus;
use crate::transcript_view::tests::render;
use crate::transcript_view::tests::text;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use std::path::Path;

fn payload(view: &mut TranscriptView, cells: &[Arc<dyn HistoryCell>]) -> String {
    let selected = view.selected_text(cells).unwrap();
    let mut copied = String::new();
    view.copy_selected_text_with(
        cells,
        &selected,
        /*clear_selection*/ false,
        |text, _| {
            copied = text.to_owned();
            Ok(CopyStatus::Confirmed)
        },
    )
    .unwrap();
    copied
}

#[test]
fn navigate_copy_resize_and_exit() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![
        Arc::new(AgentMarkdownCell::new(
            "Old answer\n\n```\nold code\n```".into(),
            Path::new("/"),
        )),
        Arc::new(crate::history_cell::new_user_prompt(
            "User text\n\n> user quote\n\n```\nuser code\n```".into(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )),
        Arc::new(crate::history_cell::new_proposed_plan(
            "Latest **plan**\n\n> assistant quote\n\n```rust\nassistant code\n```".into(),
            Path::new("/"),
        )),
    ];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 50, /*height*/ 8);
    view.begin_copy_mode(&cells, /*guard*/ None);
    render(&mut view, &cells, /*width*/ 50, /*height*/ 8);
    assert_eq!(
        payload(&mut view, &cells),
        "Latest **plan**\n\n> assistant quote\n\n```rust\nassistant code\n```"
    );
    for expected in [
        "assistant code\n",
        "assistant quote\n",
        "user code\n",
        "> user quote\n",
        "User text\n\n> user quote\n\n```\nuser code\n```",
        "old code\n",
        "old code\n",
    ] {
        view.move_copy_target(&cells, KeyCode::Up);
        assert_eq!(payload(&mut view, &cells), expected);
        render(&mut view, &cells, /*width*/ 50, /*height*/ 8);
    }
    for _ in 0..5 {
        view.move_copy_target(&cells, KeyCode::Down);
    }
    let buffer = render(&mut view, &cells, /*width*/ 50, /*height*/ 8);
    assert!(
        buffer
            .content
            .iter()
            .any(|cell| cell.modifier.contains(ratatui::style::Modifier::REVERSED))
    );
    insta::assert_snapshot!(text(&buffer), @"
    › User text > user quote ``` user code ```

      Latest plan

      > assistant quote

      assistant code
    ");
    insta::assert_snapshot!(
        view.footer_with_navigation(/*width*/ 50, crate::motion::MotionMode::Reduced, "")
            .unwrap()
            .text
            .to_string(), @"
    Copy code block
    ↑/↓/j/k select · g/G ends · enter copy · esc close
    "
    );
    render(&mut view, &cells, /*width*/ 14, /*height*/ 8);
    assert_eq!(payload(&mut view, &cells), "assistant code\n");
    view.move_copy_target(&cells, KeyCode::Down);
    assert_eq!(
        payload(&mut view, &cells),
        "Latest **plan**\n\n> assistant quote\n\n```rust\nassistant code\n```"
    );
    let selected = view.selected_text(&cells).unwrap();
    view.copy_selected_text_with(
        &cells,
        &selected,
        /*clear_selection*/ true,
        |_, _| Ok(CopyStatus::Confirmed),
    )
    .unwrap();
    assert!(view.is_following());
    view.begin_copy_mode(&cells, /*guard*/ None);
    for (key, expected) in [
        ('g', "old code\n"),
        (
            'G',
            "Latest **plan**\n\n> assistant quote\n\n```rust\nassistant code\n```",
        ),
        ('k', "assistant code\n"),
    ] {
        view.handle_key(
            KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE),
            &cells,
        );
        render(&mut view, &cells, /*width*/ 50, /*height*/ 8);
        assert_eq!(payload(&mut view, &cells), expected);
    }
    view.handle_copy_mode_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &cells);
    assert!(view.is_following());
}

#[test]
fn copy_reveals_offscreen_block_with_context() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        format!("{}\n```sh\nlast code\n```", "line\n".repeat(/*n*/ 20)),
        Path::new("/"),
    ))];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 50, /*height*/ 5);
    view.scroll(&cells, /*rows*/ -5);
    render(&mut view, &cells, /*width*/ 50, /*height*/ 5);
    view.begin_copy_mode(&cells, /*guard*/ None);
    render(&mut view, &cells, /*width*/ 50, /*height*/ 5);
    view.move_copy_target(&cells, KeyCode::Up);
    let after = render(&mut view, &cells, /*width*/ 50, /*height*/ 5);
    insta::assert_snapshot!(text(&after), @"
    line
    line

    last code
    ");
}

#[test]
fn user_messages_support_vim_navigation_without_an_assistant_response() {
    let messages = ["First **user** message", "\nPlain prompt with wrapping\n\n"];
    let cells: Vec<Arc<dyn HistoryCell>> = messages
        .iter()
        .map(|message| {
            Arc::new(crate::history_cell::new_user_prompt(
                (*message).into(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )) as Arc<dyn HistoryCell>
        })
        .collect();
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 50, /*height*/ 8);
    view.begin_copy_mode(&cells, /*guard*/ None);
    assert_eq!(payload(&mut view, &cells), messages[1]);
    for (code, modifiers, expected) in [
        (KeyCode::Char('k'), KeyModifiers::NONE, messages[0]),
        (KeyCode::Char('j'), KeyModifiers::NONE, messages[1]),
        (KeyCode::Char('g'), KeyModifiers::NONE, messages[0]),
        (KeyCode::Char('G'), KeyModifiers::NONE, messages[1]),
        (KeyCode::Char('g'), KeyModifiers::NONE, messages[0]),
        (KeyCode::Char('g'), KeyModifiers::SHIFT, messages[1]),
    ] {
        view.handle_key(KeyEvent::new(code, modifiers), &cells);
        render(&mut view, &cells, /*width*/ 18, /*height*/ 8);
        assert_eq!(payload(&mut view, &cells), expected);
    }
    insta::assert_snapshot!(
        view.footer_with_navigation(/*width*/ 60, crate::motion::MotionMode::Reduced, "")
            .unwrap().text.to_string(), @"
    Copy user message
    ↑/↓/j/k select · g/G ends · enter copy · esc close
    "
    );
}

#[test]
fn copy_highlight_fills_a_block_across_blank_and_wrapped_rows() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        "Before\n\n```text\nlonger line\n\nx\n```\n\nAfter".into(),
        Path::new("/"),
    ))];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 16, /*height*/ 12);
    view.begin_copy_mode(&cells, /*guard*/ None);
    view.move_copy_target(&cells, KeyCode::Up);
    let selected = ratatui::buffer::Cell::default()
        .set_style(crate::style::selection_style())
        .style();
    let mut snapshots = Vec::new();
    for width in [16, 8] {
        let buffer = render(&mut view, &cells, width, /*height*/ 12);
        let rows: Vec<String> = buffer
            .content
            .chunks(usize::from(width))
            .map(|row| {
                let mask: String = row
                    .iter()
                    .map(|cell| if cell.style() == selected { '#' } else { '.' })
                    .collect();
                format!(
                    "{mask} | {}",
                    row.iter()
                        .map(ratatui::buffer::Cell::symbol)
                        .collect::<String>()
                )
            })
            .collect();
        snapshots.push(rows.join("\n"));
        assert_eq!(payload(&mut view, &cells), "longer line\n\nx\n");
    }
    insta::assert_snapshot!(snapshots.join("\n---\n"));
}

#[test]
fn whole_user_turn_highlight_includes_bottom_padding() {
    let selected = ratatui::buffer::Cell::default()
        .set_style(crate::style::selection_style())
        .style();
    let mut snapshots = Vec::new();
    for (message, width, height) in [("Short prompt", 20, 4), ("Longer message\n\nlast", 10, 8)] {
        let cells: Vec<Arc<dyn HistoryCell>> =
            vec![Arc::new(crate::history_cell::new_user_prompt(
                message.into(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ))];
        let mut view = TranscriptView::default();
        render(&mut view, &cells, width, height);
        view.begin_copy_mode(&cells, /*guard*/ None);
        let buffer = render(&mut view, &cells, width, height);
        let rows: Vec<String> = buffer
            .content
            .chunks(usize::from(width))
            .map(|row| {
                let mask: String = row
                    .iter()
                    .map(|cell| if cell.style() == selected { '#' } else { '.' })
                    .collect();
                format!(
                    "{mask} | {}",
                    row.iter()
                        .map(ratatui::buffer::Cell::symbol)
                        .collect::<String>()
                )
            })
            .collect();
        snapshots.push(rows.join("\n"));
        assert_eq!(payload(&mut view, &cells), message);
    }
    insta::assert_snapshot!(snapshots.join("\n---\n"));
}

#[test]
fn rendered_mermaid_copies_source() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        "```mermaid\ngraph LR\nA --> B\n```".into(),
        Path::new("/"),
    ))];
    let mut view = TranscriptView::default();
    let buffer = render(&mut view, &cells, /*width*/ 50, /*height*/ 8);
    assert!(!text(&buffer).contains("graph LR"));
    view.begin_copy_mode(&cells, /*guard*/ None);
    view.move_copy_target(&cells, KeyCode::Up);
    assert_eq!(payload(&mut view, &cells), "graph LR\nA --> B\n");
}

#[test]
fn copy_source_survives_raw_rich_rendering_and_whitespace_normalization() {
    let source = "Before\r\n\r\n```sh\r\necho x  \r\n  \r\n```\r\n\r\n> **quote**\r\n";
    let displayed = crate::git_action_directives::parse_assistant_markdown(source, Path::new("/"))
        .visible_markdown;
    for mode in [
        crate::history_cell::HistoryRenderMode::Rich,
        crate::history_cell::HistoryRenderMode::Raw,
    ] {
        let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(
            AgentMarkdownCell::new(displayed.clone(), Path::new("/"))
                .with_copy_source(Some(source.into())),
        )];
        let mut view = TranscriptView::default();
        view.set_presentation(/*detailed*/ false, mode);
        render(&mut view, &cells, /*width*/ 50, /*height*/ 20);
        view.begin_copy_mode(&cells, /*guard*/ None);
        view.move_copy_target(&cells, KeyCode::Up);
        assert_eq!(payload(&mut view, &cells), "**quote**\r\n");
        view.move_copy_target(&cells, KeyCode::Up);
        assert_eq!(payload(&mut view, &cells), "echo x  \r\n  \r\n");
    }
}

#[test]
fn copy_completion_keeps_payload_count_and_follow_intent() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        "**bold**".into(),
        Path::new("/"),
    ))];
    for result in [CopyStatus::Confirmed, CopyStatus::Pending(7)] {
        let mut view = TranscriptView::default();
        render(&mut view, &cells, /*width*/ 40, /*height*/ 5);
        view.begin_copy_mode(&cells, /*guard*/ None);
        let selected = view.selected_text(&cells).unwrap();
        view.copy_selected_text_with(
            &cells,
            &selected,
            /*clear_selection*/ true,
            |_, _| Ok(result),
        )
        .unwrap();
        assert_eq!(view.copy_feedback.as_ref().unwrap().characters, 8);
        if matches!(result, CopyStatus::Pending(_)) {
            assert_eq!(
                view.finish_copy(
                    &cells,
                    &(7, Ok(CopyStatus::Confirmed)),
                    /*current*/ true
                ),
                Some(true)
            );
            view.jump_to_latest();
        }
        assert!(view.is_following());
        let area = ratatui::layout::Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 1,
        );
        let mut buffer = ratatui::buffer::Buffer::empty(area);
        view.render_composer_gap(
            Some(area),
            /*hint*/ None,
            &mut buffer,
            std::time::Instant::now(),
        );
        insta::allow_duplicates! {
            insta::assert_snapshot!(text(&buffer).trim(), @"Copied 8 chars to host clipboard");
        }
        assert_eq!(view.copy_feedback.as_ref().unwrap().characters, 8);
    }
}

#[test]
fn find_and_shift_click_release_copy_payload() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        "**bold** needle".into(),
        Path::new("/"),
    ))];
    for detailed in [false, true] {
        let mut view = TranscriptView::default();
        view.set_presentation(detailed, crate::history_cell::HistoryRenderMode::Rich);
        render(&mut view, &cells, /*width*/ 40, /*height*/ 5);
        view.begin_copy_mode(&cells, /*guard*/ None);
        view.begin_search();
        assert!(view.copy_mode.is_none());
        assert!(view.paste_search("needle"));
        view.advance_search(&cells);
        assert!(
            view.footer_with_navigation(/*width*/ 80, crate::motion::MotionMode::Reduced, "")
                .unwrap()
                .text
                .to_string()
                .contains("needle")
        );
    }
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 40, /*height*/ 5);
    view.begin_copy_mode(&cells, /*guard*/ None);
    render(&mut view, &cells, /*width*/ 40, /*height*/ 5);
    let row = view.area.y
        + view
            .visible
            .iter()
            .position(|row| row.layout.text().contains("needle"))
            .unwrap() as u16;
    view.extend_selection(/*column*/ 6, row);
    assert!(view.copy_mode.is_none());
    assert_eq!(payload(&mut view, &cells), "**bold**");
}

#[test]
fn empty_copy_releases_input_guard() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let guard = Arc::new(crate::copy_input_guard::CopyInputGuard(
        crate::app_event_sender::AppEventSender::new(tx),
    ));
    let weak = Arc::downgrade(&guard);
    let mut view = TranscriptView::default();
    assert!(!view.begin_copy_mode(&[], Some(guard)));
    assert!(weak.upgrade().is_none());
    assert!(matches!(
        rx.try_recv(),
        Ok(crate::app_event::AppEvent::TranscriptCopyClosed)
    ));
}

#[test]
fn copy_input_guard_is_released_on_all_selector_exits() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        "response".into(),
        Path::new("/"),
    ))];
    for exit in ["escape", "find", "mouse", "confirmed", "presentation"] {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let guard = Arc::new(crate::copy_input_guard::CopyInputGuard(
            crate::app_event_sender::AppEventSender::new(tx),
        ));
        let weak = Arc::downgrade(&guard);
        let mut view = TranscriptView::default();
        render(&mut view, &cells, /*width*/ 40, /*height*/ 5);
        view.begin_copy_mode(&cells, Some(guard));
        assert!(weak.upgrade().is_some());
        match exit {
            "escape" => {
                view.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &cells);
            }
            "find" => view.begin_search(),
            "mouse" => {
                view.begin_selection(&cells, /*column*/ 2, /*row*/ 0, /*clicks*/ 1)
            }
            "confirmed" => {
                view.copy_selected_text_with(
                    &cells,
                    "response",
                    /*clear_selection*/ true,
                    |_, _| Ok(CopyStatus::Confirmed),
                )
                .unwrap();
            }
            "presentation" => view.set_presentation(
                /*detailed*/ true,
                crate::history_cell::HistoryRenderMode::Raw,
            ),
            _ => unreachable!(),
        }
        assert!(weak.upgrade().is_none(), "{exit}");
        assert!(matches!(
            rx.try_recv(),
            Ok(crate::app_event::AppEvent::TranscriptCopyClosed)
        ));
    }
}

#[test]
fn user_code_blocks_preserve_original_line_endings() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(crate::history_cell::new_user_prompt(
        "```sh\r\necho user  \r\n```\r\n".into(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    ))];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 40, /*height*/ 10);
    view.begin_copy_mode(&cells, /*guard*/ None);
    assert_eq!(payload(&mut view, &cells), "echo user  \r\n");
}

#[test]
fn latest_partial_response_owns_its_payload() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![
        Arc::new(AgentMarkdownCell::new(
            "Previous completed answer".into(),
            Path::new("/"),
        )),
        Arc::new(AgentMarkdownCell::new(
            "Interrupted **partial** answer".into(),
            Path::new("/"),
        )),
    ];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 60, /*height*/ 12);
    view.begin_copy_mode(&cells, /*guard*/ None);
    assert_eq!(payload(&mut view, &cells), "Interrupted **partial** answer");
}

#[test]
fn source_targets_survive_quotes_lists_and_rendered_tables() {
    for (source, expected) in [
        (
            "> ```sh\n> echo first\n> ```\n\n> later **prose**\n",
            vec![
                "later **prose**\n",
                "echo first\n",
                "```sh\necho first\n```\n",
            ],
        ),
        (
            "- ```mermaid\n  graph LR\n  A --> B\n  ```\n",
            vec!["graph LR\nA --> B\n"],
        ),
        (
            "> first ::git-push{cwd=\"/repo\"}\r\n\r\n> second\r\n",
            vec!["first \r\n", "second\r\n"],
        ),
        (
            "```md\n| A |\n| --- |\n| x |\n```\n\n```md\n| A |\n| --- |\n| x |\n```\n",
            vec!["| A |\n| --- |\n| x |\n", "| A |\n| --- |\n| x |\n"],
        ),
        (
            "```md\n| A | B |\n| --- | --- |\n| x | y |\n```\n",
            vec!["| A | B |\n| --- | --- |\n| x | y |\n"],
        ),
    ] {
        for mode in [
            crate::history_cell::HistoryRenderMode::Rich,
            crate::history_cell::HistoryRenderMode::Raw,
        ] {
            let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
                source.into(),
                Path::new("/"),
            ))];
            let mut view = TranscriptView::default();
            view.set_presentation(/*detailed*/ false, mode);
            render(&mut view, &cells, /*width*/ 60, /*height*/ 20);
            view.begin_copy_mode(&cells, /*guard*/ None);
            let mut actual = Vec::new();
            for _ in &expected {
                view.move_copy_target(&cells, KeyCode::Up);
                actual.push(payload(&mut view, &cells));
            }
            actual.sort();
            let mut expected = expected.clone();
            expected.sort();
            assert_eq!(actual, expected, "{mode:?}: {source}");
        }
    }
}

#[test]
fn rendered_fenced_table_has_a_block_highlight() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        "Before\n\n```md\n| A | B |\n| --- | --- |\n| x | y |\n```\n\nAfter".into(),
        Path::new("/"),
    ))];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 30, /*height*/ 10);
    view.begin_copy_mode(&cells, /*guard*/ None);
    view.move_copy_target(&cells, KeyCode::Up);
    let buffer = render(&mut view, &cells, /*width*/ 30, /*height*/ 10);
    let style = ratatui::buffer::Cell::default()
        .set_style(crate::style::selection_style())
        .style();
    let rows = buffer
        .content
        .chunks(30)
        .map(|row| {
            let mark = if row.iter().all(|cell| cell.style() == style) {
                "selected"
            } else {
                "        "
            };
            format!(
                "{mark} | {}",
                row.iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<String>()
                    .trim_end()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(rows);
}

#[test]
fn copy_navigation_leaves_context_and_preserves_the_center_band() {
    let context = (0..12)
        .map(|i| format!("Context {i}\n\n"))
        .collect::<String>();
    let source = format!(
        "{context}```sh\nfirst target\n```\n\n{context}```sh\nsecond target\n```\n\n{context}"
    );
    let cells: Vec<Arc<dyn HistoryCell>> =
        vec![Arc::new(AgentMarkdownCell::new(source, Path::new("/")))];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 50, /*height*/ 16);
    view.begin_copy_mode(&cells, /*guard*/ None);
    for (key, expected_row) in [(KeyCode::Up, 11), (KeyCode::Up, 4), (KeyCode::Down, 11)] {
        view.move_copy_target(&cells, key);
        let buffer = render(&mut view, &cells, /*width*/ 50, /*height*/ 16);
        let selection_style = ratatui::buffer::Cell::default()
            .set_style(crate::style::selection_style())
            .style();
        let selected_rows = buffer
            .content
            .chunks(50)
            .enumerate()
            .filter_map(|(row, cells)| {
                cells
                    .iter()
                    .any(|cell| cell.style() == selection_style)
                    .then_some(row)
            })
            .collect::<Vec<_>>();
        assert_eq!(selected_rows, vec![expected_row]);
        let before = text(&buffer);
        view.reveal_copy_context(&cells);
        assert_eq!(
            text(&render(
                &mut view, &cells, /*width*/ 50, /*height*/ 16
            )),
            before
        );
    }
    let buffer = render(&mut view, &cells, /*width*/ 50, /*height*/ 10);
    insta::assert_snapshot!(text(&buffer));
}

#[test]
fn copy_resize_keeps_whole_user_padding_inside_context() {
    let user = |message: &str| -> Arc<dyn HistoryCell> {
        Arc::new(crate::history_cell::new_user_prompt(
            message.into(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ))
    };
    let cells: Vec<Arc<dyn HistoryCell>> = vec![
        user("Earlier prompt"),
        Arc::new(AgentMarkdownCell::new(
            "Earlier context\n\n".repeat(12),
            Path::new("/"),
        )),
        user("One\nTwo\nThree\nFour\nFive\nSix"),
        Arc::new(AgentMarkdownCell::new(
            "Later context\n\n".repeat(12),
            Path::new("/"),
        )),
    ];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 50, /*height*/ 20);
    view.begin_copy_mode(&cells, /*guard*/ None);
    let len = view.layout(&cells, /*index*/ 2).unwrap().text().len();
    view.select_copy_range(&cells, /*index*/ 2, 0..len);
    render(&mut view, &cells, /*width*/ 50, /*height*/ 20);
    let buffer = render(&mut view, &cells, /*width*/ 50, /*height*/ 10);
    let selected = ratatui::buffer::Cell::default()
        .set_style(crate::style::selection_style())
        .style();
    let rows = buffer
        .content
        .chunks(50)
        .enumerate()
        .filter_map(|(row, cells)| {
            cells
                .iter()
                .any(|cell| cell.style() == selected)
                .then_some(row)
        })
        .collect::<Vec<_>>();
    assert!(!rows.is_empty());
    assert!(
        rows[0] > 0 && *rows.last().unwrap() < 9,
        "selected rows: {rows:?}"
    );
    insta::assert_snapshot!(text(&buffer));
}

#[test]
fn copy_navigation_keeps_bottom_context_when_prompt_header_appears() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![
        Arc::new(crate::history_cell::new_user_prompt(
            "First prompt".into(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )),
        Arc::new(AgentMarkdownCell::new(
            format!(
                "{}\n```sh\necho target\n```\n\nLater context",
                "Earlier context\n\n".repeat(/*n*/ 12)
            ),
            Path::new("/"),
        )),
    ];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 50, /*height*/ 7);
    view.begin_copy_mode(&cells, /*guard*/ None);
    view.move_copy_target(&cells, KeyCode::Home);
    render(&mut view, &cells, /*width*/ 50, /*height*/ 7);
    view.move_copy_target(&cells, KeyCode::Down);
    let buffer = render(&mut view, &cells, /*width*/ 50, /*height*/ 7);
    let rows = text(&buffer);
    assert_eq!(
        rows.lines().position(|line| line.contains("echo target")),
        Some(5)
    );
    assert_eq!(
        text(&render(
            &mut view, &cells, /*width*/ 50, /*height*/ 7
        )),
        rows
    );
}
