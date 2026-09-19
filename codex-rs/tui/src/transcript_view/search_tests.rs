//! Literal search, history continuation, and presentation restoration share source offsets.

use super::*;
use pretty_assertions::assert_eq;
use ratatui::style::Modifier;

#[derive(Debug)]
struct TestCell(String);

impl HistoryCell for TestCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        vec![self.0.clone().into()]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.display_lines(/*width*/ 80)
    }
}

fn cell(text: impl Into<String>) -> Arc<dyn HistoryCell> {
    Arc::new(TestCell(text.into()))
}

fn finish_scan(view: &mut TranscriptView, cells: &[Arc<dyn HistoryCell>]) {
    for _ in 0..512 {
        if !view.advance_search(cells) {
            return;
        }
    }
    panic!("search did not yield after bounded scanning frames");
}

#[test]
fn selection_pauses_find_and_normalized_navigation_resumes_the_same_query() {
    let cells = vec![cell("older needle"), cell("newer needle")];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 32, /*height*/ 4,
    );
    let mut view = TranscriptView {
        area,
        ..TranscriptView::default()
    };
    view.begin_search();
    view.paste_search("needle");
    finish_scan(&mut view, &cells);
    view.render(area, &mut Buffer::empty(area), &cells);
    assert_eq!(view.search.current.as_ref().unwrap().anchor.index, 1);
    // A pending previous-match scan must not move the viewport beneath a copy gesture.
    view.handle_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        &cells,
    );
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
    let selected = view.selected_text(&cells);
    assert!(!view.advance_search(&cells));
    assert_eq!(view.selected_text(&cells), selected);
    assert_eq!(view.search.current.as_ref().unwrap().anchor.index, 1);
    let mut selected_frame = Buffer::empty(area);
    view.render(area, &mut selected_frame, &cells);
    view.handle_key(KeyCode::Esc.into(), &cells);
    finish_scan(&mut view, &cells);
    assert_eq!(view.search.current.as_ref().unwrap().anchor.index, 0);
    view.handle_key(
        KeyEvent::new(KeyCode::Char('\u{e}'), KeyModifiers::NONE),
        &cells,
    );
    finish_scan(&mut view, &cells);
    assert_eq!(view.search.current.as_ref().unwrap().anchor.index, 1);
    assert_eq!(view.search.editor.text(), "needle");
    view.handle_key(
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        &cells,
    );
    assert!(!view.is_search_active());
    insta::assert_snapshot!(
        "selection_owns_find_highlight",
        format!("{selected_frame:?}")
    );
}

#[test]
fn literal_search_preserves_unicode_source_offsets() {
    let text = "界 CAFÉ [x].* İ";
    assert_eq!(
        [
            find_literal(text, "café", Direction::Newer),
            find_literal(text, "[x].*", Direction::Older),
            find_literal(text, "i\u{307}", Direction::Newer),
            find_literal(text, "missing", Direction::Older),
        ],
        [Some(4..9), Some(10..15), Some(16..18), None],
    );
}

#[test]
fn scanning_yields_and_finds_a_match_crossing_a_chunk_boundary() {
    let text = format!(
        "{}nEeDlE{}",
        "a".repeat(SCAN_BYTES - 2),
        "z".repeat(SCAN_BYTES * 2)
    );
    let cells = vec![cell(text)];
    let mut view = TranscriptView {
        area: Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
        ),
        ..TranscriptView::default()
    };
    view.begin_search();
    view.paste_search("needle");
    assert!(view.advance_search(&cells));
    assert!(view.search.current.is_none());
    view.prepare_width(/*width*/ 40);
    finish_scan(&mut view, &cells);
    let found = view
        .search
        .current
        .as_ref()
        .expect("match across chunk boundary");
    assert_eq!(
        (found.anchor.offset, found.end),
        (SCAN_BYTES - 2, SCAN_BYTES + 4)
    );
}

#[test]
fn short_history_entries_share_a_frame_without_starving_input() {
    let mut cells = vec![cell("needle in the oldest message")];
    cells.extend((0..64).map(|index| cell(format!("newer message {index}"))));
    let mut view = TranscriptView::default();
    view.begin_search();
    view.paste_search("needle");

    let mut frames = 1;
    while view.advance_search(&cells) {
        frames += 1;
        assert!(frames < 16, "short entries must not cost one frame each");
    }
    assert!(frames > 1, "large histories must still yield to input");
    let found = view.search.current.as_ref().expect("oldest match");
    assert_eq!(
        (found.anchor.index, found.anchor.offset..found.end),
        (0, 0..6)
    );
}

