//! Selection keeps displayed stream revisions and extends by valid text units across interaction.

use super::*;
use crate::exec_cell::CommandOutput;
use crate::exec_cell::ExecCall;
use crate::exec_cell::ExecCell;
use crate::history_cell::AgentMarkdownCell;
use crate::history_cell::AgentMessageCell;
use crate::history_cell::PlainHistoryCell;
use crate::history_cell::StreamingAgentTailCell;
use codex_app_server_protocol::CommandExecutionSource;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use pretty_assertions::assert_eq;
use std::time::Instant;

#[test]
fn selection_highlights_hard_breaks_within_and_between_entries() {
    let mut frames = Vec::new();
    for (lines, end_row) in [
        (vec![vec!["code", "next"]], 1),
        (vec![vec!["code"], vec!["next"]], 2),
    ] {
        let cells: Vec<Arc<dyn HistoryCell>> = lines
            .into_iter()
            .map(|lines| {
                Arc::new(PlainHistoryCell::new(
                    lines.into_iter().map(Line::from).collect(),
                )) as _
            })
            .collect();
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 8, /*height*/ 3,
        );
        let mut view = TranscriptView::default();
        let mut buffer = Buffer::empty(area);
        view.render(area, &mut buffer, &cells);
        view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 1);
        view.extend_selection(/*column*/ 7, /*row*/ 0);
        view.render(area, &mut buffer, &cells);
        assert_eq!(view.selected_text(&cells).as_deref(), Some("code"));
        assert!(
            !buffer[(4, 0)]
                .modifier
                .contains(ratatui::style::Modifier::REVERSED)
        );
        view.extend_selection(/*column*/ 0, end_row);
        view.render(area, &mut buffer, &cells);
        assert_eq!(view.selected_text(&cells).as_deref(), Some("code\n"));
        frames.push(format!("{buffer:?}"));
    }
    insta::assert_snapshot!(frames.join("\n"));
}

#[test]
fn copy_shortcuts_clear_selection_only_after_confirmed_delivery() {
    use crate::clipboard_copy::CopyStatus;

    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(PlainHistoryCell::new(vec![
        "selected\tca\rfé\x1b\u{85}".into(),
    ]))];
    for key in [
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::SUPER),
        KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ),
        KeyEvent::new(
            KeyCode::Char('C'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ),
        KeyEvent::new(KeyCode::Char('C'), KeyModifiers::CONTROL),
    ] {
        for status in [CopyStatus::Confirmed, CopyStatus::Unconfirmed] {
            for searching in [false, true] {
                let mut view = TranscriptView::default();
                render(&mut view, &cells, /*width*/ 32, /*height*/ 3);
                if searching {
                    view.begin_search();
                }
                view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
                let action = view.handle_key(key, &cells);
                assert_eq!(
                    matches!(&action, Some(ViewAction::CopyAndFollow(_))),
                    key.code == KeyCode::Enter
                );
                let Some(ViewAction::Copy(text) | ViewAction::CopyAndFollow(text)) = action else {
                    panic!("selection key must request a copy");
                };
                assert_eq!(text, "selected\tcafé");
                let failed = view.copy_selected_text_with(&cells, &text, |text| {
                    assert_eq!(text, "selected\tcafé");
                    Err("clipboard unavailable".to_string())
                });
                assert_eq!(failed, Err("clipboard unavailable".to_string()));
                assert_eq!(
                    view.selected_text(&cells).as_deref(),
                    Some("selected\tcafé")
                );

                let Some(ViewAction::Copy(text) | ViewAction::CopyAndFollow(text)) =
                    view.handle_key(key, &cells)
                else {
                    panic!("failed copy must remain retryable");
                };
                let copied = view.copy_selected_text_with(&cells, &text, |text| {
                    assert_eq!(text, "selected\tcafé");
                    Ok(status)
                });
                assert_eq!(copied, Ok(status));
                assert_eq!(
                    (view.selected_text(&cells), view.is_search_active()),
                    (
                        (status == CopyStatus::Unconfirmed).then(|| text.clone()),
                        searching
                    )
                );
                assert_eq!(view.selection.is_none(), status == CopyStatus::Confirmed);
                if !searching && status == CopyStatus::Confirmed {
                    assert!(
                        view.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells)
                            .is_none()
                    );
                }
            }
        }
    }
}

