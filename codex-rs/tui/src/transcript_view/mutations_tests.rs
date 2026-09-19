//! Page joins preserve the displayed source until reading or selection leaves a retired group.

use super::*;
use crate::history_cell::PlainHistoryCell;
use pretty_assertions::assert_eq;

fn cell(text: &str) -> Arc<dyn HistoryCell> {
    Arc::new(PlainHistoryCell::new(
        text.lines().map(|line| line.to_owned().into()).collect(),
    ))
}

fn render(view: &mut TranscriptView, cells: &[Arc<dyn HistoryCell>], width: u16) -> Buffer {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 2);
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, cells);
    buffer
}

fn live(view: &mut TranscriptView, revision: u64, text: &str) {
    view.sync_live_tail(
        /*width*/ 20,
        Some(ActiveCellTranscriptKey {
            cacheable: true,
            revision,
            is_stream_continuation: false,
            animation_tick: None,
        }),
        |width| Some(cell(text).display_hyperlink_lines(width)),
    );
}

#[test]
fn page_join_holds_the_reading_revision_until_navigation_reaches_a_surviving_cell() {
    let mut cells = vec![cell("one\ntwo\nthree\nfour"), cell("tail\nlast")];
    let mut view = TranscriptView::default();
    view.jump_to_entry(&cells, /*index*/ 0);
    render(&mut view, &cells, /*width*/ 20);
    view.scroll(&cells, /*rows*/ 1);
    let before = render(&mut view, &cells, /*width*/ 20);

    cells.splice(0..0, [cell("ancient"), cell("older group")]);
    view.history_loaded(&cells, 0..2);
    let replacement = cell("merged summary\nchanged preview");
    view.replace_group(&cells, 1..3, &replacement);
    cells.splice(1..3, [replacement]);
    cells.push(cell("new output"));

    assert_eq!(render(&mut view, &cells, /*width*/ 20), before);
    view.scroll(&cells, /*rows*/ 1);
    assert!(view.held_reading.is_some());
    let Position::Reading(anchor) = view.position else {
        panic!("reading the held group");
    };
    assert_eq!(anchor.offset, "one\ntwo\n".len());
    render(&mut view, &cells, /*width*/ 8);
    assert_eq!(view.position, Position::Reading(anchor));

    view.scroll(&cells, /*rows*/ -100);
    assert!(view.snapshot().is_none());
    assert_eq!(
        view.position,
        Position::Reading(Anchor {
            key: EntryKey::cell(&cells[0]),
            index: 0,
            offset: 0,
            row_bias: 0,
        })
    );
    view.jump_to_latest();
    let latest = render(&mut view, &cells, /*width*/ 20);
    let mut fresh = TranscriptView::default();
    assert_eq!(latest, render(&mut fresh, &cells, /*width*/ 20));
}

#[test]
fn page_join_keeps_selection_and_clearing_it_retains_the_reading_position() {
    let mut cells = vec![cell("one\ntwo\nthree\nfour"), cell("tail\nlast")];
    let mut view = TranscriptView::default();
    view.jump_to_entry(&cells, /*index*/ 0);
    render(&mut view, &cells, /*width*/ 20);
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 1, /*clicks*/ 3);
    view.end_drag();
    let before = render(&mut view, &cells, /*width*/ 20);
    let selected = view.selected_text(&cells);
    assert_eq!(selected, Some("two\n".to_owned()));

    cells.splice(0..0, [cell("ancient"), cell("older group")]);
    view.history_loaded(&cells, 0..2);
    let replacement = cell("merged summary\nchanged preview");
    view.replace_group(&cells, 1..3, &replacement);
    cells.splice(1..3, [replacement]);
    assert_eq!(render(&mut view, &cells, /*width*/ 20), before);

    cells.insert(/*index*/ 0, cell("earliest page"));
    view.history_loaded(&cells, 0..1);
    render(&mut view, &cells, /*width*/ 8);
    assert_eq!(view.selected_text(&cells), selected);
    view.end_selection(&cells);
    assert!(view.selection.is_none());
    assert!(view.held_reading.is_some());
    view.scroll(&cells, /*rows*/ 1);
    assert!(view.held_reading.is_some());
    view.scroll(&cells, /*rows*/ -100);
    assert!(view.snapshot().is_none());
}