#[test]
fn replacing_scanned_content_restarts_search_while_reading_another_entry() {
    let retired = format!("retired needle{}", "z".repeat(SCAN_BYTES * 2));
    let mut cells = vec![cell("reader stays here"), cell(retired)];
    let mut view = TranscriptView {
        area: Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
        ),
        ..TranscriptView::default()
    };
    view.jump_to_entry(&cells, /*index*/ 0);
    let reading = view.position;
    view.begin_search();
    view.paste_search("needle");
    assert!(view.advance_search(&cells));

    let replacement = cell("replacement needle");
    view.replace_range(&cells, 1..2, &replacement);
    cells.splice(1..2, [Arc::clone(&replacement)]);
    assert_eq!(view.position, reading);
    finish_scan(&mut view, &cells);
    let found = view.search.current.as_ref().expect("match in replacement");
    assert_eq!(
        (
            found.anchor.key,
            found.anchor.index,
            found.anchor.offset..found.end
        ),
        (EntryKey::cell(&replacement), 1, 12..18),
    );
}

#[test]
fn search_resumes_in_prepended_history_without_rescanning_the_newer_entry() {
    let newest = cell("newer messages");
    let mut cells = vec![Arc::clone(&newest)];
    let mut view = TranscriptView {
        history: TranscriptHistoryState::Partial,
        ..TranscriptView::default()
    };
    view.begin_search();
    view.paste_search("needle");
    finish_scan(&mut view, &cells);
    assert!(view.search.needs_history(view.history));
    view.history = TranscriptHistoryState::LoadingOlder;
    assert!(!view.advance_search(&cells));
    cells.insert(/*index*/ 0, cell("old NEEDLE"));
    view.history = TranscriptHistoryState::Complete;
    view.history_loaded(&cells, 0..1);
    finish_scan(&mut view, &cells);
    let found = view.search.current.as_ref().expect("match in earlier page");
    assert_eq!(
        (
            found.anchor.key,
            found.anchor.index,
            found.anchor.offset,
            found.end
        ),
        (EntryKey::cell(&cells[0]), 0, 4, 10)
    );
    assert!(!view.search.needs_history(view.history));
}

#[test]
fn empty_find_keeps_following_new_output_after_a_page_join() {
    let mut cells = vec![cell("previous computer action"), cell("latest answer")];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 8,
    );
    let mut view = TranscriptView {
        area,
        history: TranscriptHistoryState::LoadingOlder,
        ..TranscriptView::default()
    };
    view.begin_search();
    view.sync_live_tail(/*width*/ 40, /*key*/ None, |_| {
        Some(vec![HyperlinkLine::from("stream before page")])
    });
    finish_scan(&mut view, &cells);
    view.render(area, &mut Buffer::empty(area), &cells);

    cells.insert(/*index*/ 0, cell("older computer action"));
    view.history_loaded(&cells, 0..1);
    let group = cell("joined computer actions");
    view.replace_group(&cells, 0..2, &group);
    cells.splice(0..2, [group]);
    view.history = TranscriptHistoryState::Complete;
    cells.push(cell("new committed answer"));
    view.sync_live_tail(/*width*/ 40, /*key*/ None, |_| {
        Some(vec![HyperlinkLine::from("stream after page")])
    });
    finish_scan(&mut view, &cells);
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, &cells);
    assert_eq!(
        (
            view.is_following(),
            view.is_search_active(),
            view.held_reading.is_none()
        ),
        (true, true, true),
        "{buffer:?}",
    );
    insta::assert_snapshot!(
        "empty_find_follows_output_after_page_join",
        format!("{buffer:?}")
    );
}