#[test]
fn command_c_ignores_release_and_copies_only_an_active_selection() {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(PlainHistoryCell::new(vec![
        "selected café".into(),
    ]))];
    let mut view = TranscriptView::default();
    let command_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::SUPER);
    render(&mut view, &cells, /*width*/ 24, /*height*/ 3);
    assert!(view.handle_key(command_c, &cells).is_none());
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 3);
    assert!(
        view.handle_key(
            KeyEvent::new_with_kind(
                KeyCode::Char('c'),
                KeyModifiers::SUPER,
                KeyEventKind::Release,
            ),
            &cells,
        )
        .is_none()
    );
    assert_eq!(view.selected_text(&cells).as_deref(), Some("selected café"));
    let Some(ViewAction::Copy(text)) = view.handle_key(command_c, &cells) else {
        panic!("Cmd+C must copy the transcript selection");
    };
    assert_eq!(text, "selected café");
    view.copy_selected_text_with(&cells, &text, |_| {
        Ok(crate::clipboard_copy::CopyStatus::Confirmed)
    })
    .unwrap();
    assert!(view.handle_key(command_c, &cells).is_none());
    assert!(
        view.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &cells
        )
        .is_none()
    );
}

#[test]
fn enter_on_an_empty_selection_does_not_copy_or_reach_the_composer() {
    let cells: Vec<Arc<dyn HistoryCell>> =
        vec![Arc::new(PlainHistoryCell::new(vec!["select this".into()]))];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 32, /*height*/ 3);
    view.begin_selection(&cells, /*column*/ 0, /*row*/ 0, /*clicks*/ 1);
    assert!(matches!(
        view.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &cells),
        Some(ViewAction::Changed)
    ));
    assert_eq!(view.selected_text(&cells), None);
}

fn agent(lines: &[&str]) -> Arc<dyn HistoryCell> {
    Arc::new(AgentMessageCell::new(
        lines
            .iter()
            .map(|line| Line::from((*line).to_owned()))
            .collect(),
        /*is_first_line*/ true,
    ))
}

