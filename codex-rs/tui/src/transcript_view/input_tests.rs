//! Mouse gestures preserve selection anchors and accumulate fractional transcript scrolling.

use super::*;
use crate::history_cell::AgentMarkdownCell;
use pretty_assertions::assert_eq;

#[test]
fn mouse_scroll_speed_scales_rows_and_accumulates_fractional_movement() {
    use crate::history_cell::PlainHistoryCell;
    use crate::transcript_view::tests::text;

    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(PlainHistoryCell::new(
        (1..=24).map(|row| format!("row {row:02}").into()).collect(),
    ))];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 12, /*height*/ 4,
    );
    let mut frames = Vec::new();
    for (speed, deltas) in [
        (None, vec![-1, 1]),
        (Some(0.5), vec![0, -1, 0, 0, 1]),
        (Some(1.5), vec![-1, -2, 1, 2]),
        (Some(3.0), vec![-3, 3]),
    ] {
        let mut view = TranscriptView::default();
        if let Some(speed) = speed {
            view.mouse_scroll_speed = speed;
        }
        let mut buffer = Buffer::empty(area);
        view.render(area, &mut buffer, &cells);
        view.scroll(&cells, /*rows*/ -8);
        view.render(area, &mut buffer, &cells);
        let (index, mut row) = view.start(&cells);
        let up_events = deltas.len().div_ceil(2);
        for (event_index, delta) in deltas.into_iter().enumerate() {
            let kind = if event_index < up_events {
                MouseEventKind::ScrollUp
            } else {
                MouseEventKind::ScrollDown
            };
            view.handle_mouse(mouse(kind, /*column*/ 1, /*row*/ 1), &cells);
            row = row.saturating_add_signed(delta);
            assert_eq!(view.start(&cells), (index, row));
            view.render(area, &mut buffer, &cells);
            if event_index + 1 == up_events {
                frames.push(format!(
                    "speed {speed:?}, {up_events} up events:\n{}",
                    text(&buffer)
                ));
            }
        }
    }
    insta::assert_snapshot!(frames.join("\n\n"));
}

#[test]
fn decimal_mouse_scroll_speeds_preserve_whole_rows() {
    use crate::history_cell::PlainHistoryCell;

    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(PlainHistoryCell::new(
        (1..=120)
            .map(|row| format!("row {row:03}").into())
            .collect(),
    ))];
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 12, /*height*/ 4,
    );
    for speed in [0.1, 0.3] {
        let mut view = TranscriptView {
            mouse_scroll_speed: speed,
            ..TranscriptView::default()
        };
        view.render(area, &mut Buffer::empty(area), &cells);
        view.scroll(&cells, /*rows*/ -40);
        let (index, row) = view.start(&cells);
        for (kind, direction) in [
            (MouseEventKind::ScrollUp, -1),
            (MouseEventKind::ScrollDown, 1),
        ] {
            let (_, start_row) = view.start(&cells);
            for event in 1..=100 {
                view.handle_mouse(mouse(kind, /*column*/ 1, /*row*/ 1), &cells);
                let rows = (f64::from(event) * speed).floor() as isize * direction;
                assert_eq!(
                    view.start(&cells),
                    (index, start_row.saturating_add_signed(rows))
                );
            }
        }
        assert_eq!(view.start(&cells), (index, row));
    }
}

#[test]
fn fractional_mouse_scroll_does_not_pause_following_until_a_row_moves() {
    let (mut view, cells) = transcript(
        "one\n\ntwo\n\nthree\n\nfour\n\nfive\n\nsix",
        /*width*/ 12,
    );
    view.mouse_scroll_speed = 0.5;
    view.handle_mouse(
        mouse(MouseEventKind::ScrollUp, /*column*/ 4, /*row*/ 1),
        &cells,
    );
    assert!(view.is_following());
    view.handle_mouse(
        mouse(MouseEventKind::ScrollUp, /*column*/ 4, /*row*/ 1),
        &cells,
    );
    assert!(!view.is_following());
}

fn transcript(markdown: &str, width: u16) -> (TranscriptView, Vec<Arc<dyn HistoryCell>>) {
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(AgentMarkdownCell::new(
        markdown.into(),
        std::path::Path::new("/"),
    ))];
    let mut view = TranscriptView::default();
    let area = Rect::new(/*x*/ 2, /*y*/ 1, width, /*height*/ 8);
    view.render(area, &mut Buffer::empty(area), &cells);
    (view, cells)
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