#[test]
fn empty_find_preserves_reading_inside_a_joined_group() {
    let mut cells = vec![cell("displayed computer action"), cell("latest answer")];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 4,
    );
    let mut view = TranscriptView {
        area,
        ..TranscriptView::default()
    };
    view.jump_to_entry(&cells, /*index*/ 0);
    view.begin_search();
    finish_scan(&mut view, &cells);
    let mut before = Buffer::empty(area);
    view.render(area, &mut before, &cells);

    cells.insert(/*index*/ 0, cell("older computer action"));
    view.history_loaded(&cells, 0..1);
    let group = cell("joined computer actions");
    view.replace_group(&cells, 0..2, &group);
    cells.splice(0..2, [group]);
    view.handle_search_key(
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        &cells,
    );
    view.handle_search_key(
        KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        &cells,
    );
    finish_scan(&mut view, &cells);
    let mut after = Buffer::empty(area);
    view.render(area, &mut after, &cells);
    assert_eq!(after, before);
    view.cancel_search();
    view.render(area, &mut after, &cells);
    assert_eq!(after, before);
}

#[test]
fn previous_match_continues_through_regrouped_pages_without_revisiting_newer_hits() {
    let middle = cell("needle middle");
    let newest = cell("needle newest");
    let mut cells = vec![
        cell("newer group"),
        Arc::clone(&middle),
        Arc::clone(&newest),
    ];
    let mut view = TranscriptView {
        history: TranscriptHistoryState::Partial,
        ..TranscriptView::default()
    };
    view.begin_search();
    view.paste_search("needle");
    finish_scan(&mut view, &cells);
    view.handle_search_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        &cells,
    );
    finish_scan(&mut view, &cells);
    assert_eq!(
        view.search.current.as_ref().unwrap().anchor.key,
        EntryKey::cell(&middle)
    );

    view.handle_search_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        &cells,
    );
    finish_scan(&mut view, &cells);
    assert!(view.search.needs_history(view.history));
    cells.splice(0..0, [cell("earlier context"), cell("older group")]);
    view.history_loaded(&cells, 0..2);
    let group = cell("joined computer group");
    view.replace_group(&cells, 1..3, &group);
    cells.splice(1..3, [group]);
    finish_scan(&mut view, &cells);
    assert_eq!(
        (
            view.search.current.as_ref().unwrap().anchor.key,
            view.search.needs_history(view.history)
        ),
        (EntryKey::cell(&middle), true),
    );
    insta::assert_snapshot!(view.search.status_line(/*width*/ 80, view.history).to_string(), @"Searching earlier history… · full transcript · esc close");

    let oldest = cell("needle oldest");
    cells.insert(/*index*/ 0, Arc::clone(&oldest));
    view.history = TranscriptHistoryState::Complete;
    view.history_loaded(&cells, 0..1);
    finish_scan(&mut view, &cells);
    let mut hits = vec![view.search.current.as_ref().unwrap().anchor.key];
    for character in ['n', 'n'] {
        view.handle_search_key(
            KeyEvent::new(KeyCode::Char(character), KeyModifiers::CONTROL),
            &cells,
        );
        finish_scan(&mut view, &cells);
        hits.push(view.search.current.as_ref().unwrap().anchor.key);
    }
    assert_eq!(
        hits,
        vec![
            EntryKey::cell(&oldest),
            EntryKey::cell(&middle),
            EntryKey::cell(&newest)
        ]
    );
}

#[test]
fn previous_match_finds_the_prepended_part_of_a_regrouped_entry() {
    let newest = cell("needle newest");
    let mut cells = vec![cell("newer group"), Arc::clone(&newest)];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 30, /*height*/ 3,
    );
    let mut view = TranscriptView {
        area,
        history: TranscriptHistoryState::Partial,
        ..TranscriptView::default()
    };
    view.begin_search();
    view.paste_search("needle");
    finish_scan(&mut view, &cells);
    view.handle_search_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        &cells,
    );
    finish_scan(&mut view, &cells);
    assert!(view.search.needs_history(view.history));

    let older = cell("needle older prefix");
    cells.insert(/*index*/ 0, Arc::clone(&older));
    view.history = TranscriptHistoryState::Complete;
    view.history_loaded(&cells, 0..1);
    let group = cell("needle older prefix and newer group");
    view.replace_group(&cells, 0..2, &group);
    cells.splice(0..2, [group]);
    finish_scan(&mut view, &cells);
    assert_eq!(
        view.search.current.as_ref().unwrap().anchor.key,
        EntryKey::cell(&older)
    );
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, &cells);
    assert_eq!(view.visible[0].key, EntryKey::cell(&older));
    assert!(buffer[(0, 0)].modifier.contains(Modifier::REVERSED));

    view.handle_search_key(
        KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
        &cells,
    );
    finish_scan(&mut view, &cells);
    assert_eq!(
        (
            view.search.current.as_ref().unwrap().anchor.key,
            view.held_reading.is_none()
        ),
        (EntryKey::cell(&newest), true)
    );
}