fn render(
    view: &mut TranscriptView,
    cells: &[Arc<dyn HistoryCell>],
    width: u16,
    height: u16,
) -> String {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, cells);
    (0..height)
        .map(|row| {
            (0..width)
                .map(|column| buffer[(column, row)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_owned()
}

fn live(view: &mut TranscriptView, cell: &dyn HistoryCell, revision: u64) {
    let key = ActiveCellTranscriptKey {
        cacheable: true,
        revision,
        is_stream_continuation: cell.is_stream_continuation(),
        animation_tick: None,
    };
    view.sync_live_tail(/*width*/ 32, Some(key), |width| {
        Some(cell.display_hyperlink_lines(width))
    });
}

fn release(view: &mut TranscriptView, cells: &[Arc<dyn HistoryCell>], column: u16, row: u16) {
    view.handle_mouse(
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        },
        cells,
    );
}

#[test]
fn clicking_and_clearing_selection_preserves_the_separator_row() {
    let mut frames = Vec::new();
    for prepend in [false, true] {
        let mut cells = vec![
            agent(&["first"]),
            agent(&["alpha beta gamma", "second line", "third line"]),
            agent(&["tail"]),
        ];
        let mut view = TranscriptView::default();
        view.jump_to_entry(&cells, /*index*/ 1);
        render(&mut view, &cells, /*width*/ 32, /*height*/ 3);
        view.scroll(&cells, /*rows*/ -1);
        let before = render(&mut view, &cells, /*width*/ 32, /*height*/ 3);
        assert!(matches!(
            view.position,
            Position::Reading(Anchor { row_bias: 1, .. })
        ));
        if prepend {
            cells.insert(/*index*/ 0, agent(&["older"]));
            view.history_loaded(&cells, 0..1);
            assert_eq!(
                render(&mut view, &cells, /*width*/ 32, /*height*/ 3),
                before
            );
        }
        let position = view.position;
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 9,
            row: 1,
            modifiers: KeyModifiers::NONE,
        };
        view.handle_mouse(down, &cells);
        release(&mut view, &cells, /*column*/ 9, /*row*/ 1);
        assert_eq!(
            (
                render(&mut view, &cells, /*width*/ 32, /*height*/ 3),
                view.position,
                view.selected_text(&cells),
            ),
            (before.clone(), position, None)
        );

        // Keep the real first click's coordinates/count, without depending on test scheduling.
        view.last_click.as_mut().expect("first mouse down").0 = Instant::now();
        view.handle_mouse(down, &cells);
        release(&mut view, &cells, /*column*/ 9, /*row*/ 1);
        assert_eq!(
            (
                render(&mut view, &cells, /*width*/ 32, /*height*/ 3),
                view.position,
                view.selected_text(&cells),
            ),
            (before.clone(), position, Some("beta".to_string()))
        );
        view.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &cells);
        let after = render(&mut view, &cells, /*width*/ 32, /*height*/ 3);
        assert_eq!(
            (after.clone(), view.position, view.selected_text(&cells)),
            (before, position, None)
        );
        // Repeated clicks on padding or a separator must not select adjacent source text.
        for (column, row) in [(31, down.row), (down.column, 0)] {
            for clicks in [2, 3] {
                view.last_click = Some((Instant::now(), column, row, clicks - 1));
                view.handle_mouse(
                    MouseEvent {
                        column,
                        row,
                        ..down
                    },
                    &cells,
                );
                assert_eq!(view.selected_text(&cells), None);
            }
        }
        frames.push(format!("prepend={prepend}\n{after}"));
    }
    insta::assert_snapshot!(frames.join("\n\n"), @"
    prepend=false

    • alpha beta gamma
      second line

    prepend=true

    • alpha beta gamma
      second line
    ");
}

#[test]
fn live_selection_survives_commits_finalization_new_live_content_and_resize() {
    let mut cells = vec![agent(&["first"])];
    let mut view = TranscriptView::default();
    let tail =
        StreamingAgentTailCell::new(vec!["tail selected".into()], /*is_first_line*/ false);
    live(&mut view, &tail, /*revision*/ 1);
    render(&mut view, &cells, /*width*/ 32, /*height*/ 4);
    view.begin_selection(&cells, /*column*/ 4, /*row*/ 1, /*clicks*/ 3);
    release(&mut view, &cells, /*column*/ 4, /*row*/ 1);
    assert_eq!(view.selected_text(&cells), Some("tail selected".to_owned()));

    cells.push(Arc::new(AgentMessageCell::new(
        vec!["tail selected".into()],
        /*is_first_line*/ false,
    )));
    let finalized: Arc<dyn HistoryCell> = Arc::new(AgentMarkdownCell::new(
        "first\ntail selected".to_owned(),
        std::path::Path::new("."),
    ));
    view.replace_range(&cells, 0..2, &finalized);
    cells.splice(0..2, [finalized]);
    let unrelated = StreamingAgentTailCell::new(
        vec!["unrelated next tail".into()],
        /*is_first_line*/ true,
    );
    live(&mut view, &unrelated, /*revision*/ 2);
    insta::assert_snapshot!(render(&mut view, &cells, /*width*/ 32, /*height*/ 4), @"
    • first
      tail selected
    ");
    render(&mut view, &cells, /*width*/ 10, /*height*/ 6);
    assert_eq!(view.selected_text(&cells), Some("tail selected".to_owned()));

    cells.insert(/*index*/ 0, agent(&["older page"]));
    view.prepend_snapshot_history(&cells, 0..1);
    render(&mut view, &cells, /*width*/ 32, /*height*/ 5);
    assert_eq!(view.selected_text(&cells), Some("tail selected".to_owned()));
    assert_eq!(
        view.selection
            .as_ref()
            .expect("selection")
            .snapshot
            .cells
            .len(),
        2
    );
    view.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &cells);
    assert!(!view.is_following());
    assert!(view.selection.is_none());
    view.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &cells);
    assert!(view.is_following());
    assert!(render(&mut view, &cells, /*width*/ 32, /*height*/ 5).contains("unrelated next tail"));
}