#[test]
fn page_join_merges_unseen_groups_while_an_unrelated_selection_is_held() {
    let mut cells = vec![cell("newer group"), cell("tail\nlast")];
    let mut view = TranscriptView::default();
    view.jump_to_entry(&cells, /*index*/ 1);
    render(&mut view, &cells, /*width*/ 20);
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 1, /*clicks*/ 3);
    view.end_drag();
    let before = render(&mut view, &cells, /*width*/ 20);
    let selected = view.selected_text(&cells);
    assert_eq!(selected, Some("last".to_owned()));

    cells.insert(/*index*/ 0, cell("older group"));
    view.history_loaded(&cells, 0..1);
    let replacement = cell("merged summary\nchanged preview");
    view.replace_group(&cells, 0..2, &replacement);
    cells.splice(0..2, [replacement]);
    assert_eq!(render(&mut view, &cells, /*width*/ 20), before);
    assert_eq!(view.selected_text(&cells), selected);

    view.scroll(&cells, /*rows*/ -100);
    let buffer = render(&mut view, &cells, /*width*/ 20);
    let lines = buffer
        .content
        .chunks(/*chunk_size*/ 20)
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(lines, @"
    merged summary
    changed preview
    ");
    assert_eq!(view.selected_text(&cells), selected);
}

#[test]
fn page_join_keeps_groups_scrolled_into_view_after_selecting_elsewhere() {
    let mut cells = vec![
        cell("before"),
        cell("older group"),
        cell("newer group"),
        cell("tail\nlast"),
    ];
    let mut view = TranscriptView::default();
    view.jump_to_entry(&cells, /*index*/ 3);
    render(&mut view, &cells, /*width*/ 20);
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 1, /*clicks*/ 3);
    view.end_drag();
    view.scroll(&cells, /*rows*/ -100);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 8,
    );
    let mut before = Buffer::empty(area);
    view.render(area, &mut before, &cells);
    let position = view.position;

    let replacement = cell("merged summary\nchanged preview");
    view.replace_group(&cells, 1..3, &replacement);
    cells.splice(1..3, [replacement]);
    let mut after = Buffer::empty(area);
    view.render(area, &mut after, &cells);

    assert_eq!(after, before);
    assert_eq!(view.position, position);
    assert_eq!(view.selected_text(&cells), Some("last".to_owned()));
}

#[test]
fn scrolling_forward_out_of_a_replaced_group_resolves_the_surviving_cell_index() {
    let mut cells = vec![cell("one\ntwo"), cell("tail\na\nb\nc\nd\ne")];
    let mut view = TranscriptView::default();
    view.jump_to_entry(&cells, /*index*/ 0);
    render(&mut view, &cells, /*width*/ 20);
    cells.insert(/*index*/ 0, cell("older group"));
    view.history_loaded(&cells, 0..1);
    let replacement = cell("merged summary");
    view.replace_group(&cells, 0..2, &replacement);
    cells.splice(0..2, [replacement]);

    view.scroll(&cells, /*rows*/ 3);
    assert!(view.snapshot().is_none());
    assert_eq!(
        view.position,
        Position::Reading(Anchor {
            key: EntryKey::cell(&cells[1]),
            index: 1,
            offset: 0,
            row_bias: 0,
        })
    );
}

#[test]
fn page_join_does_not_hold_latest_or_a_reader_outside_the_replaced_group() {
    let mut cells = vec![cell("before"), cell("group"), cell("tail\nlast")];
    let mut latest = TranscriptView::default();
    let mut reader = TranscriptView::default();
    reader.jump_to_entry(&cells, /*index*/ 0);
    let position = reader.position;
    let replacement = cell("changed group");
    latest.replace_group(&cells, 1..2, &replacement);
    reader.replace_group(&cells, 1..2, &replacement);
    cells.splice(1..2, [replacement]);

    assert!(latest.is_following());
    assert!(latest.snapshot().is_none());
    assert!(reader.snapshot().is_none());
    assert_eq!(reader.position, position);
}