#[test]
fn enter_preserves_the_initial_scan_and_each_older_page_until_the_first_match() {
    let mut cells = vec![cell("recent text ".repeat(SCAN_BYTES))];
    let mut view = TranscriptView {
        area: Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
        ),
        history: TranscriptHistoryState::Partial,
        ..TranscriptView::default()
    };
    view.begin_search();
    view.paste_search("LONG T0000 USER");
    view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
    insta::assert_snapshot!(view.search.status_line(/*width*/ 80, view.history).to_string(), @"Searching… · full transcript · esc close");
    assert!(view.advance_search(&cells));
    let Progress::Scanning(before) = view.search.progress else {
        panic!("initial scan should still have unread text");
    };
    let layout = Arc::clone(
        &view
            .search
            .scanning_layout
            .as_ref()
            .expect("scanning layout")
            .1,
    );
    for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
        view.handle_search_key(KeyEvent::new(KeyCode::Enter, modifiers), &cells);
    }
    let Progress::Scanning(after) = view.search.progress else {
        panic!("confirmation must retain the scan cursor");
    };
    assert_eq!(
        (after.anchor, after.direction),
        (before.anchor, Direction::Older)
    );
    assert!(Arc::ptr_eq(
        &layout,
        &view
            .search
            .scanning_layout
            .as_ref()
            .expect("retained layout")
            .1
    ));
    finish_scan(&mut view, &cells);
    assert!(view.search.needs_history(view.history));
    insta::assert_snapshot!(view.search.status_line(/*width*/ 80, view.history).to_string(), @"Searching earlier history… · full transcript · esc close");

    for page in ["middle page without a hit", "LONG T0000 USER"] {
        view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
        assert!(view.search.needs_history(view.history));
        view.history = TranscriptHistoryState::LoadingOlder;
        assert!(!view.advance_search(&cells));
        view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
        assert!(!view.search.needs_history(view.history));
        cells.insert(/*index*/ 0, cell(page));
        view.history = TranscriptHistoryState::Partial;
        view.history_loaded(&cells, 0..1);
        view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
        assert_eq!(view.search.page_start, Some(EntryKey::cell(&cells[0])));
        finish_scan(&mut view, &cells);
    }
    let found = view
        .search
        .current
        .as_ref()
        .expect("first match in oldest page");
    assert_eq!(
        (
            found.anchor.key,
            found.anchor.index,
            found.anchor.offset..found.end
        ),
        (EntryKey::cell(&cells[0]), 0, 0..15),
    );
    assert!(!view.search.needs_history(view.history));
}

#[test]
fn search_scans_a_page_inserted_after_the_retained_session_header() {
    let header: Arc<dyn HistoryCell> =
        Arc::new(crate::history_cell::SessionHeaderHistoryCell::new(
            "test model".to_string(),
            /*reasoning_effort*/ None,
            /*show_fast_status*/ false,
            std::path::PathBuf::from("/project"),
            "test",
        ));
    let mut cells = vec![Arc::clone(&header), cell("recent messages")];
    let mut view = TranscriptView {
        area: Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
        ),
        history: TranscriptHistoryState::Partial,
        ..TranscriptView::default()
    };
    view.begin_search();
    view.paste_search("needle");
    finish_scan(&mut view, &cells);
    assert!(view.search.needs_history(view.history));
    cells.insert(/*index*/ 1, cell("older NEEDLE"));
    view.history = TranscriptHistoryState::Complete;
    view.history_loaded(&cells, 1..2);
    finish_scan(&mut view, &cells);
    let found = view
        .search
        .current
        .as_ref()
        .expect("match in page after header");
    assert_eq!(
        (
            found.anchor.key,
            found.anchor.index,
            found.anchor.offset,
            found.end
        ),
        (EntryKey::cell(&cells[1]), 1, 6, 12)
    );
    assert!(Arc::ptr_eq(&cells[0], &header));
}