#[test]
fn tool_selection_copies_its_displayed_revision_after_the_tool_commits() {
    let tool = ExecCell::new(
        ExecCall {
            call_id: "tool".to_owned(),
            command: vec!["printf".to_owned(), "selected tool output".to_owned()],
            parsed: Vec::new(),
            output: Some(CommandOutput::new(
                /*exit_code*/ 0,
                "selected tool output".to_owned(),
            )),
            source: CommandExecutionSource::Agent,
            start_time: None,
            duration: None,
            interaction_input: None,
        },
        /*animations_enabled*/ false,
    );
    let mut cells = Vec::new();
    let mut view = TranscriptView::default();
    live(&mut view, &tool, /*revision*/ 1);
    let visible = render(&mut view, &cells, /*width*/ 32, /*height*/ 8);
    let row = visible
        .lines()
        .enumerate()
        .filter_map(|(row, line)| line.contains("selected tool output").then_some(row))
        .last()
        .expect("tool output") as u16;
    view.begin_selection(&cells, /*column*/ 4, row, /*clicks*/ 3);
    release(&mut view, &cells, /*column*/ 4, row);
    let selected = view.selected_text(&cells).expect("selected tool output");
    cells.push(Arc::new(tool) as Arc<dyn HistoryCell>);
    live(
        &mut view,
        agent(&["another answer"]).as_ref(),
        /*revision*/ 2,
    );
    render(&mut view, &cells, /*width*/ 32, /*height*/ 8);
    let Some(ViewAction::Copy(copied)) = view.handle_key(
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        &cells,
    ) else {
        panic!("copy action");
    };
    assert_eq!(copied, selected);
}

#[test]
fn word_and_line_drags_keep_the_initial_unit_when_direction_changes() {
    let cells = vec![agent(&["alpha beta gamma", "second line", "third"])];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 32, /*height*/ 5);
    view.begin_selection(&cells, /*column*/ 9, /*row*/ 0, /*clicks*/ 2);
    view.extend_selection(/*column*/ 16, /*row*/ 0);
    assert_eq!(view.selected_text(&cells).as_deref(), Some("beta gamma"));
    view.extend_selection(/*column*/ 3, /*row*/ 0);
    assert_eq!(view.selected_text(&cells).as_deref(), Some("alpha beta"));
    view.extend_selection(/*column*/ 16, /*row*/ 0);
    assert_eq!(view.selected_text(&cells).as_deref(), Some("beta gamma"));

    view.begin_selection(&cells, /*column*/ 4, /*row*/ 1, /*clicks*/ 3);
    view.extend_selection(/*column*/ 3, /*row*/ 0);
    assert_eq!(
        view.selected_text(&cells).as_deref(),
        Some("alpha beta gamma\nsecond line\n")
    );
    view.extend_selection(/*column*/ 4, /*row*/ 2);
    assert_eq!(
        view.selected_text(&cells).as_deref(),
        Some("second line\nthird")
    );
}

#[test]
fn keyboard_selection_moves_by_grapheme_and_keeps_its_column_across_short_lines() {
    let cells = vec![
        agent(&["A👨‍👩‍👧‍👦界e\u{301}Z", "x", "1234567890"]),
        agent(&["tail"]),
    ];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 32, /*height*/ 5);
    view.begin_selection(&cells, /*column*/ 2, /*row*/ 0, /*clicks*/ 1);
    for _ in 0..4 {
        view.selection_key(&cells, KeyCode::Right);
    }
    assert_eq!(view.selected_text(&cells).as_deref(), Some("A👨‍👩‍👧‍👦界e\u{301}"));
    let original = view.selection.as_ref().expect("selection").end;
    for key in [KeyCode::Down, KeyCode::Down, KeyCode::Up, KeyCode::Up] {
        view.selection_key(&cells, key);
    }
    assert_eq!(view.selection.as_ref().expect("selection").end, original);
    for _ in 0..10 {
        view.selection_key(&cells, KeyCode::Down);
    }
    let end = view.selection.as_ref().expect("selection").end;
    assert!(end.index < cells.len());
    let layout = view
        .layout(&cells, end.index)
        .expect("valid endpoint layout");
    assert!(layout.text().is_char_boundary(end.offset));
    for _ in 0..10 {
        view.selection_key(&cells, KeyCode::Up);
    }
    assert_eq!(view.selection.as_ref().expect("selection").end, original);
}

