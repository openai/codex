//! Backtrack selection preserves unrelated layout caches and the complete rendered viewport.

use super::*;
use crate::history_cell::UserHistoryCell;
use crate::keymap::RuntimeKeymap;
use pretty_assertions::assert_eq;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

#[derive(Debug)]
struct MeasuredCell {
    measurements: AtomicUsize,
}

impl HistoryCell for MeasuredCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        self.measurements.fetch_add(1, Ordering::Relaxed);
        vec!["history".into()]
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        vec!["history".into()]
    }
}

#[test]
fn moving_highlight_preserves_unaffected_height_caches() {
    let cells: Vec<_> = (0..32)
        .map(|_| {
            Arc::new(MeasuredCell {
                measurements: AtomicUsize::new(0),
            })
        })
        .collect();
    let mut overlay = TranscriptOverlay::new(
        cells
            .iter()
            .map(|cell| cell.clone() as Arc<dyn HistoryCell>)
            .collect(),
        RuntimeKeymap::defaults().pager,
    );
    let mut area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 12,
    );
    overlay.render(area, &mut Buffer::empty(area));

    for selection in [Some(30), Some(28), Some(28), None] {
        overlay.set_highlight_cell(selection);
        overlay.render(area, &mut Buffer::empty(area));
    }

    let measurements = || {
        cells
            .iter()
            .map(|cell| cell.measurements.load(Ordering::Relaxed))
            .collect::<Vec<_>>()
    };
    let mut expected = vec![1; cells.len()];
    assert_eq!(measurements(), expected);

    // Width changes still invalidate every cached height.
    area.width = 24;
    overlay.render(area, &mut Buffer::empty(area));
    for count in &mut expected {
        *count += 1;
    }
    assert_eq!(measurements(), expected);
}

#[test]
fn moving_highlight_matches_full_rebuild_with_live_tail() {
    let cells: Vec<Arc<dyn HistoryCell>> = ["first prompt", "second prompt"]
        .into_iter()
        .map(|message| {
            Arc::new(UserHistoryCell {
                message: message.to_string(),
                text_elements: Vec::new(),
                local_image_paths: Vec::new(),
                remote_image_urls: Vec::new(),
                spoken: false,
            }) as Arc<dyn HistoryCell>
        })
        .collect();
    let mut actual = TranscriptOverlay::new(cells.clone(), RuntimeKeymap::defaults().pager);
    let mut expected = TranscriptOverlay::new(cells, RuntimeKeymap::defaults().pager);
    for overlay in [&mut actual, &mut expected] {
        overlay.sync_live_tail(
            /*width*/ 40,
            Some(ActiveCellTranscriptKey {
                cacheable: true,
                revision: 1,
                is_stream_continuation: false,
                animation_tick: None,
            }),
            |_| Some(vec![HyperlinkLine::from("live tail")]),
        );
    }

    for width in [40, 24, 40] {
        for selection in [Some(0), Some(1), Some(1), None, Some(99), Some(0)] {
            actual.set_highlight_cell(selection);
            let tail = expected.take_live_tail_renderable();
            expected.highlight_cell = selection;
            expected.rebuild_renderables(tail);
            if let Some(index) = selection {
                expected.view.scroll_chunk_into_view(index);
            }

            let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 12);
            let mut actual_buffer = Buffer::empty(area);
            let mut expected_buffer = Buffer::empty(area);
            actual.render(area, &mut actual_buffer);
            expected.render(area, &mut expected_buffer);
            assert_eq!(actual_buffer, expected_buffer);
            assert_eq!(actual.view.scroll_offset, expected.view.scroll_offset);
        }
    }

    actual.set_highlight_cell(Some(1));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 40, /*height*/ 20,
    );
    let mut buffer = Buffer::empty(area);
    actual.render(area, &mut buffer);
    let content = (1..area.height - 4)
        .map(|y| {
            (0..area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .map(|line| line.trim_end().to_string())
        .filter(|line| !line.is_empty() && line != "~")
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(content, @"
    › first prompt
    › second prompt
    live tail
    ");
}