#[test]
fn next_previous_highlight_and_cancel_share_the_transcript_position() {
    let cells = vec![cell("Needle one"), cell("Needle two")];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 3,
    );
    let mut view = TranscriptView {
        area,
        history: TranscriptHistoryState::Complete,
        ..TranscriptView::default()
    };
    view.jump_to_entry(&cells, /*index*/ 0);
    let original = view.position;
    view.begin_search();
    view.paste_search("needle");
    finish_scan(&mut view, &cells);
    assert_eq!(
        view.search.current.as_ref().map(|found| found.anchor.index),
        Some(1)
    );
    view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT), &cells);
    finish_scan(&mut view, &cells);
    assert_eq!(
        view.search.current.as_ref().map(|found| found.anchor.index),
        Some(0)
    );
    view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
    finish_scan(&mut view, &cells);
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, &cells);
    assert_eq!(
        (0..10)
            .map(|column| buffer[(column, 0)].modifier.contains(Modifier::REVERSED))
            .collect::<Vec<_>>(),
        vec![
            true, true, true, true, true, true, false, false, false, false
        ]
    );
    insta::assert_snapshot!(view.search.status_line(/*width*/ 80, view.history).to_string(), @"enter next · ctrl+p previous · full transcript · esc close");
    view.handle_search_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &cells);
    assert!(matches!(view.search.progress, Progress::Found));
    view.handle_search_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &cells);
    assert_eq!((view.position, view.search.is_active()), (original, false));
}

#[test]
fn legacy_search_shortcuts_navigate_in_both_directions_without_wrapping() {
    let cells = vec![
        cell("SHORT T0000 USER]"),
        cell("SHORT T0001 USER]"),
        cell("SHORT T0002 USER]"),
    ];
    let mut view = TranscriptView {
        history: TranscriptHistoryState::Complete,
        ..TranscriptView::default()
    };
    view.begin_search();
    view.paste_search("USER]");
    finish_scan(&mut view, &cells);
    let mut positions = Vec::new();
    for character in ['p', 'p', 'p', 'n', 'n', 'n'] {
        view.handle_key(
            KeyEvent::new(KeyCode::Char(character), KeyModifiers::CONTROL),
            &cells,
        );
        finish_scan(&mut view, &cells);
        positions.push((
            view.search.current.as_ref().unwrap().anchor.index,
            matches!(view.search.progress, Progress::Exhausted),
        ));
    }
    assert_eq!(
        positions,
        vec![
            (1, false),
            (0, false),
            (0, true),
            (1, false),
            (2, false),
            (2, true)
        ],
    );
    insta::assert_snapshot!(view.search.status_line(/*width*/ 80, view.history).to_string(), @"No more matches · enter next · ctrl+p previous · full transcript · esc close");
}

#[test]
fn find_reveals_hidden_command_output_and_restores_compact_presentation() {
    let output = format!(
        "{}hidden needle\n{}",
        "head\n".repeat(/*n*/ 20),
        "tail\n".repeat(/*n*/ 20)
    );
    let command = crate::exec_cell::ExecCell::new(
        crate::exec_cell::ExecCall {
            call_id: "command".to_string(),
            command: vec!["echo".to_string()],
            parsed: Vec::new(),
            output: Some(crate::exec_cell::CommandOutput::new(
                /*exit_code*/ 0, output,
            )),
            source: codex_app_server_protocol::CommandExecutionSource::Agent,
            start_time: None,
            duration: None,
            interaction_input: None,
        },
        /*animations_enabled*/ false,
    );
    assert!(
        !command
            .display_lines(/*width*/ 80)
            .iter()
            .any(|line| line.to_string().contains("hidden needle"))
    );
    let mut cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(command)];
    let mut view = TranscriptView {
        area: Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
        ),
        ..TranscriptView::default()
    };
    view.begin_search();
    view.paste_search("hidden needle");
    finish_scan(&mut view, &cells);
    assert!(view.search.current.is_some());
    assert!(view.is_detailed());
    view.handle_search_key(
        KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
        &cells,
    );
    assert!(view.is_detailed());
    view.handle_search_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &cells);
    assert_eq!(
        (view.is_detailed(), view.position),
        (false, Position::Latest)
    );
    view.jump_to_entry(&cells, /*index*/ 0);
    let mut compact = Buffer::empty(view.area);
    view.render(view.area, &mut compact, &cells);
    view.begin_search();
    view.render(view.area, &mut Buffer::empty(view.area), &cells);
    let joined = cell("regrouped command output");
    view.replace_group(&cells, 0..1, &joined);
    cells[0] = joined;
    view.cancel_search();
    let mut restored = Buffer::empty(view.area);
    view.render(view.area, &mut restored, &cells);
    assert_eq!(restored, compact);
}

