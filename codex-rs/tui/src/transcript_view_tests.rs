//! Viewport behavior and cache lifetime across scrolling, pagination, and redraws.

use super::*;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

fn cell(text: impl Into<String>) -> Arc<dyn HistoryCell> {
    Arc::new(crate::history_cell::PlainHistoryCell::new(
        text.into()
            .lines()
            .map(|line| line.to_owned().into())
            .collect(),
    ))
}

fn render(
    view: &mut TranscriptView,
    cells: &[Arc<dyn HistoryCell>],
    width: u16,
    height: u16,
) -> Buffer {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let mut buffer = Buffer::empty(area);
    view.render(area, &mut buffer, cells, /*highlight*/ None);
    buffer
}

fn text(buffer: &Buffer) -> String {
    (buffer.area.top()..buffer.area.bottom())
        .map(|y| {
            (buffer.area.left()..buffer.area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug)]
struct MutableHistoryCell {
    version: Arc<AtomicUsize>,
    renders: Arc<AtomicUsize>,
}

impl HistoryCell for MutableHistoryCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.renders.fetch_add(/*val*/ 1, Ordering::Relaxed);
        let version = self.version.load(Ordering::Relaxed);
        (0..8)
            .map(|row| Line::from(format!("version {version} row {row}")))
            .collect()
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.display_lines(/*width*/ 80)
    }

    fn has_stable_transcript_height(&self) -> bool {
        false
    }
}

#[test]
fn reading_survives_prepend_and_new_output_then_returns_to_latest() {
    let mut cells = vec![cell("older\nline two"), cell("current\nlast line")];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 20, /*height*/ 3);
    view.scroll(&cells, /*rows*/ -2);
    let before = text(&render(
        &mut view, &cells, /*width*/ 20, /*height*/ 3,
    ));
    cells.insert(/*index*/ 0, cell("loaded from history"));
    view.history_changed(&cells);
    cells.push(cell("new output"));
    assert_eq!(
        text(&render(
            &mut view, &cells, /*width*/ 20, /*height*/ 3
        )),
        before
    );
    cells.remove(/*index*/ 0);
    view.history_changed(&cells);
    assert_eq!(
        text(&render(
            &mut view, &cells, /*width*/ 20, /*height*/ 3
        )),
        before
    );
    view.jump_to_latest();
    insta::assert_snapshot!(text(&render(&mut view, &cells, /*width*/ 20, /*height*/ 3)), @"
    last line

    new output
    ");
}

#[test]
fn prepending_history_updates_pagination_position_before_the_next_frame() {
    let mut cells = vec![cell("current first"), cell("current last")];
    let mut view = TranscriptView::default();
    view.jump_to_entry(&cells, /*index*/ 0);
    assert!(view.near_start(&cells));
    render(&mut view, &cells, /*width*/ 20, /*height*/ 3);
    cells.remove(/*index*/ 0);
    view.history_changed(&cells);
    cells.insert(/*index*/ 0, cell("earlier\n".repeat(100)));
    view.history_changed(&cells);
    assert!(!view.near_start(&cells));
    cells.remove(/*index*/ 0);
    view.history_changed(&cells);
    let earlier = (0..100)
        .map(|index| cell(format!("earlier {index}")))
        .collect::<Vec<_>>();
    cells.splice(0..0, earlier);
    view.history_changed(&cells);
    assert!(!view.near_start(&cells));
    let Position::Reading(anchor) = view.position else {
        panic!("reading anchor");
    };
    assert_eq!(
        (anchor.key, anchor.index),
        (EntryKey::cell(&cells[100]), 100)
    );
}

#[test]
fn single_row_scrolling_crosses_an_entry_separator_in_both_directions() {
    let cells = vec![cell("first"), cell("second")];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 20, /*height*/ 1);
    let mut frames = Vec::new();
    for rows in [-1, -1, 1, 1] {
        view.scroll(&cells, rows);
        frames.push(text(&render(
            &mut view, &cells, /*width*/ 20, /*height*/ 1,
        )));
    }
    assert_eq!(frames, ["", "first", "", "second"]);
}

#[test]
fn resize_and_consolidation_clamp_the_row_within_the_same_entry() {
    let mut cells = vec![
        cell("older"),
        cell("alpha beta gamma delta epsilon zeta eta theta"),
    ];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 10, /*height*/ 2);
    view.scroll(&cells, /*rows*/ -1);
    let Position::Reading(anchor) = view.position else {
        panic!("reading anchor");
    };
    render(&mut view, &cells, /*width*/ 8, /*height*/ 2);
    assert_eq!(view.position, Position::Reading(anchor));
    let wide = render(&mut view, &cells, /*width*/ 60, /*height*/ 2);
    assert_eq!(
        text(&wide),
        "alpha beta gamma delta epsilon zeta eta theta\n"
    );

    let replacement = cell("final answer");
    view.replace_range(&cells, 0..2, &replacement);
    cells.splice(0..2, [replacement]);
    let completed = render(&mut view, &cells, /*width*/ 60, /*height*/ 2);
    assert!(!view.is_following());
    insta::assert_snapshot!(text(&completed), @"
    final answer

    ");
}

#[test]
fn mutable_history_formats_once_per_frame_and_refreshes_the_next_frame() {
    let version = Arc::new(AtomicUsize::new(/*v*/ 1));
    let renders = Arc::new(AtomicUsize::new(/*v*/ 0));
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(MutableHistoryCell {
        version: Arc::clone(&version),
        renders: Arc::clone(&renders),
    })];
    let mut view = TranscriptView::default();
    let first = render(&mut view, &cells, /*width*/ 24, /*height*/ 5);
    assert_eq!(renders.load(Ordering::Relaxed), 1);
    version.store(/*val*/ 2, Ordering::Relaxed);
    let second = render(&mut view, &cells, /*width*/ 24, /*height*/ 5);
    assert_eq!(renders.load(Ordering::Relaxed), 2);
    assert!(text(&first).contains("version 1 row 3"));
    insta::assert_snapshot!(text(&second), @"
    version 2 row 3
    version 2 row 4
    version 2 row 5
    version 2 row 6
    version 2 row 7
    ");
}