#[test]
fn regrouping_the_tail_for_older_pages_does_not_report_new_activity() {
    let mut cells = vec![cell("one\ntwo\nthree\nfour")];
    let mut view = TranscriptView::default();
    view.jump_to_entry(&cells, /*index*/ 0);
    render(&mut view, &cells, /*width*/ 20);

    for older in ["older group", "earliest group"] {
        cells.insert(/*index*/ 0, cell(older));
        view.history_loaded(&cells, 0..1);
        let replacement = cell("merged summary");
        view.replace_group(&cells, 0..2, &replacement);
        cells.splice(0..2, [replacement]);
        render(&mut view, &cells, /*width*/ 20);
        assert!(!view.unseen_activity);
    }

    cells.push(cell("new output"));
    render(&mut view, &cells, /*width*/ 20);
    assert!(view.unseen_activity);
}

#[test]
fn regrouping_preserves_new_activity_that_has_not_yet_been_drawn() {
    let mut cells = vec![cell("one\ntwo\nthree\nfour")];
    let mut view = TranscriptView::default();
    view.jump_to_entry(&cells, /*index*/ 0);
    render(&mut view, &cells, /*width*/ 20);
    cells.push(cell("new output"));
    let replacement = cell("merged summary");
    view.replace_group(&cells, 0..2, &replacement);
    cells.splice(0..2, [replacement]);

    render(&mut view, &cells, /*width*/ 20);
    assert!(view.unseen_activity);
}

#[test]
fn absorbing_older_history_keeps_live_readers_and_selections_on_their_displayed_revision() {
    for selecting in [false, true] {
        let mut cells = vec![cell("header")];
        let mut view = TranscriptView::default();
        live(
            &mut view,
            /*revision*/ 1,
            "zero\none\ntwo\nthree\nfour\nfive",
        );
        render(&mut view, &cells, /*width*/ 20);
        view.scroll(&cells, /*rows*/ -2);
        render(&mut view, &cells, /*width*/ 20);
        if selecting {
            view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
            view.end_drag();
        }
        let before = render(&mut view, &cells, /*width*/ 20);
        let selected = view.selected_text(&cells);

        cells.extend([cell("older turn"), cell("older group")]);
        view.history_loaded(&cells, 1..3);
        view.absorb_tail_into_live(
            &cells, /*previous_revision*/ 1, /*hydrated_revision*/ 2,
        );
        cells.pop();
        live(
            &mut view,
            /*revision*/ 2,
            "hydrated group\ncurrent preview",
        );

        assert_eq!(render(&mut view, &cells, /*width*/ 20), before);
        assert_eq!(view.selected_text(&cells), selected);
        assert!(!view.unseen_activity);
        assert_eq!(view.snapshot_cells().expect("held revision").len(), 3);
        view.jump_to_latest();
        render(&mut view, &cells, /*width*/ 20);
        assert!(view.snapshot().is_none());
        assert_eq!(
            view.live.as_ref().expect("hydrated tail").text(),
            "hydrated group\ncurrent preview"
        );
    }
}

#[test]
fn absorbing_a_read_canonical_tail_holds_it_until_navigation_leaves() {
    let mut cells = vec![cell("header")];
    let mut view = TranscriptView::default();
    live(&mut view, /*revision*/ 1, "current live tail");
    render(&mut view, &cells, /*width*/ 20);
    cells.push(cell("older one\nolder two\nolder three"));
    view.history_loaded(&cells, 1..2);
    view.jump_to_entry(&cells, /*index*/ 1);
    let before = render(&mut view, &cells, /*width*/ 20);

    view.absorb_tail_into_live(
        &cells, /*previous_revision*/ 1, /*hydrated_revision*/ 2,
    );
    cells.pop();
    live(
        &mut view,
        /*revision*/ 2,
        "hydrated group\ncurrent preview",
    );
    assert_eq!(render(&mut view, &cells, /*width*/ 20), before);
    assert!(!view.unseen_activity);
    view.scroll(&cells, /*rows*/ -100);
    assert!(view.snapshot().is_none());
}