#[test]
fn search_preserves_both_presentation_positions() {
    let cells = vec![
        cell("old needle"),
        cell("saved detailed position"),
        cell("saved compact position"),
    ];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 30, /*height*/ 2,
    );
    for detailed in [false, true] {
        let mut view = TranscriptView::default();
        view.render(area, &mut Buffer::empty(area), &cells);
        view.set_presentation(/*detailed*/ true, HistoryRenderMode::Rich);
        view.jump_to_entry(&cells, /*index*/ 1);
        let detailed_position = view.position;
        view.set_presentation(/*detailed*/ false, HistoryRenderMode::Rich);
        view.jump_to_entry(&cells, /*index*/ 2);
        let compact_position = view.position;
        view.set_presentation(detailed, HistoryRenderMode::Rich);
        let positions = (view.position, view.saved_position);

        view.begin_search();
        finish_scan(&mut view, &cells);
        assert_eq!((view.position, view.saved_position), positions);
        view.render(area, &mut Buffer::empty(area), &cells);

        view.paste_search("needle");
        finish_scan(&mut view, &cells);
        assert_eq!(view.search.current.as_ref().unwrap().anchor.index, 0);
        view.handle_search_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &cells);
        assert_eq!((view.position, view.saved_position), positions);
        view.render(area, &mut Buffer::empty(area), &cells);

        view.set_presentation(!detailed, HistoryRenderMode::Rich);
        assert_eq!(
            view.position,
            if detailed {
                compact_position
            } else {
                detailed_position
            },
        );
        view.render(area, &mut Buffer::empty(area), &cells);
    }
}

#[test]
fn failed_history_waits_for_explicit_retry_and_empty_query_cancels_loading() {
    let cells = vec![cell("nothing here")];
    let mut view = TranscriptView {
        history: TranscriptHistoryState::Partial,
        ..TranscriptView::default()
    };
    view.begin_search();
    view.paste_search("needle");
    finish_scan(&mut view, &cells);
    view.history = TranscriptHistoryState::LoadingOlder;
    assert!(!view.search.needs_history(view.history));
    // Loader transitions immediately determine requests and hints, without another search frame.
    view.history = TranscriptHistoryState::Failed;
    assert!(!view.search.needs_history(view.history));
    assert_eq!(
        view.search
            .status_line(/*width*/ 32, view.history)
            .to_string(),
        "ctrl+p retry · esc close",
    );
    view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
    assert_eq!(view.history, TranscriptHistoryState::Failed);
    view.handle_search_key(
        KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
        &cells,
    );
    view.advance_search(&cells);
    assert!(view.search.needs_history(view.history));
    view.handle_search_key(
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        &cells,
    );
    assert!(!view.advance_search(&cells));
    assert!(!view.search.needs_history(view.history));
    assert_eq!(view.position, Position::Latest);
}

#[test]
fn live_scanning_restarts_when_its_source_changes() {
    let mut view = TranscriptView {
        area: Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 4,
        ),
        ..TranscriptView::default()
    };
    view.begin_search();
    let retired = format!("needle{}", "z".repeat(SCAN_BYTES * 2));
    view.sync_live_tail(
        /*width*/ 80,
        /*key*/ None,
        |_| Some(vec![HyperlinkLine::from(retired)]),
    );
    view.paste_search("needle");
    assert!(view.advance_search(&[]));
    view.sync_live_tail(
        /*width*/ 80,
        /*key*/ None,
        |_| Some(vec![HyperlinkLine::from("new needle")]),
    );
    finish_scan(&mut view, &[]);
    let found = view
        .search
        .current
        .as_ref()
        .expect("current live source match");
    assert_eq!(found.anchor.offset..found.end, 4..10);
}