#[test]
fn hover_links_match_wrapped_click_targets_and_refresh_after_scrolling() {
    let (mut view, cells) = transcript(
        "before [wide 界 wrapped label](https://example.com) after\n\nplain\n\nmore\n\nlast",
        /*width*/ 12,
    );
    let area = view.area;
    view.jump_to_beginning(&cells);
    view.render(area, &mut Buffer::empty(area), &cells);
    let mut targets = Vec::new();
    for row in area.y..area.bottom() {
        for column in area.x..area.right() {
            if let Some(url) = view.link_at(column, row) {
                targets.push((column, row, url));
            }
        }
    }
    assert!(!targets.is_empty());
    assert!(targets.iter().any(|(_, row, _)| *row != targets[0].1));
    for (column, row, url) in &targets {
        let action = view.handle_mouse(
            MouseEvent {
                modifiers: KeyModifiers::CONTROL,
                ..mouse(MouseEventKind::Down(MouseButton::Left), *column, *row)
            },
            &cells,
        );
        let actual = match action {
            Some(ViewAction::OpenLink(url)) => Some(url),
            _ => None,
        };
        assert_eq!(actual.as_ref(), Some(url));
    }
    assert_eq!(view.link_at(area.x - 1, area.y), None);
    assert_eq!(view.link_at(area.right(), area.y), None);
    view.scroll(&cells, /*rows*/ 20);
    view.render(area, &mut Buffer::empty(area), &cells);
    assert!(
        targets
            .iter()
            .any(|(column, row, _)| view.link_at(*column, *row).is_none())
    );
}

#[test]
fn shift_click_extends_a_double_clicked_word_in_both_directions() {
    let (mut view, cells) = transcript("alpha beta gamma", /*width*/ 24);
    let area = view.area;
    let mut buffer = Buffer::empty(area);
    for _ in 0..2 {
        if let Some((at, ..)) = &mut view.last_click {
            *at = std::time::Instant::now();
        }
        for kind in [
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::Up(MouseButton::Left),
        ] {
            view.handle_mouse(mouse(kind, /*column*/ 11, /*row*/ 1), &cells);
            view.render(area, &mut buffer, &cells);
        }
    }
    assert_eq!(view.selected_text(&cells).as_deref(), Some("beta"));

    let mut frames = Vec::new();
    for (column, expected) in [(17, "beta gamma"), (5, "alpha beta"), (17, "beta gamma")] {
        for kind in [
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::Up(MouseButton::Left),
        ] {
            let action = view.handle_mouse(
                MouseEvent {
                    modifiers: KeyModifiers::SHIFT,
                    ..mouse(kind, column, /*row*/ 1)
                },
                &cells,
            );
            assert!(matches!(action, Some(ViewAction::Changed)));
            view.render(area, &mut buffer, &cells);
            assert_eq!(view.selected_text(&cells).as_deref(), Some(expected));
        }
        frames.push(format!("{buffer:?}"));
    }
    // An ordinary click after extending still starts a fresh, empty selection.
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        view.handle_mouse(mouse(kind, /*column*/ 17, /*row*/ 1), &cells);
    }
    assert_eq!(view.selected_text(&cells), None);
    insta::assert_snapshot!(frames.join("\n\n"));
}

#[test]
fn shift_click_preserves_selection_units_and_allows_dragging() {
    for (clicks, expected) in [(1, "beta gam"), (3, "alpha beta gamma")] {
        let (mut view, cells) = transcript("alpha beta gamma", /*width*/ 24);
        view.begin_selection(&cells, /*column*/ 10, /*row*/ 1, clicks);
        view.handle_mouse(
            mouse(
                MouseEventKind::Drag(MouseButton::Left),
                /*column*/ 14,
                /*row*/ 1,
            ),
            &cells,
        );
        view.handle_mouse(
            mouse(
                MouseEventKind::Up(MouseButton::Left),
                /*column*/ 14,
                /*row*/ 1,
            ),
            &cells,
        );
        for kind in [
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::Drag(MouseButton::Left),
            MouseEventKind::Up(MouseButton::Left),
        ] {
            let column = if matches!(kind, MouseEventKind::Down(_)) {
                16
            } else {
                18
            };
            view.handle_mouse(
                MouseEvent {
                    modifiers: KeyModifiers::SHIFT,
                    ..mouse(kind, column, /*row*/ 1)
                },
                &cells,
            );
        }
        assert_eq!(view.selected_text(&cells).as_deref(), Some(expected));
    }
}