#[test]
fn plain_and_shift_arrows_move_selection_on_press_and_repeat() {
    let cells = vec![agent(&["abc", "def", "ghi"])];
    for kind in [KeyEventKind::Press, KeyEventKind::Repeat] {
        for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
            for (code, expected) in [
                (KeyCode::Left, "d"),
                (KeyCode::Right, "e"),
                (KeyCode::Up, "bc\nd"),
                (KeyCode::Down, "ef\ng"),
            ] {
                let mut view = TranscriptView::default();
                render(&mut view, &cells, /*width*/ 32, /*height*/ 5);
                view.begin_selection(&cells, /*column*/ 3, /*row*/ 1, /*clicks*/ 1);
                assert!(matches!(
                    view.handle_key(KeyEvent::new_with_kind(code, modifiers, kind), &cells),
                    Some(ViewAction::Changed)
                ));
                assert_eq!(view.selected_text(&cells).as_deref(), Some(expected));
            }
        }
    }
}

#[test]
fn modified_arrows_end_selection_and_return_input_to_the_owner() {
    let cells = vec![agent(&["alpha beta gamma"])];
    for kind in [KeyEventKind::Press, KeyEventKind::Repeat] {
        for modifier in [
            KeyModifiers::CONTROL,
            KeyModifiers::ALT,
            KeyModifiers::CONTROL | KeyModifiers::ALT,
            KeyModifiers::SUPER,
            KeyModifiers::HYPER,
            KeyModifiers::META,
        ] {
            for modifiers in [modifier, modifier | KeyModifiers::SHIFT] {
                for code in [KeyCode::Left, KeyCode::Right, KeyCode::Up, KeyCode::Down] {
                    let mut view = TranscriptView::default();
                    render(&mut view, &cells, /*width*/ 32, /*height*/ 5);
                    view.begin_selection(
                        &cells, /*column*/ 9, /*row*/ 0, /*clicks*/ 2,
                    );
                    assert_eq!(view.selected_text(&cells).as_deref(), Some("beta"));
                    assert!(
                        view.handle_key(KeyEvent::new_with_kind(code, modifiers, kind), &cells)
                            .is_none()
                    );
                    assert_eq!(
                        (view.has_active_interaction(), view.selected_text(&cells)),
                        (false, None)
                    );
                }
            }
        }
    }
}

#[test]
fn released_arrows_preserve_selection_and_reading_position() {
    let cells = vec![agent(&["alpha beta gamma"])];
    for modifiers in [
        KeyModifiers::NONE,
        KeyModifiers::SHIFT,
        KeyModifiers::CONTROL,
        KeyModifiers::ALT | KeyModifiers::SHIFT,
    ] {
        for code in [KeyCode::Left, KeyCode::Right, KeyCode::Up, KeyCode::Down] {
            let mut view = TranscriptView::default();
            render(&mut view, &cells, /*width*/ 32, /*height*/ 5);
            view.begin_selection(&cells, /*column*/ 9, /*row*/ 0, /*clicks*/ 2);
            let position = view.position;
            let selected = view.selected_text(&cells);
            assert_eq!(selected.as_deref(), Some("beta"));
            assert!(
                view.handle_key(
                    KeyEvent::new_with_kind(code, modifiers, KeyEventKind::Release),
                    &cells,
                )
                .is_none()
            );
            assert_eq!(
                (view.position, view.selected_text(&cells)),
                (position, selected)
            );
        }
    }
}

#[test]
fn loading_older_history_extends_the_snapshot_without_adopting_new_commits() {
    let header = agent(&["header"]);
    let selected = agent(&["selected"]);
    let mut cells = vec![Arc::clone(&header), Arc::clone(&selected)];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 32, /*height*/ 5);
    view.begin_selection(&cells, /*column*/ 4, /*row*/ 2, /*clicks*/ 2);
    release(&mut view, &cells, /*column*/ 4, /*row*/ 2);
    cells.push(agent(&["new commit"]));
    let older = agent(&["older"]);
    cells.insert(/*index*/ 1, Arc::clone(&older));
    view.prepend_snapshot_history(&cells, 1..2);
    view.prepend_snapshot_history(&cells, 1..2);
    assert_eq!(
        view.snapshot_cells()
            .expect("snapshot")
            .iter()
            .map(EntryKey::cell)
            .collect::<Vec<_>>(),
        vec![
            EntryKey::cell(&header),
            EntryKey::cell(&older),
            EntryKey::cell(&selected)
        ],
    );
    render(&mut view, &cells, /*width*/ 32, /*height*/ 6);
    assert_eq!(view.selected_text(&cells).as_deref(), Some("selected"));
}