#[test]
fn live_match_stays_displayed_after_commit_and_a_fresh_query_searches_current_content() {
    let mut cells = vec![cell("earlier needle")];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 30, /*height*/ 3,
    );
    let mut view = TranscriptView {
        area,
        ..TranscriptView::default()
    };
    view.begin_search();
    view.sync_live_tail(
        /*width*/ 30,
        /*key*/ None,
        |_| Some(vec![HyperlinkLine::from("live needle")]),
    );
    view.paste_search("needle");
    finish_scan(&mut view, &cells);
    view.sync_live_tail(/*width*/ 30, /*key*/ None, |_| {
        Some(vec![HyperlinkLine::from("live needle again needle")])
    });
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, &cells);
    assert_eq!(view.visible[0].layout.text(), "live needle");
    view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
    finish_scan(&mut view, &cells);
    let found = view.search.current.as_ref().expect("new live match");
    assert_eq!(
        (found.anchor.key, found.anchor.offset..found.end),
        (EntryKey::Live, 18..24)
    );
    cells.push(cell("live needle again needle committed needle"));
    view.sync_live_tail(/*width*/ 30, /*key*/ None, |_| {
        Some(vec![HyperlinkLine::from("current needle")])
    });
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, &cells);
    assert!(view.held_reading.is_some());
    assert_eq!(view.visible[0].layout.text(), "live needle again needle");
    view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
    finish_scan(&mut view, &cells);
    let found = view.search.current.as_ref().expect("new committed match");
    assert_eq!(
        (found.anchor.key, found.anchor.offset..found.end),
        (EntryKey::cell(&cells[1]), 35..41)
    );
    view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
    finish_scan(&mut view, &cells);
    let found = view.search.current.as_ref().expect("current live match");
    assert_eq!(
        (found.anchor.key, found.anchor.offset..found.end),
        (EntryKey::Live, 8..14)
    );
    view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT), &cells);
    finish_scan(&mut view, &cells);
    assert!(view.held_reading.is_none());
    assert_eq!(
        view.search
            .current
            .as_ref()
            .expect("earlier match")
            .anchor
            .key,
        EntryKey::cell(&cells[1])
    );
    view.handle_search_key(
        KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL),
        &cells,
    );
    view.paste_search("current");
    finish_scan(&mut view, &cells);
    let found = view.search.current.as_ref().expect("fresh current match");
    assert_eq!(
        (found.anchor.key, found.anchor.offset..found.end),
        (EntryKey::Live, 0..7)
    );
    view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
    finish_scan(&mut view, &cells);
    let found = view
        .search
        .current
        .as_ref()
        .expect("exhausted search retains its match");
    assert_eq!(
        (found.anchor.key, found.anchor.offset..found.end),
        (EntryKey::Live, 0..7)
    );
    cells.push(cell("new current"));
    view.sync_live_tail(
        /*width*/ 30,
        /*key*/ None,
        |_| Some(vec![HyperlinkLine::from("other")]),
    );
    view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
    finish_scan(&mut view, &cells);
    let found = view
        .search
        .current
        .as_ref()
        .expect("commit after exhausting live matches");
    assert_eq!(
        (found.anchor.key, found.anchor.offset..found.end),
        (EntryKey::cell(&cells[2]), 4..11)
    );
    view.jump_to_latest();
    assert!(view.held_reading.is_none());
    assert!(view.search.current.is_none());
}

#[test]
fn leaving_a_regrouped_search_match_restarts_from_current_history() {
    let retired = cell("retired needle");
    let mut cells = vec![
        Arc::clone(&retired),
        cell("trailing words ".repeat(/*n*/ 20)),
    ];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 30, /*height*/ 3,
    );
    let mut view = TranscriptView {
        area,
        ..TranscriptView::default()
    };
    view.begin_search();
    view.paste_search("needle");
    finish_scan(&mut view, &cells);
    view.render(area, &mut Buffer::empty(area), &cells);

    cells.splice(0..0, [cell("surviving before"), cell("older group")]);
    view.history_loaded(&cells, 0..2);
    let replacement = cell("merged needle");
    view.replace_group(&cells, 1..3, &replacement);
    cells.splice(1..3, [Arc::clone(&replacement)]);
    finish_scan(&mut view, &cells);
    assert_eq!(
        view.search.current.as_ref().expect("held match").anchor.key,
        EntryKey::cell(&retired)
    );
    assert!(view.held_reading.is_some());

    view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
    assert!(view.held_reading.is_none());
    assert!(view.search.current.is_none());
    view.handle_search_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells);
    finish_scan(&mut view, &cells);
    let found = view.search.current.as_ref().expect("current group match");
    assert_eq!(
        (
            found.anchor.key,
            found.anchor.index,
            found.anchor.offset..found.end,
        ),
        (EntryKey::cell(&replacement), 1, 7..13),
    );
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, &cells);
    assert_eq!(view.visible[0].key, EntryKey::cell(&replacement));
    assert!(buffer[(7, 0)].modifier.contains(Modifier::REVERSED));
}