#[test]
fn ordinary_click_opens_bare_and_markdown_links_on_release() {
    for markdown in [
        "https://example.com/docs",
        "[docs](https://example.com/docs)",
    ] {
        for width in [12, 60] {
            let (mut view, cells) = transcript(markdown, width);
            let down = mouse(
                MouseEventKind::Down(MouseButton::Left),
                /*column*/ 4,
                /*row*/ 1,
            );
            assert!(matches!(
                view.handle_mouse(down, &cells),
                Some(ViewAction::Changed)
            ));
            // Match the app's redraw before dispatching the next mouse event.
            let area = view.area;
            view.render(area, &mut Buffer::empty(area), &cells);
            let up = MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                ..down
            };
            let Some(ViewAction::OpenLink(url)) = view.handle_mouse(up, &cells) else {
                panic!("click should open {markdown}");
            };
            assert_eq!(
                (
                    url.as_str(),
                    view.selected_text(&cells),
                    view.is_following()
                ),
                ("https://example.com/docs", None, true)
            );
            assert!(view.handle_mouse(up, &cells).is_none());
        }
    }
}

#[test]
fn dragging_a_link_selects_text_and_never_opens_it() {
    for return_to_origin in [false, true] {
        let (mut view, cells) = transcript(
            "[documentation](https://example.com/docs)",
            /*width*/ 60,
        );
        view.handle_mouse(
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                /*column*/ 4,
                /*row*/ 1,
            ),
            &cells,
        );
        view.handle_mouse(
            mouse(
                MouseEventKind::Drag(MouseButton::Left),
                /*column*/ 7,
                /*row*/ 1,
            ),
            &cells,
        );
        assert_eq!(view.selected_text(&cells).as_deref(), Some("doc"));
        let column = if return_to_origin { 4 } else { 7 };
        let action = view.handle_mouse(
            mouse(
                MouseEventKind::Up(MouseButton::Left),
                column,
                /*row*/ 1,
            ),
            &cells,
        );
        assert!(matches!(action, Some(ViewAction::Changed)));
        assert_eq!(
            view.selected_text(&cells).as_deref(),
            (!return_to_origin).then_some("doc")
        );
    }
}

#[test]
fn scrolling_or_releasing_elsewhere_cancels_link_activation() {
    for interruption in [
        mouse(MouseEventKind::ScrollUp, /*column*/ 4, /*row*/ 1),
        mouse(
            MouseEventKind::ScrollDown,
            /*column*/ 4,
            /*row*/ 1,
        ),
        mouse(
            MouseEventKind::Up(MouseButton::Left),
            /*column*/ 5,
            /*row*/ 1,
        ),
        mouse(
            MouseEventKind::Up(MouseButton::Left),
            /*column*/ 0,
            /*row*/ 0,
        ),
    ] {
        let (mut view, cells) = transcript("[docs](https://example.com/docs)", /*width*/ 60);
        view.handle_mouse(
            mouse(
                MouseEventKind::Down(MouseButton::Left),
                /*column*/ 4,
                /*row*/ 1,
            ),
            &cells,
        );
        assert!(!matches!(
            view.handle_mouse(interruption, &cells),
            Some(ViewAction::OpenLink(_))
        ));
        assert!(!matches!(
            view.handle_mouse(
                mouse(
                    MouseEventKind::Up(MouseButton::Left),
                    /*column*/ 4,
                    /*row*/ 1
                ),
                &cells
            ),
            Some(ViewAction::OpenLink(_))
        ));
    }
}

#[test]
fn modified_clicks_still_open_immediately() {
    for modifiers in [KeyModifiers::CONTROL, KeyModifiers::SUPER] {
        let (mut view, cells) = transcript("[docs](https://example.com/docs)", /*width*/ 60);
        let event = MouseEvent {
            modifiers,
            ..mouse(
                MouseEventKind::Down(MouseButton::Left),
                /*column*/ 4,
                /*row*/ 1,
            )
        };
        let Some(ViewAction::OpenLink(url)) = view.handle_mouse(event, &cells) else {
            panic!("modified click should open link");
        };
        assert_eq!(url, "https://example.com/docs");
        assert!(view.selection.is_none());
    }
}