#[test]
fn absorbing_older_history_excludes_only_the_hydration_revision_from_new_activity() {
    for (previous_revision, hydrated_revision, next_revision, expected_unseen) in
        [(1, 2, 2, false), (2, 3, 3, true), (1, 2, 3, true)]
    {
        let mut cells = vec![cell("header")];
        let mut view = TranscriptView::default();
        live(&mut view, /*revision*/ 1, "original live tail");
        view.jump_to_entry(&cells, /*index*/ 0);
        render(&mut view, &cells, /*width*/ 20);
        cells.extend([cell("older turn"), cell("older group")]);
        view.history_loaded(&cells, 1..3);
        view.absorb_tail_into_live(&cells, previous_revision, hydrated_revision);
        cells.pop();
        live(&mut view, next_revision, "updated live tail");
        render(&mut view, &cells, /*width*/ 20);

        assert_eq!(view.unseen_activity, expected_unseen);
        assert_eq!(
            view.live.as_ref().expect("updated tail").text(),
            "updated live tail"
        );
    }
}

#[test]
fn folding_a_reasoning_only_page_retains_its_find_and_copy_revision() {
    use crate::thread_transcript::RawReasoningVisibility;
    use crate::thread_transcript::fold_trailing_activity_details;
    use crate::thread_transcript::thread_items_to_transcript_cells;
    use codex_app_server_protocol::ThreadItem;
    use codex_app_server_protocol::Turn;
    use codex_app_server_protocol::TurnItemsView;
    use codex_app_server_protocol::TurnStatus;

    let call = serde_json::from_value(serde_json::json!({
        "type": "mcpToolCall", "id": "call", "server": "cua_repl", "tool": "js",
        "status": "completed", "arguments": {"title": "Inspect"},
        "result": {"content": [{"type": "text", "text": "retained call output"}]}, "durationMs": 1
    }))
    .expect("computer call");
    let reasoning = ThreadItem::Reasoning {
        id: "reasoning".to_owned(),
        summary: vec!["retained needle  text with exact spacing".to_owned()],
        content: Vec::new(),
    };
    let current = Turn {
        id: "turn".to_owned(),
        items: vec![call, reasoning],
        items_view: TurnItemsView::Full,
        status: TurnStatus::Completed,
        error: None,
        started_at: None,
        completed_at: None,
        duration_ms: None,
    };
    let project = |items: &[ThreadItem]| {
        thread_items_to_transcript_cells(
            /*thread_id*/ None,
            &codex_utils_absolute_path::AbsolutePathBuf::current_dir().unwrap(),
            items.iter().cloned(),
            RawReasoningVisibility::Hidden,
            /*config*/ None,
        )
    };
    for selecting in [false, true] {
        let mut cells = project(&current.items[1..]);
        let mut view = TranscriptView::default();
        view.set_presentation(/*detailed*/ true, HistoryRenderMode::Rich);
        view.jump_to_entry(&cells, /*index*/ 0);
        render(&mut view, &cells, /*width*/ 30);
        if selecting {
            view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 1);
            for _ in 0..8 {
                view.selection_key(&cells, crossterm::event::KeyCode::Right);
            }
        } else {
            view.begin_search();
            view.paste_search("needle");
            for _ in 0..8 {
                if !view.advance_search(&cells) {
                    break;
                }
            }
            assert!(
                view.search
                    .status_line(/*width*/ 80, view.history)
                    .to_string()
                    .starts_with("enter next")
            );
        }
        let before = render(&mut view, &cells, /*width*/ 30);
        let copied = view.selected_text(&cells);
        if selecting {
            assert!(copied.as_ref().is_some_and(|text| !text.is_empty()));
        }
        cells.insert(
            /*index*/ 0,
            project(&current.items[..1]).remove(/*index*/ 0),
        );
        view.history_loaded(&cells, 0..1);
        let joined =
            fold_trailing_activity_details(&cells[0], &cells[1..], std::slice::from_ref(&current))
                .unwrap();
        view.replace_group(&cells, 0..2, &joined);
        cells.splice(0..2, [joined]);
        assert_eq!(render(&mut view, &cells, /*width*/ 30), before);
        render(&mut view, &cells, /*width*/ 16);
        assert_eq!(view.selected_text(&cells), copied);
        if !selecting {
            assert!(
                view.search
                    .status_line(/*width*/ 80, view.history)
                    .to_string()
                    .starts_with("enter next")
            );
        }
    }
}