#[test]
fn refining_a_query_restores_the_reading_position_while_history_loads() {
    let cells = vec![
        cell("Earlier context"),
        cell("Middle context"),
        cell("Last answer"),
        cell("Fixture link"),
    ];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 30, /*height*/ 6,
    );
    let mut view = TranscriptView {
        history: TranscriptHistoryState::Partial,
        ..TranscriptView::default()
    };
    for position in [
        Position::Latest,
        Position::Reading(Anchor {
            key: EntryKey::cell(&cells[0]),
            index: 0,
            offset: 0,
            row_bias: 0,
        }),
    ] {
        view.position = position;
        view.begin_search();
        let mut before = Buffer::empty(area);
        view.render(area, &mut before, &cells);
        view.handle_search_key(
            KeyEvent::new(KeyCode::Char('F'), KeyModifiers::NONE),
            &cells,
        );
        finish_scan(&mut view, &cells);
        assert_eq!(
            view.search.current.as_ref().unwrap().anchor.key,
            EntryKey::cell(&cells[3])
        );

        view.paste_search("IRST ANSWER");
        finish_scan(&mut view, &cells);
        assert!(view.search.needs_history(view.history));
        let mut loading = Buffer::empty(area);
        view.render(area, &mut loading, &cells);
        assert_eq!(loading, before);
        view.handle_search_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &cells);
    }
}

#[test]
fn bounded_query_paste_keeps_the_suffix_and_whole_graphemes() {
    let mut view = TranscriptView::default();
    view.begin_search();
    view.paste_search("X\r\nY\rZ");
    assert_eq!(view.search.editor.text(), "X\nY\nZ");
    view.cancel_search();
    view.begin_search();
    let original = format!("{}Z", "a".repeat(QUERY_BYTES - 2));
    view.paste_search(&original);
    view.handle_search_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &[]);
    view.paste_search("e\u{301}");
    assert_eq!(
        (view.search.editor.text(), view.search.query_truncated),
        (original.as_str(), true)
    );
    view.paste_search("b");
    let full = format!("{}bZ", "a".repeat(QUERY_BYTES - 2));
    view.handle_search_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), &[]);
    assert_eq!(
        (view.search.editor.text(), view.search.query_truncated),
        (full.as_str(), true)
    );
    insta::assert_snapshot!(view.search.status_line(/*width*/ 100, view.history).to_string(), @"Searching… · full transcript · esc close · query limited to 4 KiB");
    view.handle_search_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE), &[]);
    assert_eq!(view.search.editor.text(), &full[..full.len() - 1]);
}

#[test]
fn query_projection_scrolls_the_existing_editor_and_tracks_its_caret() {
    let mut view = TranscriptView::default();
    view.begin_search();
    view.paste_search("abcdefghijklmno");
    let (end, end_cursor) = view.search_footer(/*width*/ 14).expect("query footer");
    view.handle_search_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &[]);
    let (start, start_cursor) = view.search_footer(/*width*/ 14).expect("query footer");
    assert_eq!(
        (end.to_string(), end_cursor, start.to_string(), start_cursor),
        (
            "Find: ijklmno ".to_string(),
            13,
            "Find: abcdefgh".to_string(),
            6
        ),
    );
}

#[test]
fn selection_keys_precede_find_and_typing_returns_to_query() {
    let cells = vec![cell("needle")];
    let mut view = TranscriptView::default();
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 3,
    );
    view.render(area, &mut Buffer::empty(area), &cells);
    view.begin_search();
    view.paste_search("needle");
    finish_scan(&mut view, &cells);
    view.render(area, &mut Buffer::empty(area), &cells);
    view.handle_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
        &cells,
    );
    view.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &cells);
    assert!(matches!(
        view.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &cells
        ),
        Some(ViewAction::Copy(_))
    ));
    assert!(view.is_search_active());
    view.handle_key(
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        &cells,
    );
    assert_eq!(
        (view.selection.is_none(), view.search.editor.text()),
        (true, "needlex")
    );
}
